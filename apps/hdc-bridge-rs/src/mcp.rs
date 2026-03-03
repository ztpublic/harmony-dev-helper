use axum::{routing::get, Router};
use rmcp::{
    model::{Implementation, ServerCapabilities, ServerInfo},
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ServerHandler,
};
use tokio::net::TcpListener;

#[derive(Debug, Clone, Default)]
pub(crate) struct HarmonyMcpServer;

impl ServerHandler for HarmonyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().build(),
            server_info: Implementation {
                name: env!("CARGO_PKG_NAME").to_string(),
                title: Some("Harmony HDC Bridge".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some(
                    "Infrastructure-only MCP endpoint. HDC tool surface is not exposed yet."
                        .to_string(),
                ),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "This MCP endpoint is infrastructure-only for now. HDC tools are not exposed in this iteration."
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

async fn health_handler() -> &'static str {
    "ok"
}

pub(crate) fn build_mcp_router() -> Router {
    let service: StreamableHttpService<HarmonyMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(HarmonyMcpServer),
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
    use std::net::SocketAddr;

    use axum::http::StatusCode;
    use reqwest::header::{ACCEPT, CONTENT_TYPE};
    use serde_json::json;
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::task::JoinHandle;

    use super::build_mcp_router;

    struct TestServer {
        addr: SocketAddr,
        shutdown_tx: Option<oneshot::Sender<()>>,
        handle: JoinHandle<()>,
    }

    impl TestServer {
        async fn spawn() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind test server");
            let addr = listener.local_addr().expect("resolve local addr");
            let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
            let app = build_mcp_router();

            let handle = tokio::spawn(async move {
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    })
                    .await;
            });

            Self {
                addr,
                shutdown_tx: Some(shutdown_tx),
                handle,
            }
        }

        fn url(&self, path: &str) -> String {
            format!("http://{}{}", self.addr, path)
        }

        async fn shutdown(mut self) {
            if let Some(sender) = self.shutdown_tx.take() {
                let _ = sender.send(());
            }
            let _ = self.handle.await;
        }
    }

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let server = TestServer::spawn().await;
        let client = reqwest::Client::new();
        let response = client
            .get(server.url("/health"))
            .send()
            .await
            .expect("health request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .text()
                .await
                .expect("health body should be readable"),
            "ok"
        );

        server.shutdown().await;
    }

    #[tokio::test]
    async fn initialize_request_returns_valid_mcp_payload() {
        let server = TestServer::spawn().await;
        let client = reqwest::Client::new();
        let response = client
            .post(server.url("/mcp"))
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream")
            .body(
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
            )
            .send()
            .await
            .expect("initialize request should succeed");

        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(
            content_type.contains("text/event-stream"),
            "unexpected content type: {content_type}"
        );

        let body = response
            .text()
            .await
            .expect("response body should be readable");
        assert!(body.contains("data:"));
        assert!(body.contains("\"jsonrpc\":\"2.0\""));
        assert!(body.contains("\"id\":1"));
        assert!(body.contains("\"protocolVersion\""));
        assert!(body.contains("\"serverInfo\""));

        server.shutdown().await;
    }

    #[tokio::test]
    async fn invalid_route_and_method_return_expected_status_codes() {
        let server = TestServer::spawn().await;
        let client = reqwest::Client::new();

        let missing_route_response = client
            .get(server.url("/does-not-exist"))
            .send()
            .await
            .expect("missing route request should complete");
        assert_eq!(missing_route_response.status(), StatusCode::NOT_FOUND);

        let invalid_method_response = client
            .post(server.url("/health"))
            .send()
            .await
            .expect("invalid method request should complete");
        assert_eq!(
            invalid_method_response.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );

        server.shutdown().await;
    }
}
