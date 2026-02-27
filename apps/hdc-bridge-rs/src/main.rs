use hdc_bridge_rs::{run_bridge, DEFAULT_WS_ADDR};

fn parse_ws_addr() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let mut ws_addr = DEFAULT_WS_ADDR.to_string();

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
            "-h" | "--help" => {
                println!("Usage: hdc-bridge-rs [--ws-addr <host:port>]");
                std::process::exit(0);
            }
            _ => {
                return Err(format!(
                    "unknown argument: {arg}. Usage: hdc-bridge-rs [--ws-addr <host:port>]"
                ))
            }
        }
    }

    Ok(ws_addr)
}

#[tokio::main]
async fn main() {
    let ws_addr = match parse_ws_addr() {
        Ok(value) => value,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };

    if let Err(error) = run_bridge(&ws_addr).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
