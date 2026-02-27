use futures_util::{SinkExt, StreamExt};
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
        Ok(incoming) if incoming.kind == "ping" => host_message(
          incoming.id,
          "pong",
          json!({
            "host": "tauri",
            "note": "pong from rust websocket bridge"
          }),
        ),
        Ok(incoming) => host_message(
          incoming.id,
          "event",
          json!({
            "name": "invoke.received",
            "data": {
              "receivedType": incoming.kind,
              "host": "tauri"
            }
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
