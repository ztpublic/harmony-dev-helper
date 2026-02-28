use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use hdckit_rs::HilogEntry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, MissedTickBehavior};
use tokio_tungstenite::{accept_async, tungstenite::Message};

mod hdc_bin;

use hdc_bin::{build_hdc_client_from_config, get_bin_config, set_custom_bin_path};

pub const DEFAULT_WS_ADDR: &str = "127.0.0.1:8787";

const OUTBOUND_QUEUE_CAPACITY: usize = 128;
const QUEUE_MAX_LINES: usize = 4_000;
const BATCH_INTERVAL_MS: u64 = 40;
const BATCH_MAX_LINES: usize = 200;
const BATCH_MAX_BYTES: usize = 64 * 1024;

static NEXT_MESSAGE_ID: AtomicU64 = AtomicU64::new(1);

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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HilogPidOption {
    pid: i64,
    command: String,
}

#[derive(Debug)]
struct ActiveHilogSubscription {
    subscription_id: String,
    connect_key: String,
    stop_sender: Option<oneshot::Sender<()>>,
    task_handle: tokio::task::JoinHandle<()>,
}

impl ActiveHilogSubscription {
    async fn stop(mut self) {
        if let Some(stop_sender) = self.stop_sender.take() {
            let _ = stop_sender.send(());
        }

        let _ = self.task_handle.await;
    }
}

#[derive(Debug)]
struct ClientSession {
    outbound_tx: mpsc::Sender<Envelope>,
    active_hilog: Option<ActiveHilogSubscription>,
}

impl ClientSession {
    fn new(outbound_tx: mpsc::Sender<Envelope>) -> Self {
        Self {
            outbound_tx,
            active_hilog: None,
        }
    }

    async fn stop_active_hilog(&mut self) -> Option<String> {
        let active = self.active_hilog.take()?;
        let subscription_id = active.subscription_id.clone();

        println!(
            "stopping hilog subscription {} ({})",
            active.subscription_id, active.connect_key
        );

        active.stop().await;
        Some(subscription_id)
    }

    async fn stop_active_hilog_matching(&mut self, expected_id: Option<&str>) -> Option<String> {
        if let Some(expected_id) = expected_id {
            let active_id = self
                .active_hilog
                .as_ref()
                .map(|active| active.subscription_id.as_str());

            if active_id != Some(expected_id) {
                return None;
            }
        }

        self.stop_active_hilog().await
    }
}

#[derive(Debug, Default)]
struct HilogBatcher {
    lines: VecDeque<String>,
    bytes: usize,
    dropped_since_last_emit: u64,
}

impl HilogBatcher {
    fn push_line(&mut self, line: String) {
        self.bytes += line.len();
        self.lines.push_back(line);

        while self.lines.len() > QUEUE_MAX_LINES {
            if let Some(removed) = self.lines.pop_front() {
                self.bytes = self.bytes.saturating_sub(removed.len());
                self.dropped_since_last_emit += 1;
            }
        }
    }

    fn should_flush_early(&self) -> bool {
        self.lines.len() >= BATCH_MAX_LINES || self.bytes >= BATCH_MAX_BYTES
    }

    fn has_pending(&self) -> bool {
        !self.lines.is_empty() || self.dropped_since_last_emit > 0
    }

    fn next_batch(&mut self) -> Option<(String, u64)> {
        if !self.has_pending() {
            return None;
        }

        let dropped = std::mem::take(&mut self.dropped_since_last_emit);
        let mut chunk = String::new();
        let mut line_count = 0usize;
        let mut chunk_bytes = 0usize;

        while let Some(next) = self.lines.front() {
            let next_len = next.len();
            if line_count >= BATCH_MAX_LINES {
                break;
            }
            if chunk_bytes > 0 && chunk_bytes + next_len > BATCH_MAX_BYTES {
                break;
            }

            let line = self.lines.pop_front().expect("front exists");
            self.bytes = self.bytes.saturating_sub(line.len());
            chunk_bytes += line.len();
            line_count += 1;
            chunk.push_str(&line);
        }

        Some((chunk, dropped))
    }
}

fn now_ms() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    now.as_millis() as u64
}

fn next_message_id(prefix: &str) -> String {
    let sequence = NEXT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{sequence}", now_ms())
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

fn required_nullable_string_arg(args: &Value, key: &str) -> Result<Option<String>, String> {
    let value = args
        .get(key)
        .ok_or_else(|| format!("`{key}` is required"))?;

    if value.is_null() {
        return Ok(None);
    }

    let Some(raw) = value.as_str() else {
        return Err(format!("`{key}` must be a string or null"));
    };

    let normalized = raw.trim();
    if normalized.is_empty() {
        return Ok(None);
    }

    Ok(Some(normalized.to_string()))
}

fn optional_string_arg(args: &Value, key: &str) -> Result<Option<String>, String> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };

    if value.is_null() {
        return Ok(None);
    }

    let Some(raw) = value.as_str() else {
        return Err(format!("`{key}` must be a string when provided"));
    };

    let normalized = raw.trim();
    if normalized.is_empty() {
        return Ok(None);
    }

    Ok(Some(normalized.to_string()))
}

fn optional_positive_i64_arg(args: &Value, key: &str) -> Result<Option<i64>, String> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };

    if value.is_null() {
        return Ok(None);
    }

    let Some(raw) = value.as_i64() else {
        return Err(format!("`{key}` must be a positive integer when provided"));
    };

    if raw <= 0 {
        return Err(format!("`{key}` must be a positive integer when provided"));
    }

    Ok(Some(raw))
}

fn parse_hidumper_processes(output: &str) -> Vec<HilogPidOption> {
    let has_tid_header = output.lines().any(|line| {
        let cols = line.split_whitespace().collect::<Vec<_>>();
        cols.len() >= 3 && cols[0] == "UID" && cols[1] == "PID" && cols[2] == "TID"
    });

    let mut deduped = BTreeMap::<i64, String>::new();

    for line in output.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 8 {
            continue;
        }

        let Ok(pid) = fields[1].parse::<i64>() else {
            continue;
        };

        if pid <= 0 {
            continue;
        }

        let command = if has_tid_header {
            if fields.len() < 9 {
                continue;
            }

            let Ok(tid) = fields[2].parse::<i64>() else {
                continue;
            };

            if tid <= 0 || pid != tid {
                continue;
            }

            fields[8..].join(" ")
        } else {
            fields[7..].join(" ")
        };

        if command.is_empty() {
            continue;
        }

        deduped.entry(pid).or_insert(command);
    }

    deduped
        .into_iter()
        .map(|(pid, command)| HilogPidOption { pid, command })
        .collect()
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

fn hdc_bin_error(id: String, message: String) -> Envelope {
    host_message(
        id,
        "error",
        json!({
          "code": "HDC_BIN_UNAVAILABLE",
          "message": message
        }),
    )
}

fn format_hilog_entry(entry: &HilogEntry) -> String {
    let time = entry.date.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = time.as_secs();
    let millis = time.subsec_millis();
    let level = level_to_char(entry.level);
    let level_ansi = level_to_ansi(entry.level);

    format!(
        "{seconds}.{millis:03} {} {} {level_ansi}{level}\x1b[0m {}{}/{}: {}\n",
        entry.pid,
        entry.tid,
        kind_to_char(entry.kind),
        entry.domain,
        entry.tag,
        entry.message
    )
}

fn level_to_char(level: i32) -> char {
    match level {
        2 => 'V',
        3 => 'D',
        4 => 'I',
        5 => 'W',
        6 => 'E',
        7 => 'F',
        _ => '?',
    }
}

fn level_to_ansi(level: i32) -> &'static str {
    match level {
        2 => "\x1b[90m",
        3 => "\x1b[36m",
        4 => "\x1b[32m",
        5 => "\x1b[33m",
        6 => "\x1b[31m",
        7 => "\x1b[1;31m",
        _ => "\x1b[39m",
    }
}

fn kind_to_char(kind: i32) -> char {
    match kind {
        0 => 'A',
        1 => 'I',
        2 => 'C',
        3 => 'K',
        4 => 'P',
        _ => '?',
    }
}

async fn emit_hilog_state(
    outbound_tx: &mpsc::Sender<Envelope>,
    subscription_id: &str,
    connect_key: &str,
    state: &str,
    message: Option<&str>,
) -> Result<(), ()> {
    let payload = if let Some(message) = message {
        json!({
            "name": "hdc.hilog.state",
            "data": {
                "subscriptionId": subscription_id,
                "connectKey": connect_key,
                "state": state,
                "message": message
            }
        })
    } else {
        json!({
            "name": "hdc.hilog.state",
            "data": {
                "subscriptionId": subscription_id,
                "connectKey": connect_key,
                "state": state
            }
        })
    };

    outbound_tx
        .send(host_message(next_message_id("event"), "event", payload))
        .await
        .map_err(|_| ())
}

async fn emit_hilog_batch(
    outbound_tx: &mpsc::Sender<Envelope>,
    subscription_id: &str,
    connect_key: &str,
    chunk: String,
    dropped: u64,
) -> Result<(), ()> {
    let payload = json!({
        "name": "hdc.hilog.batch",
        "data": {
            "subscriptionId": subscription_id,
            "connectKey": connect_key,
            "chunk": chunk,
            "dropped": dropped
        }
    });

    outbound_tx
        .send(host_message(next_message_id("event"), "event", payload))
        .await
        .map_err(|_| ())
}

async fn flush_hilog_batches(
    batcher: &mut HilogBatcher,
    outbound_tx: &mpsc::Sender<Envelope>,
    subscription_id: &str,
    connect_key: &str,
) -> Result<(), ()> {
    while let Some((chunk, dropped)) = batcher.next_batch() {
        if chunk.is_empty() && dropped == 0 {
            break;
        }

        if dropped > 0 {
            println!(
                "hilog stream {} ({}) dropped {} buffered lines",
                subscription_id, connect_key, dropped
            );
        }

        emit_hilog_batch(outbound_tx, subscription_id, connect_key, chunk, dropped).await?;
    }

    Ok(())
}

async fn run_hilog_worker(
    mut hilog: hdckit_rs::HilogStream,
    subscription_id: String,
    connect_key: String,
    outbound_tx: mpsc::Sender<Envelope>,
    mut stop_rx: oneshot::Receiver<()>,
) {
    println!(
        "hilog stream {} started for {}",
        subscription_id, connect_key
    );

    if emit_hilog_state(
        &outbound_tx,
        &subscription_id,
        &connect_key,
        "started",
        None,
    )
    .await
    .is_err()
    {
        hilog.end();
        return;
    }

    let mut ticker = interval(Duration::from_millis(BATCH_INTERVAL_MS));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut batcher = HilogBatcher::default();

    loop {
        tokio::select! {
            _ = &mut stop_rx => {
                break;
            }
            _ = ticker.tick() => {
                if flush_hilog_batches(&mut batcher, &outbound_tx, &subscription_id, &connect_key).await.is_err() {
                    hilog.end();
                    return;
                }
            }
            next = hilog.next_entry() => {
                match next {
                    Some(Ok(entry)) => {
                        batcher.push_line(format_hilog_entry(&entry));

                        if batcher.should_flush_early()
                            && flush_hilog_batches(&mut batcher, &outbound_tx, &subscription_id, &connect_key).await.is_err()
                        {
                            hilog.end();
                            return;
                        }
                    }
                    Some(Err(error)) => {
                        let _ = flush_hilog_batches(&mut batcher, &outbound_tx, &subscription_id, &connect_key).await;
                        let _ = emit_hilog_state(
                            &outbound_tx,
                            &subscription_id,
                            &connect_key,
                            "error",
                            Some(&error.to_string()),
                        )
                        .await;
                        hilog.end();
                        println!(
                            "hilog stream {} failed for {}: {}",
                            subscription_id, connect_key, error
                        );
                        return;
                    }
                    None => {
                        break;
                    }
                }
            }
        }
    }

    hilog.end();

    let _ = flush_hilog_batches(&mut batcher, &outbound_tx, &subscription_id, &connect_key).await;
    let _ = emit_hilog_state(
        &outbound_tx,
        &subscription_id,
        &connect_key,
        "stopped",
        None,
    )
    .await;

    println!(
        "hilog stream {} stopped for {}",
        subscription_id, connect_key
    );
}

async fn handle_hilog_list_pids(id: String, args: Value) -> Envelope {
    let connect_key = match required_string_arg(&args, "connectKey") {
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

    let client = match build_hdc_client_from_config() {
        Ok(value) => value,
        Err(message) => return hdc_bin_error(id, message),
    };

    let target = match client.get_target(connect_key) {
        Ok(target) => target,
        Err(error) => return hdc_error(id, error.to_string()),
    };

    match target.shell("ps -efT").await {
        Ok(mut session) => {
            let output = session.read_all_string().await;
            let _ = session.end().await;

            match output {
                Ok(output) => {
                    let pids = parse_hidumper_processes(&output);
                    host_message(
                        id,
                        "event",
                        json!({
                          "name": "hdc.hilog.listPids.result",
                          "data": {
                            "pids": pids
                          }
                        }),
                    )
                }
                Err(error) => hdc_error(id, error.to_string()),
            }
        }
        Err(error) => hdc_error(id, error.to_string()),
    }
}

async fn handle_hilog_subscribe(id: String, args: Value, session: &mut ClientSession) -> Envelope {
    let connect_key = match required_string_arg(&args, "connectKey") {
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

    let level = match optional_string_arg(&args, "level") {
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

    let pid = match optional_positive_i64_arg(&args, "pid") {
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

    let _ = session.stop_active_hilog().await;

    let client = match build_hdc_client_from_config() {
        Ok(value) => value,
        Err(message) => return hdc_bin_error(id, message),
    };

    let target = match client.get_target(connect_key.clone()) {
        Ok(target) => target,
        Err(error) => return hdc_error(id, error.to_string()),
    };

    let hilog = match target
        .open_hilog_with_filters(false, level.as_deref(), pid)
        .await
    {
        Ok(stream) => stream,
        Err(error) => return hdc_error(id, error.to_string()),
    };

    let subscription_id = next_message_id("hilog");
    let (stop_sender, stop_receiver) = oneshot::channel();
    let task_handle = tokio::spawn(run_hilog_worker(
        hilog,
        subscription_id.clone(),
        connect_key.clone(),
        session.outbound_tx.clone(),
        stop_receiver,
    ));

    session.active_hilog = Some(ActiveHilogSubscription {
        subscription_id: subscription_id.clone(),
        connect_key: connect_key.clone(),
        stop_sender: Some(stop_sender),
        task_handle,
    });

    host_message(
        id,
        "event",
        json!({
            "name": "hdc.hilog.subscribe.result",
            "data": {
                "subscriptionId": subscription_id,
                "connectKey": connect_key
            }
        }),
    )
}

async fn handle_hilog_unsubscribe(
    id: String,
    args: Value,
    session: &mut ClientSession,
) -> Envelope {
    let requested_subscription_id = match optional_string_arg(&args, "subscriptionId") {
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

    let stopped_subscription_id = session
        .stop_active_hilog_matching(requested_subscription_id.as_deref())
        .await;

    host_message(
        id,
        "event",
        json!({
            "name": "hdc.hilog.unsubscribe.result",
            "data": {
                "stopped": stopped_subscription_id.is_some(),
                "subscriptionId": stopped_subscription_id.or(requested_subscription_id)
            }
        }),
    )
}

async fn handle_invoke(id: String, payload: Value, session: &mut ClientSession) -> Envelope {
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

    match invoke.action.as_str() {
        "host.getCapabilities" => host_message(
            id,
            "event",
            json!({
              "name": "host.getCapabilities.result",
              "data": {
                "capabilities": {
                  "host.getCapabilities": true,
                  "hdc.listTargets": true,
                  "hdc.getParameters": true,
                  "hdc.shell": true,
                  "hdc.getBinConfig": true,
                  "hdc.setBinPath": true,
                  "hdc.hilog.listPids": true,
                  "hdc.hilog.subscribe": true,
                  "hdc.hilog.unsubscribe": true
                }
              }
            }),
        ),
        "hdc.getBinConfig" => host_message(
            id,
            "event",
            json!({
              "name": "hdc.getBinConfig.result",
              "data": get_bin_config()
            }),
        ),
        "hdc.setBinPath" => {
            let bin_path = match required_nullable_string_arg(&invoke.args, "binPath") {
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

            match set_custom_bin_path(bin_path) {
                Ok(config) => host_message(
                    id,
                    "event",
                    json!({
                      "name": "hdc.setBinPath.result",
                      "data": config
                    }),
                ),
                Err(error) => hdc_error(id, error),
            }
        }
        "hdc.listTargets" => {
            let client = match build_hdc_client_from_config() {
                Ok(value) => value,
                Err(message) => return hdc_bin_error(id, message),
            };

            match client.list_targets().await {
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
            }
        }
        "hdc.getParameters" => {
            let client = match build_hdc_client_from_config() {
                Ok(value) => value,
                Err(message) => return hdc_bin_error(id, message),
            };

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
            let client = match build_hdc_client_from_config() {
                Ok(value) => value,
                Err(message) => return hdc_bin_error(id, message),
            };

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
        "hdc.hilog.listPids" => handle_hilog_list_pids(id, invoke.args).await,
        "hdc.hilog.subscribe" => handle_hilog_subscribe(id, invoke.args, session).await,
        "hdc.hilog.unsubscribe" => handle_hilog_unsubscribe(id, invoke.args, session).await,
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
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Envelope>(OUTBOUND_QUEUE_CAPACITY);

    let writer_task = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            let payload = match serde_json::to_string(&message) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("failed to serialize outbound message: {error}");
                    continue;
                }
            };

            if write.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });

    let mut session = ClientSession::new(outbound_tx.clone());

    while let Some(next) = read.next().await {
        let Ok(message) = next else {
            break;
        };

        let response = match message {
            Message::Text(text) => {
                let parsed = serde_json::from_str::<Envelope>(&text);

                match parsed {
                    Ok(incoming) if incoming.kind == "invoke" => {
                        handle_invoke(incoming.id, incoming.payload, &mut session).await
                    }
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
                }
            }
            Message::Close(_) => {
                break;
            }
            _ => {
                continue;
            }
        };

        if session.outbound_tx.send(response).await.is_err() {
            break;
        }
    }

    let _ = session.stop_active_hilog().await;

    drop(session);
    drop(outbound_tx);
    let _ = writer_task.await;
}

pub async fn run_bridge(ws_addr: &str) -> Result<(), String> {
    let listener = TcpListener::bind(ws_addr)
        .await
        .map_err(|error| format!("failed to bind websocket bridge ({ws_addr}): {error}"))?;

    println!("Harmony websocket bridge listening on ws://{ws_addr}");

    loop {
        let accepted = listener.accept().await;
        let (stream, _) = match accepted {
            Ok(value) => value,
            Err(error) => {
                eprintln!("accept error: {error}");
                continue;
            }
        };

        tokio::spawn(handle_client(stream));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_hilog_entry, kind_to_char, level_to_ansi, level_to_char, optional_positive_i64_arg,
        optional_string_arg, parse_hidumper_processes, HilogBatcher, HilogPidOption,
        BATCH_MAX_BYTES, BATCH_MAX_LINES, QUEUE_MAX_LINES,
    };
    use hdckit_rs::HilogEntry;
    use serde_json::json;
    use std::time::{Duration, UNIX_EPOCH};

    fn build_entry(message: &str) -> HilogEntry {
        HilogEntry {
            date: UNIX_EPOCH + Duration::from_secs(123),
            pid: 100,
            tid: 101,
            level: 4,
            kind: 1,
            domain: "0ABC1".to_string(),
            tag: "TAG".to_string(),
            message: message.to_string(),
        }
    }

    #[test]
    fn batcher_flushes_by_line_threshold() {
        let mut batcher = HilogBatcher::default();

        for idx in 0..BATCH_MAX_LINES {
            batcher.push_line(format!("line-{idx}\n"));
        }

        assert!(batcher.should_flush_early());

        let (chunk, dropped) = batcher.next_batch().expect("expected batch");
        assert_eq!(dropped, 0);
        assert!(chunk.contains("line-0"));
        assert!(chunk.contains(&format!("line-{}", BATCH_MAX_LINES - 1)));
    }

    #[test]
    fn batcher_tracks_drop_oldest() {
        let mut batcher = HilogBatcher::default();

        for idx in 0..(QUEUE_MAX_LINES + 7) {
            batcher.push_line(format!("line-{idx}\n"));
        }

        let (chunk, dropped) = batcher.next_batch().expect("expected batch");
        assert_eq!(dropped, 7);
        assert!(!chunk.contains("line-0"));
        assert!(chunk.contains("line-7"));
    }

    #[test]
    fn batcher_flushes_by_byte_threshold() {
        let mut batcher = HilogBatcher::default();
        let line = "x".repeat((BATCH_MAX_BYTES / 2).max(1));

        batcher.push_line(format!("{line}\n"));
        batcher.push_line(format!("{line}\n"));

        assert!(batcher.should_flush_early());
        let (chunk, dropped) = batcher.next_batch().expect("expected batch");
        assert_eq!(dropped, 0);
        assert!(!chunk.is_empty());
    }

    #[test]
    fn event_payload_format_is_stable() {
        let payload = json!({
            "name": "hdc.hilog.batch",
            "data": {
                "subscriptionId": "sub-1",
                "connectKey": "device-1",
                "chunk": "hello\n",
                "dropped": 3
            }
        });

        assert_eq!(payload["name"], "hdc.hilog.batch");
        assert_eq!(payload["data"]["subscriptionId"], "sub-1");
        assert_eq!(payload["data"]["connectKey"], "device-1");
        assert_eq!(payload["data"]["chunk"], "hello\n");
        assert_eq!(payload["data"]["dropped"], 3);
    }

    #[test]
    fn format_hilog_entry_matches_wire_text() {
        let entry = build_entry("hello");
        let line = format_hilog_entry(&entry);

        assert!(line.contains("123.000"));
        assert!(line.contains("100 101 \x1b[32mI\x1b[0m I0ABC1/TAG: hello"));
    }

    #[test]
    fn level_and_kind_mapping_fallbacks() {
        assert_eq!(level_to_char(4), 'I');
        assert_eq!(level_to_char(-1), '?');
        assert_eq!(kind_to_char(1), 'I');
        assert_eq!(kind_to_char(99), '?');
    }

    #[test]
    fn level_to_ansi_mapping_is_stable() {
        assert_eq!(level_to_ansi(2), "\x1b[90m");
        assert_eq!(level_to_ansi(3), "\x1b[36m");
        assert_eq!(level_to_ansi(4), "\x1b[32m");
        assert_eq!(level_to_ansi(5), "\x1b[33m");
        assert_eq!(level_to_ansi(6), "\x1b[31m");
        assert_eq!(level_to_ansi(7), "\x1b[1;31m");
        assert_eq!(level_to_ansi(999), "\x1b[39m");
    }

    #[test]
    fn optional_level_accepts_trimmed_string() {
        let args = json!({ "level": "  I,W,E  " });
        let parsed = optional_string_arg(&args, "level").expect("expected valid level");
        assert_eq!(parsed.as_deref(), Some("I,W,E"));
    }

    #[test]
    fn optional_level_empty_string_is_none() {
        let args = json!({ "level": "   " });
        let parsed = optional_string_arg(&args, "level").expect("expected empty to normalize");
        assert_eq!(parsed, None);
    }

    #[test]
    fn optional_level_non_string_is_invalid() {
        let args = json!({ "level": 123 });
        let error = optional_string_arg(&args, "level").expect_err("expected invalid level");
        assert_eq!(error, "`level` must be a string when provided");
    }

    #[test]
    fn optional_pid_accepts_positive_integer() {
        let args = json!({ "pid": 1234 });
        let parsed = optional_positive_i64_arg(&args, "pid").expect("expected valid pid");
        assert_eq!(parsed, Some(1234));
    }

    #[test]
    fn optional_pid_rejects_invalid_values() {
        let args = json!({ "pid": 0 });
        let error = optional_positive_i64_arg(&args, "pid").expect_err("expected invalid pid");
        assert_eq!(error, "`pid` must be a positive integer when provided");

        let args = json!({ "pid": -1 });
        let error = optional_positive_i64_arg(&args, "pid").expect_err("expected invalid pid");
        assert_eq!(error, "`pid` must be a positive integer when provided");

        let args = json!({ "pid": "123" });
        let error = optional_positive_i64_arg(&args, "pid").expect_err("expected invalid pid");
        assert_eq!(error, "`pid` must be a positive integer when provided");
    }

    #[test]
    fn parse_hidumper_processes_filters_threads_and_dedupes() {
        let output = r#"
UID        PID   TID   PPID  C STIME TTY          TIME CMD
root         1     1      0  0 00:00 ?        00:00:01 /init
root       200   200      1  0 00:00 ?        00:00:02 system_server
root       200   201      1  0 00:00 ?        00:00:00 Binder:200_1
shell      300   300      1  0 00:00 ?        00:00:00 com.demo.app
shell      300   300      1  0 00:00 ?        00:00:00 com.demo.app.dup
invalid line without columns
"#;

        let parsed = parse_hidumper_processes(output);
        assert_eq!(
            parsed,
            vec![
                HilogPidOption {
                    pid: 1,
                    command: "/init".to_string()
                },
                HilogPidOption {
                    pid: 200,
                    command: "system_server".to_string()
                },
                HilogPidOption {
                    pid: 300,
                    command: "com.demo.app".to_string()
                }
            ]
        );
    }

    #[test]
    fn parse_ps_ef_processes_without_tid_column() {
        let output = r#"
UID            PID  PPID C STIME TTY          TIME CMD
root             1     0 0 18:01:44 ?     00:00:12 init --second-stage 1714884
root            63     1 0 18:01:44 ?     00:00:14 crypto.elf hongmeng
invalid row
"#;

        let parsed = parse_hidumper_processes(output);
        assert_eq!(
            parsed,
            vec![
                HilogPidOption {
                    pid: 1,
                    command: "init --second-stage 1714884".to_string()
                },
                HilogPidOption {
                    pid: 63,
                    command: "crypto.elf hongmeng".to_string()
                }
            ]
        );
    }
}
