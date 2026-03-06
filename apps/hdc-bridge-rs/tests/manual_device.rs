use std::path::{Path, PathBuf};
use std::time::Duration;

use hdc_bridge_rs::run_bridge_with_mcp;
use hdckit_rs::{Client, ClientOptions, HdcError};
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::time::{sleep, Instant};

const HDC_BIN: &str =
    "/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc";
const ACCEPT_BOTH: &str = "application/json, text/event-stream";
const MCP_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

fn hdc_client() -> Client {
    assert!(
        Path::new(HDC_BIN).exists(),
        "hdc binary not found at expected path: {HDC_BIN}"
    );

    Client::new(ClientOptions {
        host: "127.0.0.1".to_string(),
        port: std::env::var("OHOS_HDC_SERVER_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8710),
        bin: PathBuf::from(HDC_BIN),
    })
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind ephemeral port")
        .local_addr()
        .expect("failed to read local addr")
        .port()
}

async fn first_target_key(client: &Client) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        match client.list_targets().await {
            Ok(targets) => match targets.into_iter().next() {
                Some(target) => return target,
                None => panic!("expected at least one connected HarmonyOS device"),
            },
            Err(HdcError::Io(err))
                if err.kind() == std::io::ErrorKind::ConnectionRefused
                    && Instant::now() < deadline =>
            {
                sleep(Duration::from_millis(300)).await;
            }
            Err(err) => panic!("list targets failed: {err:?}"),
        }
    }
}

async fn wait_for_health(client: &reqwest::Client, health_url: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        match client.get(health_url).send().await {
            Ok(response) if response.status() == StatusCode::OK => return,
            Ok(response) if Instant::now() < deadline => sleep(Duration::from_millis(150)).await,
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                panic!("health endpoint did not become ready: {status}, body={body:?}");
            }
            Err(_) if Instant::now() < deadline => sleep(Duration::from_millis(150)).await,
            Err(error) => panic!("health endpoint did not become ready: {error}"),
        }
    }
}

fn extract_first_sse_json(body: &str) -> Option<Value> {
    let normalized = body.replace("\r\n", "\n");
    for event in normalized.split("\n\n") {
        let payload = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        if payload.is_empty() {
            continue;
        }

        if let Ok(value) = serde_json::from_str(&payload) {
            return Some(value);
        }
    }

    None
}

async fn read_mcp_response_json(response: reqwest::Response) -> Value {
    let mut response = response;
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if content_type.contains("application/json") {
        let body = response
            .text()
            .await
            .expect("JSON response should be readable");
        return serde_json::from_str(&body).expect("JSON response should decode");
    }

    assert!(
        content_type.contains("text/event-stream"),
        "unexpected MCP response content type: {content_type}"
    );

    let body = tokio::time::timeout(MCP_RESPONSE_TIMEOUT, async {
        let mut body = String::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .expect("SSE response chunk should be readable")
        {
            body.push_str(&String::from_utf8_lossy(&chunk));
            if extract_first_sse_json(&body).is_some() {
                break;
            }
        }
        body
    })
    .await
    .expect("timed out waiting for first MCP SSE event");

    extract_first_sse_json(&body).unwrap_or_else(|| {
        panic!("expected SSE response with at least one JSON data frame, got: {body}")
    })
}

async fn initialize_session(client: &reqwest::Client, mcp_url: &str) -> String {
    let response = client
        .post(mcp_url)
        .header("Accept", ACCEPT_BOTH)
        .header("Content-Type", "application/json")
        .body(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "manual-device-test",
                        "version": "1.0.0"
                    }
                }
            })
            .to_string(),
        )
        .send()
        .await
        .expect("initialize request should succeed");

    assert!(
        response.status().is_success(),
        "initialize request failed with status {}",
        response.status()
    );

    response
        .headers()
        .get("mcp-session-id")
        .expect("initialize response should include MCP session id")
        .to_str()
        .expect("session id should be valid ASCII")
        .to_string()
}

async fn send_initialized_notification(client: &reqwest::Client, mcp_url: &str, session_id: &str) {
    let response = client
        .post(mcp_url)
        .header("Accept", ACCEPT_BOTH)
        .header("Content-Type", "application/json")
        .header("Mcp-Session-Id", session_id)
        .body(
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })
            .to_string(),
        )
        .send()
        .await
        .expect("initialized notification should succeed");

    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "initialized notification should return 202 Accepted"
    );
}

async fn post_mcp_request(
    client: &reqwest::Client,
    mcp_url: &str,
    session_id: &str,
    body: Value,
) -> Value {
    let response = client
        .post(mcp_url)
        .header("Accept", ACCEPT_BOTH)
        .header("Content-Type", "application/json")
        .header("Mcp-Session-Id", session_id)
        .body(body.to_string())
        .send()
        .await
        .expect("MCP request should succeed");

    assert!(
        response.status().is_success(),
        "MCP request failed with status {}",
        response.status()
    );

    read_mcp_response_json(response).await
}

#[tokio::test(flavor = "multi_thread")]
async fn integration_mcp_search_hilog_logs() {
    let connect_key = first_target_key(&hdc_client()).await;
    let ws_addr = format!("127.0.0.1:{}", find_free_port());
    let mcp_http_addr = format!("127.0.0.1:{}", find_free_port());
    let health_url = format!("http://{mcp_http_addr}/health");
    let mcp_url = format!("http://{mcp_http_addr}/mcp");

    let bridge_handle = tokio::spawn({
        let ws_addr = ws_addr.clone();
        let mcp_http_addr = mcp_http_addr.clone();
        async move { run_bridge_with_mcp(&ws_addr, Some(&mcp_http_addr)).await }
    });

    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("should build reqwest client without proxies");
    wait_for_health(&client, &health_url).await;

    let session_id = initialize_session(&client, &mcp_url).await;
    send_initialized_notification(&client, &mcp_url, &session_id).await;

    let list_tools_response = post_mcp_request(
        &client,
        &mcp_url,
        &session_id,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await;

    let tool_names = list_tools_response["result"]["tools"]
        .as_array()
        .expect("tools/list should return a tools array")
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(
        tool_names.contains(&"hdc.search_hilog_logs"),
        "hdc.search_hilog_logs should be exposed, got {tool_names:?}"
    );

    let search_response = post_mcp_request(
        &client,
        &mcp_url,
        &session_id,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "hdc.search_hilog_logs",
                "arguments": {
                    "connectKey": connect_key.clone(),
                    "regex": ".",
                    "tailLines": 20
                }
            }
        }),
    )
    .await;

    let result = &search_response["result"];
    assert_eq!(result["isError"], Value::Bool(false));

    let structured = result["structuredContent"]
        .as_object()
        .expect("tools/call should return structuredContent");
    assert_eq!(
        structured.get("connectKey").and_then(Value::as_str),
        Some(connect_key.as_str())
    );
    assert_eq!(structured.get("regex").and_then(Value::as_str), Some("."));
    assert_eq!(
        structured.get("tailLines").and_then(Value::as_u64),
        Some(20)
    );
    assert!(
        structured
            .get("command")
            .and_then(Value::as_array)
            .is_some(),
        "response should include the executed HDC command preview"
    );
    assert!(
        structured.get("logs").and_then(Value::as_str).is_some(),
        "response should include a logs string"
    );
    assert!(
        structured
            .get("returnedLineCount")
            .and_then(Value::as_u64)
            .expect("returnedLineCount should exist")
            <= 20,
        "tailLines limit should cap returnedLineCount"
    );
    assert!(
        structured
            .get("totalLineCount")
            .and_then(Value::as_u64)
            .expect("totalLineCount should exist")
            >= structured
                .get("returnedLineCount")
                .and_then(Value::as_u64)
                .expect("returnedLineCount should exist"),
        "totalLineCount should be >= returnedLineCount"
    );

    bridge_handle.abort();
    let _ = bridge_handle.await;
}
