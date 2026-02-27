use futures_util::{SinkExt, StreamExt};
use hdckit_rs::Client as HdcClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const WS_ADDR: &str = "127.0.0.1:8787";

#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
  id: String,
  #[serde(rename = "type")]
  kind: String,
  payload: Value,
  ts: u64,
}

#[derive(Debug, Deserialize)]
struct InvokePayload {
  action: String,
  #[serde(default)]
  args: Value,
}

fn now_ms() -> u64 {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default();

  now.as_millis() as u64
}

fn host_message(id: String, kind: &str, payload: Value) -> Envelope {
  Envelope {
    id,
    kind: kind.to_string(),
    payload,
    ts: now_ms(),
  }
}

fn required_string_arg(args: &Value, key: &str) -> Result<String, String> {
  let value = args
    .get(key)
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(ToString::to_string);

  value.ok_or_else(|| format!("`{key}` must be a non-empty string"))
}

fn hdc_error(id: String, message: String) -> Envelope {
  host_message(
    id,
    "error",
    json!({
      "code": "HDC_ERROR",
      "message": message
    }),
  )
}

async fn handle_invoke(id: String, payload: Value) -> Envelope {
  let invoke = match serde_json::from_value::<InvokePayload>(payload) {
    Ok(value) => value,
    Err(error) => {
      return host_message(
        id,
        "error",
        json!({
          "code": "INVALID_INVOKE_PAYLOAD",
          "message": format!("Expected payload: {{ action: string, args?: object }} ({error})")
        }),
      )
    }
  };

  let client = HdcClient::from_env();

  match invoke.action.as_str() {
    "hdc.listTargets" => match client.list_targets().await {
      Ok(targets) => host_message(
        id,
        "event",
        json!({
          "name": "hdc.listTargets.result",
          "data": {
            "targets": targets
          }
        }),
      ),
      Err(error) => hdc_error(id, error.to_string()),
    },
    "hdc.getParameters" => {
      let connect_key = match required_string_arg(&invoke.args, "connectKey") {
        Ok(value) => value,
        Err(message) => {
          return host_message(
            id,
            "error",
            json!({
              "code": "INVALID_ARGS",
              "message": message
            }),
          )
        }
      };

      match client.get_target(connect_key) {
        Ok(target) => match target.get_parameters().await {
          Ok(parameters) => host_message(
            id,
            "event",
            json!({
              "name": "hdc.getParameters.result",
              "data": {
                "parameters": parameters
              }
            }),
          ),
          Err(error) => hdc_error(id, error.to_string()),
        },
        Err(error) => hdc_error(id, error.to_string()),
      }
    }
    "hdc.shell" => {
      let connect_key = match required_string_arg(&invoke.args, "connectKey") {
        Ok(value) => value,
        Err(message) => {
          return host_message(
            id,
            "error",
            json!({
              "code": "INVALID_ARGS",
              "message": message
            }),
          )
        }
      };

      let command = match required_string_arg(&invoke.args, "command") {
        Ok(value) => value,
        Err(message) => {
          return host_message(
            id,
            "error",
            json!({
              "code": "INVALID_ARGS",
              "message": message
            }),
          )
        }
      };

      match client.get_target(connect_key) {
        Ok(target) => match target.shell(&command).await {
          Ok(mut session) => match session.read_all_string().await {
            Ok(output) => {
              let _ = session.end().await;
              host_message(
                id,
                "event",
                json!({
                  "name": "hdc.shell.result",
                  "data": {
                    "output": output
                  }
                }),
              )
            }
            Err(error) => hdc_error(id, error.to_string()),
          },
          Err(error) => hdc_error(id, error.to_string()),
        },
        Err(error) => hdc_error(id, error.to_string()),
      }
    }
    _ => host_message(
      id,
      "error",
      json!({
        "code": "UNKNOWN_ACTION",
        "message": format!("Unsupported invoke action: {}", invoke.action)
      }),
    ),
  }
}

async fn handle_client(stream: TcpStream) {
  let ws_stream = match accept_async(stream).await {
    Ok(value) => value,
    Err(error) => {
      eprintln!("ws handshake failed: {error}");
      return;
    }
  };

  let (mut write, mut read) = ws_stream.split();

  while let Some(next) = read.next().await {
    let Ok(message) = next else {
      continue;
    };

    if let Message::Text(text) = message {
      let parsed = serde_json::from_str::<Envelope>(&text);

      let response = match parsed {
        Ok(incoming) if incoming.kind == "invoke" => handle_invoke(incoming.id, incoming.payload).await,
        Ok(incoming) => host_message(
          incoming.id,
          "error",
          json!({
            "code": "UNSUPPORTED_MESSAGE_TYPE",
            "message": format!("Unsupported message type: {}", incoming.kind)
          }),
        ),
        Err(_) => host_message(
          "decode-error".to_string(),
          "error",
          json!({
            "code": "INVALID_MESSAGE",
            "message": "Expected Harmony protocol JSON envelope"
          }),
        ),
      };

      let payload = match serde_json::to_string(&response) {
        Ok(value) => value,
        Err(_) => continue,
      };

      if write.send(Message::Text(payload.into())).await.is_err() {
        break;
      }
    }
  }
}

async fn run_websocket_bridge() -> Result<(), String> {
  let listener = TcpListener::bind(WS_ADDR)
    .await
    .map_err(|error| format!("failed to bind websocket bridge ({WS_ADDR}): {error}"))?;

  println!("Harmony Tauri websocket bridge listening on ws://{WS_ADDR}");

  loop {
    let accepted = listener.accept().await;
    let (stream, _) = match accepted {
      Ok(value) => value,
      Err(error) => {
        eprintln!("accept error: {error}");
        continue;
      }
    };

    tauri::async_runtime::spawn(handle_client(stream));
  }
}

fn main() {
  tauri::Builder::default()
    .plugin(tauri_plugin_log::Builder::default().build())
    .setup(|_app| {
      tauri::async_runtime::spawn(async {
        if let Err(error) = run_websocket_bridge().await {
          eprintln!("{error}");
        }
      });

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
