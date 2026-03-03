use hdc_bridge_rs::{run_bridge_with_mcp, DEFAULT_WS_ADDR};

#[derive(Debug)]
struct RuntimeConfig {
    ws_addr: String,
    mcp_http_addr: Option<String>,
}

fn parse_runtime_config() -> Result<RuntimeConfig, String> {
    let mut args = std::env::args().skip(1);
    let mut ws_addr = DEFAULT_WS_ADDR.to_string();
    let mut mcp_http_addr: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ws-addr" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --ws-addr".to_string())?;
                if value.trim().is_empty() {
                    return Err("--ws-addr must be a non-empty string".to_string());
                }
                ws_addr = value;
            }
            "--mcp-http-addr" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --mcp-http-addr".to_string())?;
                if value.trim().is_empty() {
                    return Err("--mcp-http-addr must be a non-empty string".to_string());
                }
                mcp_http_addr = Some(value);
            }
            "-h" | "--help" => {
                println!(
                    "Usage: hdc-bridge-rs [--ws-addr <host:port>] [--mcp-http-addr <host:port>]"
                );
                std::process::exit(0);
            }
            _ => {
                return Err(format!(
                    "unknown argument: {arg}. Usage: hdc-bridge-rs [--ws-addr <host:port>] [--mcp-http-addr <host:port>]"
                ))
            }
        }
    }

    Ok(RuntimeConfig {
        ws_addr,
        mcp_http_addr,
    })
}

#[tokio::main]
async fn main() {
    let runtime_config = match parse_runtime_config() {
        Ok(value) => value,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };

    if let Err(error) = run_bridge_with_mcp(
        &runtime_config.ws_addr,
        runtime_config.mcp_http_addr.as_deref(),
    )
    .await
    {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
