use axum::{routing::get, Router};
use hdckit_rs::{Client as HdcClient, HilogQueryOptions};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    Json, ServerHandler,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::hdc_bin::build_hdc_client_from_config;

const DEFAULT_HILOG_SEARCH_TAIL_LINES: u32 = 200;
const MAX_HILOG_SEARCH_LINES: u32 = 2_000;
const MAX_HILOG_SEARCH_OUTPUT_BYTES: usize = 256 * 1024;
#[cfg(test)]
const HILOG_SEARCH_TOOL_NAME: &str = "hdc.search_hilog_logs";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpToolSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct HilogSearchToolRequest {
    connect_key: Option<String>,
    regex: String,
    head_lines: Option<u32>,
    tail_lines: Option<u32>,
    log_types: Option<String>,
    level: Option<String>,
    domain: Option<String>,
    tag: Option<String>,
    pid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HilogSearchToolResponse {
    connect_key: String,
    auto_selected_target: bool,
    command: Vec<String>,
    regex: String,
    logs: String,
    truncated: bool,
    total_line_count: usize,
    returned_line_count: usize,
    total_byte_count: usize,
    returned_byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_lines: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tail_lines: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    log_types: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedHilogSearchRequest {
    connect_key: Option<String>,
    regex: String,
    head_lines: Option<u32>,
    tail_lines: Option<u32>,
    log_types: Option<String>,
    level: Option<String>,
    domain: Option<String>,
    tag: Option<String>,
    pid: Option<i64>,
}

#[derive(Debug, Clone)]
pub(crate) struct HarmonyMcpServer {
    tool_router: ToolRouter<Self>,
}

impl Default for HarmonyMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router(router = tool_router)]
impl HarmonyMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "hdc.search_hilog_logs",
        description = "Search buffered Hilog logs on a connected HarmonyOS/OpenHarmony device using bounded `hdc shell hilog -z/-a ...` queries with optional Hilog filters.",
        annotations(
            title = "Search Device Hilog Logs",
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn search_hilog_logs(
        &self,
        params: Parameters<HilogSearchToolRequest>,
    ) -> Result<Json<HilogSearchToolResponse>, String> {
        let request = normalize_hilog_search_request(params.0)?;
        let client = build_hdc_client_from_config()?;
        let (connect_key, auto_selected_target) =
            resolve_connect_key(&client, request.connect_key.clone()).await?;
        let target = client
            .get_target(connect_key.clone())
            .map_err(|error| error.to_string())?;

        let options = HilogQueryOptions {
            regex: request.regex.clone(),
            head_lines: request.head_lines,
            tail_lines: request.tail_lines,
            log_types: request.log_types.clone(),
            level: request.level.clone(),
            domain: request.domain.clone(),
            tag: request.tag.clone(),
            pid: request.pid,
        };

        let shell_args = options.to_shell_args().map_err(|error| error.to_string())?;
        let raw_output = target
            .query_hilog(&options)
            .await
            .map_err(|error| error.to_string())?;
        let limited_output =
            apply_requested_line_window(&raw_output, request.head_lines, request.tail_lines);

        let total_line_count = count_lines(&limited_output);
        let total_byte_count = limited_output.len();
        let (logs, truncated) = truncate_utf8(&limited_output, MAX_HILOG_SEARCH_OUTPUT_BYTES);
        let returned_line_count = count_lines(&logs);
        let returned_byte_count = logs.len();

        Ok(Json(HilogSearchToolResponse {
            connect_key: connect_key.clone(),
            auto_selected_target,
            command: build_hdc_command_preview(&connect_key, &shell_args),
            regex: request.regex,
            logs,
            truncated,
            total_line_count,
            returned_line_count,
            total_byte_count,
            returned_byte_count,
            head_lines: request.head_lines,
            tail_lines: request.tail_lines,
            log_types: request.log_types,
            level: request.level,
            domain: request.domain,
            tag: request.tag,
            pid: request.pid,
        }))
    }
}

pub(crate) fn list_builtin_mcp_tools() -> Vec<McpToolSummary> {
    let mut tools = HarmonyMcpServer::new()
        .tool_router
        .list_all()
        .into_iter()
        .map(|tool| McpToolSummary {
            name: tool.name.to_string(),
            title: tool.title.or_else(|| {
                tool.annotations
                    .as_ref()
                    .and_then(|annotations| annotations.title.clone())
            }),
            description: tool.description.map(|value| value.into_owned()),
        })
        .collect::<Vec<_>>();

    tools.sort_by(|left, right| left.name.cmp(&right.name));
    tools
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for HarmonyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: env!("CARGO_PKG_NAME").to_string(),
                title: Some("Harmony HDC Bridge".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some(
                    "Harmony MCP server exposing HDC-backed device tools for code AI agents."
                        .to_string(),
                ),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Use `hdc.search_hilog_logs` to query buffered device Hilog output. `connectKey` is optional only when exactly one device is connected."
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

fn normalize_hilog_search_request(
    request: HilogSearchToolRequest,
) -> Result<NormalizedHilogSearchRequest, String> {
    let connect_key = normalize_optional_string(request.connect_key);
    let regex = request.regex.trim();
    if regex.is_empty() {
        return Err("`regex` must be a non-empty string".to_string());
    }

    let head_lines = validate_line_limit("headLines", request.head_lines)?;
    let mut tail_lines = validate_line_limit("tailLines", request.tail_lines)?;

    if head_lines.is_some() && tail_lines.is_some() {
        return Err("`headLines` and `tailLines` are mutually exclusive".to_string());
    }

    if head_lines.is_none() && tail_lines.is_none() {
        tail_lines = Some(DEFAULT_HILOG_SEARCH_TAIL_LINES);
    }

    let pid = if let Some(pid) = request.pid {
        if pid <= 0 {
            return Err("`pid` must be a positive integer".to_string());
        }
        Some(pid)
    } else {
        None
    };

    Ok(NormalizedHilogSearchRequest {
        connect_key,
        regex: regex.to_string(),
        head_lines,
        tail_lines,
        log_types: normalize_optional_string(request.log_types),
        level: normalize_optional_string(request.level),
        domain: normalize_optional_string(request.domain),
        tag: normalize_optional_string(request.tag),
        pid,
    })
}

fn validate_line_limit(field_name: &str, value: Option<u32>) -> Result<Option<u32>, String> {
    let Some(value) = value else {
        return Ok(None);
    };

    if value == 0 {
        return Err(format!("`{field_name}` must be a positive integer"));
    }

    if value > MAX_HILOG_SEARCH_LINES {
        return Err(format!(
            "`{field_name}` must be <= {MAX_HILOG_SEARCH_LINES}"
        ));
    }

    Ok(Some(value))
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn resolve_connect_key(
    client: &HdcClient,
    requested_connect_key: Option<String>,
) -> Result<(String, bool), String> {
    if let Some(connect_key) = requested_connect_key {
        return Ok((connect_key, false));
    }

    let targets = client
        .list_targets()
        .await
        .map_err(|error| error.to_string())?;

    match targets.as_slice() {
        [] => Err(
            "No connected HDC targets found. Connect a device or pass `connectKey` explicitly."
                .to_string(),
        ),
        [target] => Ok((target.clone(), true)),
        _ => Err(format!(
            "Multiple HDC targets are connected. Pass `connectKey`. Available targets: {}",
            targets.join(", ")
        )),
    }
}

fn build_hdc_command_preview(connect_key: &str, shell_args: &[String]) -> Vec<String> {
    let mut command = vec![
        "hdc".to_string(),
        "-t".to_string(),
        connect_key.to_string(),
        "shell".to_string(),
    ];
    command.extend(shell_args.iter().cloned());
    command
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_string(), false);
    }

    let mut truncation_index = max_bytes;
    while truncation_index > 0 && !value.is_char_boundary(truncation_index) {
        truncation_index -= 1;
    }

    (value[..truncation_index].to_string(), true)
}

fn apply_requested_line_window(
    value: &str,
    head_lines: Option<u32>,
    tail_lines: Option<u32>,
) -> String {
    if head_lines.is_none() && tail_lines.is_none() {
        return value.to_string();
    }

    let lines = value.lines().collect::<Vec<_>>();
    let selected = if let Some(head_lines) = head_lines {
        &lines[..lines.len().min(head_lines as usize)]
    } else if let Some(tail_lines) = tail_lines {
        let start = lines.len().saturating_sub(tail_lines as usize);
        &lines[start..]
    } else {
        &lines[..]
    };

    selected.join("\n")
}

fn count_lines(value: &str) -> usize {
    if value.is_empty() {
        0
    } else {
        value.lines().count()
    }
}

async fn health_handler() -> &'static str {
    "ok"
}

pub(crate) fn build_mcp_router() -> Router {
    let service: StreamableHttpService<HarmonyMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(HarmonyMcpServer::new()),
            Default::default(),
            StreamableHttpServerConfig::default(),
        );

    Router::new()
        .nest_service("/mcp", service)
        .route("/health", get(health_handler))
}

pub async fn run_mcp_http_server(http_addr: &str) -> Result<(), String> {
    let listener = TcpListener::bind(http_addr)
        .await
        .map_err(|error| format!("failed to bind mcp http server ({http_addr}): {error}"))?;

    println!(
        "Harmony MCP server listening on http://{http_addr}/mcp (health: http://{http_addr}/health)"
    );

    axum::serve(listener, build_mcp_router())
        .await
        .map_err(|error| format!("mcp http server failed ({http_addr}): {error}"))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::util::ServiceExt;

    use super::{
        apply_requested_line_window, build_mcp_router, count_lines, list_builtin_mcp_tools,
        normalize_hilog_search_request, truncate_utf8, HarmonyMcpServer, HilogSearchToolRequest,
        DEFAULT_HILOG_SEARCH_TAIL_LINES, HILOG_SEARCH_TOOL_NAME,
    };

    #[test]
    fn hilog_search_tool_is_registered_with_schema_and_annotations() {
        let server = HarmonyMcpServer::new();
        let tools = server.tool_router.list_all();
        let tool = tools
            .iter()
            .find(|tool| tool.name == HILOG_SEARCH_TOOL_NAME)
            .expect("hilog search tool should be registered");

        assert!(tool.output_schema.is_some());
        assert_eq!(
            tool.annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint),
            Some(true)
        );
        assert_eq!(
            tool.annotations
                .as_ref()
                .and_then(|annotations| annotations.idempotent_hint),
            Some(true)
        );

        let input_schema = serde_json::to_string(&tool.input_schema).expect("serialize schema");
        assert!(input_schema.contains("connectKey"));
        assert!(input_schema.contains("regex"));
        assert!(input_schema.contains("tailLines"));
    }

    #[test]
    fn normalize_hilog_request_defaults_to_recent_tail_window() {
        let normalized = normalize_hilog_search_request(HilogSearchToolRequest {
            connect_key: Some("  ".to_string()),
            regex: " panic ".to_string(),
            head_lines: None,
            tail_lines: None,
            log_types: Some(" app ".to_string()),
            level: Some(" E,F ".to_string()),
            domain: None,
            tag: None,
            pid: None,
        })
        .expect("request should normalize");

        assert_eq!(normalized.connect_key, None);
        assert_eq!(normalized.regex, "panic");
        assert_eq!(normalized.tail_lines, Some(DEFAULT_HILOG_SEARCH_TAIL_LINES));
        assert_eq!(normalized.log_types.as_deref(), Some("app"));
        assert_eq!(normalized.level.as_deref(), Some("E,F"));
    }

    #[test]
    fn truncate_and_line_count_helpers_behave_as_expected() {
        let output = "one\ntwo\nthree\n";
        assert_eq!(count_lines(output), 3);

        let (truncated, was_truncated) = truncate_utf8(output, 5);
        assert!(was_truncated);
        assert_eq!(truncated, "one\nt");
    }

    #[test]
    fn requested_line_window_is_enforced_locally() {
        assert_eq!(
            apply_requested_line_window("one\ntwo\nthree\nfour\n", Some(2), None),
            "one\ntwo"
        );
        assert_eq!(
            apply_requested_line_window("one\ntwo\nthree\nfour\n", None, Some(2)),
            "three\nfour"
        );
    }

    #[test]
    fn builtin_mcp_tool_summaries_include_hilog_search_tool() {
        let tools = list_builtin_mcp_tools();
        let tool = tools
            .iter()
            .find(|tool| tool.name == HILOG_SEARCH_TOOL_NAME)
            .expect("hilog search tool should be exported");

        assert_eq!(tool.title.as_deref(), Some("Search Device Hilog Logs"));
        assert!(tool
            .description
            .as_deref()
            .is_some_and(|description| description.contains("Search buffered Hilog logs")));
    }

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let response = build_mcp_router()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("health request should build"),
            )
            .await
            .expect("health request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("health body should be readable");
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn initialize_request_returns_valid_mcp_payload() {
        let response = build_mcp_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .header("accept", "application/json, text/event-stream")
                    .body(Body::from(
                        json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "initialize",
                            "params": {
                                "protocolVersion": "2025-11-25",
                                "capabilities": {},
                                "clientInfo": {
                                    "name": "integration-test",
                                    "version": "1.0.0"
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .expect("initialize request should build"),
            )
            .await
            .expect("initialize request should succeed");

        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(
            content_type.contains("text/event-stream"),
            "unexpected content type: {content_type}"
        );

        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("response body should be readable")
                .to_vec(),
        )
        .expect("response body should be utf-8");
        assert!(body.contains("data:"));
        assert!(body.contains("\"jsonrpc\":\"2.0\""));
        assert!(body.contains("\"id\":1"));
        assert!(body.contains("\"protocolVersion\""));
        assert!(body.contains("\"serverInfo\""));
        assert!(body.contains("\"tools\""));
    }

    #[tokio::test]
    async fn invalid_route_and_method_return_expected_status_codes() {
        let missing_route_response = build_mcp_router()
            .oneshot(
                Request::builder()
                    .uri("/does-not-exist")
                    .body(Body::empty())
                    .expect("missing route request should build"),
            )
            .await
            .expect("missing route request should complete");
        assert_eq!(missing_route_response.status(), StatusCode::NOT_FOUND);

        let invalid_method_response = build_mcp_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/health")
                    .body(Body::empty())
                    .expect("invalid method request should build"),
            )
            .await
            .expect("invalid method request should complete");
        assert_eq!(
            invalid_method_response.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }
}
