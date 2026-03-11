use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use hdckit_rs::HilogEntry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::fs as tokio_fs;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, MissedTickBehavior};
use tokio_tungstenite::{accept_async, tungstenite::Message};

mod emulator;
mod hdc_bin;
mod mcp;

use emulator::{
    create_device as emulator_create_device, delete_device as emulator_delete_device,
    download_image as emulator_download_image, get_create_device_options as emulator_get_create_device_options,
    get_environment as emulator_get_environment, list_devices as emulator_list_devices,
    list_images as emulator_list_images, start_device as emulator_start_device,
    stop_device as emulator_stop_device, EmulatorCreateDeviceArgs, EmulatorSessionState,
    list_download_jobs as emulator_list_download_jobs,
};
use hdc_bin::{build_hdc_client_from_config, get_bin_config, set_custom_bin_path};
use mcp::{list_builtin_mcp_tools, run_mcp_http_server};

pub const DEFAULT_WS_ADDR: &str = "127.0.0.1:8787";

const OUTBOUND_QUEUE_CAPACITY: usize = 128;
const QUEUE_MAX_LINES: usize = 4_000;
const BATCH_INTERVAL_MS: u64 = 40;
const BATCH_MAX_LINES: usize = 200;
const BATCH_MAX_BYTES: usize = 64 * 1024;
const DEFAULT_MCP_PORT_OFFSET: u16 = 100;
const FS_LIST_EXIT_SENTINEL_PREFIX: &str = "__HARMONY_FS_EXIT:";
const FS_DELETE_EXIT_SENTINEL_PREFIX: &str = "__HARMONY_FS_DELETE_EXIT:";
const FS_DOWNLOAD_TEMP_MAX_BYTES_DEFAULT: u64 = 10 * 1024 * 1024;

static NEXT_MESSAGE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Envelope {
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HdcFsListEntry {
    path: String,
    name: String,
    kind: String,
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
    emulator_session: EmulatorSessionState,
}

impl ClientSession {
    fn new(outbound_tx: mpsc::Sender<Envelope>) -> Self {
        Self {
            outbound_tx,
            active_hilog: None,
            emulator_session: EmulatorSessionState::default(),
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

pub(crate) fn next_message_id(prefix: &str) -> String {
    let sequence = NEXT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{sequence}", now_ms())
}

pub fn derive_default_mcp_http_addr(ws_addr: &str) -> Result<String, String> {
    let trimmed = ws_addr.trim();
    if trimmed.is_empty() {
        return Err("websocket address must be a non-empty string".to_string());
    }

    let (host_raw, ws_port_raw) = trimmed
        .rsplit_once(':')
        .ok_or_else(|| format!("invalid websocket address `{trimmed}`: expected <host>:<port>"))?;

    let host = host_raw.trim();
    if host.is_empty() {
        return Err(format!(
            "invalid websocket address `{trimmed}`: host must be non-empty"
        ));
    }

    let ws_port = ws_port_raw.parse::<u16>().map_err(|_| {
        format!("invalid websocket address `{trimmed}`: port must be a u16 integer")
    })?;

    let mcp_port = ws_port
        .checked_add(DEFAULT_MCP_PORT_OFFSET)
        .ok_or_else(|| {
            format!(
                "cannot derive MCP HTTP address from websocket address `{trimmed}`: port overflow"
            )
        })?;

    Ok(format!("{host}:{mcp_port}"))
}

pub(crate) fn host_message(id: String, kind: &str, payload: Value) -> Envelope {
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

fn required_positive_u32_arg(args: &Value, key: &str) -> Result<u32, String> {
    let value = args
        .get(key)
        .ok_or_else(|| format!("`{key}` is required"))?;

    let Some(raw) = value.as_i64() else {
        return Err(format!("`{key}` must be a positive integer"));
    };

    if raw <= 0 || raw > u32::MAX as i64 {
        return Err(format!("`{key}` must be a positive integer"));
    }

    Ok(raw as u32)
}

fn optional_bool_arg(args: &Value, key: &str) -> Result<Option<bool>, String> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };

    if value.is_null() {
        return Ok(None);
    }

    let Some(raw) = value.as_bool() else {
        return Err(format!("`{key}` must be a boolean when provided"));
    };

    Ok(Some(raw))
}

fn required_absolute_path_arg(args: &Value, key: &str) -> Result<String, String> {
    let raw = args
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`{key}` must be a non-empty string"))?;

    if raw.contains('\n') || raw.contains('\r') {
        return Err(format!("`{key}` must not contain newline characters"));
    }

    let path = raw.trim();
    if path.is_empty() {
        return Err(format!("`{key}` must be a non-empty string"));
    }

    if !path.starts_with('/') {
        return Err(format!(
            "`{key}` must be an absolute path starting with `/`"
        ));
    }

    Ok(path.to_string())
}

fn required_absolute_local_path_arg(args: &Value, key: &str) -> Result<String, String> {
    let raw = args
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`{key}` must be a non-empty string"))?;

    if raw.contains('\n') || raw.contains('\r') {
        return Err(format!("`{key}` must not contain newline characters"));
    }

    let path = raw.trim();
    if path.is_empty() {
        return Err(format!("`{key}` must be a non-empty string"));
    }

    if !Path::new(path).is_absolute() {
        return Err(format!("`{key}` must be an absolute local filesystem path"));
    }

    Ok(path.to_string())
}

fn shell_single_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn build_fs_list_shell_command(path: &str, include_hidden: bool) -> String {
    let flags = if include_hidden { "-A1p" } else { "-1p" };
    let escaped_path = shell_single_quote(path);
    format!("ls {flags} -- {escaped_path} 2>&1; echo {FS_LIST_EXIT_SENTINEL_PREFIX}$?")
}

fn build_fs_delete_shell_command(path: &str) -> String {
    let escaped_path = shell_single_quote(path);
    format!("rm -rf -- {escaped_path} 2>&1; echo {FS_DELETE_EXIT_SENTINEL_PREFIX}$?")
}

fn join_device_path(parent: &str, name: &str) -> String {
    let trimmed_name = name.trim_start_matches('/');
    if parent == "/" {
        return format!("/{trimmed_name}");
    }

    format!("{}/{}", parent.trim_end_matches('/'), trimmed_name)
}

fn local_path_basename(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
}

fn device_path_basename(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        return None;
    }

    trimmed
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "." && *value != "..")
        .map(ToString::to_string)
}

fn build_fs_upload_remote_path(remote_directory: &str, local_path: &str) -> Result<String, String> {
    let file_name = local_path_basename(local_path)
        .ok_or_else(|| "`localPath` must include a file name".to_string())?;
    Ok(join_device_path(remote_directory, &file_name))
}

fn build_fs_download_local_path(
    local_directory: &str,
    remote_path: &str,
) -> Result<PathBuf, String> {
    let file_name = device_path_basename(remote_path)
        .ok_or_else(|| "`remotePath` must include a file name".to_string())?;
    Ok(Path::new(local_directory).join(file_name))
}

fn fs_download_temp_root_dir() -> PathBuf {
    std::env::temp_dir()
        .join("harmony-dev-helper")
        .join("open-in-editor")
}

fn build_fs_download_temp_local_path(remote_path: &str) -> Result<PathBuf, String> {
    let file_name = device_path_basename(remote_path)
        .ok_or_else(|| "`remotePath` must include a file name".to_string())?;
    let unique_id = next_message_id("fs-download-temp");
    Ok(fs_download_temp_root_dir().join(format!("{unique_id}-{file_name}")))
}

fn ensure_fs_download_temp_within_limit(byte_length: u64, max_bytes: u64) -> Result<(), String> {
    if byte_length > max_bytes {
        return Err(format!(
            "File is too large to open in editor ({byte_length} bytes > {max_bytes} bytes limit)"
        ));
    }

    Ok(())
}

fn ensure_fs_download_temp_utf8(bytes: &[u8]) -> Result<(), String> {
    std::str::from_utf8(bytes)
        .map(|_| ())
        .map_err(|_| "Only UTF-8 text files are supported for Open in Editor".to_string())
}

async fn remove_temp_file_if_exists(path: &Path) {
    let _ = tokio_fs::remove_file(path).await;
}

fn parse_fs_list_output(parent_path: &str, output: &str) -> Result<Vec<HdcFsListEntry>, String> {
    let lines = output.lines().collect::<Vec<_>>();
    let sentinel_index = lines
        .iter()
        .rposition(|line| line.trim_end().starts_with(FS_LIST_EXIT_SENTINEL_PREFIX))
        .ok_or_else(|| {
            "failed to parse filesystem listing result (missing exit sentinel)".to_string()
        })?;

    let sentinel_line = lines[sentinel_index].trim_end();
    let exit_code_raw = sentinel_line
        .strip_prefix(FS_LIST_EXIT_SENTINEL_PREFIX)
        .ok_or_else(|| {
            "failed to parse filesystem listing result (invalid exit sentinel)".to_string()
        })?;
    let exit_code = exit_code_raw
        .parse::<i32>()
        .map_err(|_| "failed to parse filesystem listing result (invalid exit code)".to_string())?;

    let listing_lines = lines[..sentinel_index]
        .iter()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();

    if exit_code != 0 {
        let message = listing_lines.join("\n").trim().to_string();
        if message.is_empty() {
            return Err(format!(
                "failed to list `{parent_path}` (exit code {exit_code})"
            ));
        }

        return Err(message);
    }

    let mut entries = Vec::new();
    for line in listing_lines {
        if line.is_empty() {
            continue;
        }

        let (name, kind) = if let Some(name) = line.strip_suffix('/') {
            (name, "directory")
        } else {
            (line, "file")
        };

        if name.is_empty() || name == "." || name == ".." {
            continue;
        }

        entries.push(HdcFsListEntry {
            path: join_device_path(parent_path, name),
            name: name.to_string(),
            kind: kind.to_string(),
        });
    }

    Ok(entries)
}

fn parse_fs_delete_output(target_path: &str, output: &str) -> Result<(), String> {
    let lines = output.lines().collect::<Vec<_>>();
    let sentinel_index = lines
        .iter()
        .rposition(|line| line.trim_end().starts_with(FS_DELETE_EXIT_SENTINEL_PREFIX))
        .ok_or_else(|| {
            "failed to parse filesystem delete result (missing exit sentinel)".to_string()
        })?;

    let sentinel_line = lines[sentinel_index].trim_end();
    let exit_code_raw = sentinel_line
        .strip_prefix(FS_DELETE_EXIT_SENTINEL_PREFIX)
        .ok_or_else(|| {
            "failed to parse filesystem delete result (invalid exit sentinel)".to_string()
        })?;
    let exit_code = exit_code_raw
        .parse::<i32>()
        .map_err(|_| "failed to parse filesystem delete result (invalid exit code)".to_string())?;

    let shell_lines = lines[..sentinel_index]
        .iter()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();

    if exit_code != 0 {
        let message = shell_lines.join("\n").trim().to_string();
        if message.is_empty() {
            return Err(format!(
                "failed to delete `{target_path}` (exit code {exit_code})"
            ));
        }

        return Err(message);
    }

    Ok(())
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

fn emulator_error(id: String, message: String) -> Envelope {
    host_message(
        id,
        "error",
        json!({
          "code": "EMULATOR_ERROR",
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

async fn handle_fs_list(id: String, args: Value) -> Envelope {
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

    let path = match required_absolute_path_arg(&args, "path") {
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

    let include_hidden = match optional_bool_arg(&args, "includeHidden") {
        Ok(value) => value.unwrap_or(true),
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

    let command = build_fs_list_shell_command(&path, include_hidden);

    match client.get_target(connect_key) {
        Ok(target) => match target.shell(&command).await {
            Ok(mut session) => match session.read_all_string().await {
                Ok(output) => {
                    let _ = session.end().await;
                    match parse_fs_list_output(&path, &output) {
                        Ok(entries) => host_message(
                            id,
                            "event",
                            json!({
                              "name": "hdc.fs.list.result",
                              "data": {
                                "entries": entries
                              }
                            }),
                        ),
                        Err(message) => hdc_error(id, message),
                    }
                }
                Err(error) => hdc_error(id, error.to_string()),
            },
            Err(error) => hdc_error(id, error.to_string()),
        },
        Err(error) => hdc_error(id, error.to_string()),
    }
}

async fn handle_fs_upload(id: String, args: Value) -> Envelope {
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

    let local_path = match required_absolute_local_path_arg(&args, "localPath") {
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

    let remote_directory = match required_absolute_path_arg(&args, "remoteDirectory") {
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

    let remote_path = match build_fs_upload_remote_path(&remote_directory, &local_path) {
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

    match client.get_target(connect_key) {
        Ok(target) => match target.send_file(Path::new(&local_path), &remote_path).await {
            Ok(()) => host_message(
                id,
                "event",
                json!({
                  "name": "hdc.fs.upload.result",
                  "data": {
                    "remotePath": remote_path
                  }
                }),
            ),
            Err(error) => hdc_error(id, error.to_string()),
        },
        Err(error) => hdc_error(id, error.to_string()),
    }
}

async fn handle_fs_download(id: String, args: Value) -> Envelope {
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

    let remote_path = match required_absolute_path_arg(&args, "remotePath") {
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

    let local_directory = match required_absolute_local_path_arg(&args, "localDirectory") {
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

    let local_path = match build_fs_download_local_path(&local_directory, &remote_path) {
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

    match client.get_target(connect_key) {
        Ok(target) => match target.recv_file(&remote_path, &local_path).await {
            Ok(()) => host_message(
                id,
                "event",
                json!({
                  "name": "hdc.fs.download.result",
                  "data": {
                    "localPath": local_path.to_string_lossy().to_string()
                  }
                }),
            ),
            Err(error) => hdc_error(id, error.to_string()),
        },
        Err(error) => hdc_error(id, error.to_string()),
    }
}

async fn handle_fs_download_temp(id: String, args: Value) -> Envelope {
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

    let remote_path = match required_absolute_path_arg(&args, "remotePath") {
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

    let max_bytes = match optional_positive_i64_arg(&args, "maxBytes") {
        Ok(value) => value
            .map(|raw| raw as u64)
            .unwrap_or(FS_DOWNLOAD_TEMP_MAX_BYTES_DEFAULT),
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

    let local_path = match build_fs_download_temp_local_path(&remote_path) {
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

    if let Some(parent_dir) = local_path.parent() {
        if let Err(error) = tokio_fs::create_dir_all(parent_dir).await {
            return hdc_error(
                id,
                format!(
                    "failed to prepare temporary directory for Open in Editor ({}): {error}",
                    parent_dir.to_string_lossy()
                ),
            );
        }
    }

    let client = match build_hdc_client_from_config() {
        Ok(value) => value,
        Err(message) => return hdc_bin_error(id, message),
    };

    match client.get_target(connect_key) {
        Ok(target) => match target.recv_file(&remote_path, &local_path).await {
            Ok(()) => {
                let metadata = match tokio_fs::metadata(&local_path).await {
                    Ok(value) => value,
                    Err(error) => {
                        remove_temp_file_if_exists(&local_path).await;
                        return hdc_error(
                            id,
                            format!(
                                "failed to inspect downloaded temporary file ({}): {error}",
                                local_path.to_string_lossy()
                            ),
                        );
                    }
                };

                if let Err(message) =
                    ensure_fs_download_temp_within_limit(metadata.len(), max_bytes)
                {
                    remove_temp_file_if_exists(&local_path).await;
                    return hdc_error(id, message);
                }

                let bytes = match tokio_fs::read(&local_path).await {
                    Ok(value) => value,
                    Err(error) => {
                        remove_temp_file_if_exists(&local_path).await;
                        return hdc_error(
                            id,
                            format!(
                                "failed to read downloaded temporary file ({}): {error}",
                                local_path.to_string_lossy()
                            ),
                        );
                    }
                };

                if let Err(message) =
                    ensure_fs_download_temp_within_limit(bytes.len() as u64, max_bytes)
                {
                    remove_temp_file_if_exists(&local_path).await;
                    return hdc_error(id, message);
                }

                if let Err(message) = ensure_fs_download_temp_utf8(&bytes) {
                    remove_temp_file_if_exists(&local_path).await;
                    return hdc_error(id, message);
                }

                host_message(
                    id,
                    "event",
                    json!({
                      "name": "hdc.fs.downloadTemp.result",
                      "data": {
                        "localPath": local_path.to_string_lossy().to_string(),
                        "byteLength": bytes.len()
                      }
                    }),
                )
            }
            Err(error) => {
                remove_temp_file_if_exists(&local_path).await;
                hdc_error(id, error.to_string())
            }
        },
        Err(error) => hdc_error(id, error.to_string()),
    }
}

async fn handle_fs_delete(id: String, args: Value) -> Envelope {
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

    let path = match required_absolute_path_arg(&args, "path") {
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

    if path == "/" {
        return host_message(
            id,
            "error",
            json!({
              "code": "INVALID_ARGS",
              "message": "`path` must not be root (`/`)"
            }),
        );
    }

    let client = match build_hdc_client_from_config() {
        Ok(value) => value,
        Err(message) => return hdc_bin_error(id, message),
    };

    let command = build_fs_delete_shell_command(&path);

    match client.get_target(connect_key) {
        Ok(target) => match target.shell(&command).await {
            Ok(mut session) => match session.read_all_string().await {
                Ok(output) => {
                    let _ = session.end().await;
                    match parse_fs_delete_output(&path, &output) {
                        Ok(()) => host_message(
                            id,
                            "event",
                            json!({
                              "name": "hdc.fs.delete.result",
                              "data": {
                                "deletedPath": path
                              }
                            }),
                        ),
                        Err(message) => hdc_error(id, message),
                    }
                }
                Err(error) => hdc_error(id, error.to_string()),
            },
            Err(error) => hdc_error(id, error.to_string()),
        },
        Err(error) => hdc_error(id, error.to_string()),
    }
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
                  "mcp.listTools": true,
                  "hdc.listTargets": true,
                  "hdc.getParameters": true,
                  "hdc.shell": true,
                  "hdc.fs.list": true,
                  "hdc.fs.upload": true,
                  "hdc.fs.download": true,
                  "hdc.fs.downloadTemp": true,
                  "hdc.fs.delete": true,
                  "hdc.getBinConfig": true,
                  "hdc.setBinPath": true,
                  "hdc.hilog.listPids": true,
                  "hdc.hilog.subscribe": true,
                  "hdc.hilog.unsubscribe": true,
                  "emulator.getEnvironment": true,
                  "emulator.listImages": true,
                  "emulator.listDownloadJobs": true,
                  "emulator.getCreateDeviceOptions": true,
                  "emulator.downloadImage": true,
                  "emulator.listDevices": true,
                  "emulator.createDevice": true,
                  "emulator.startDevice": true,
                  "emulator.stopDevice": true,
                  "emulator.deleteDevice": true
                }
              }
            }),
        ),
        "mcp.listTools" => host_message(
            id,
            "event",
            json!({
              "name": "mcp.listTools.result",
              "data": {
                "tools": list_builtin_mcp_tools()
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
        "hdc.fs.list" => handle_fs_list(id, invoke.args).await,
        "hdc.fs.upload" => handle_fs_upload(id, invoke.args).await,
        "hdc.fs.download" => handle_fs_download(id, invoke.args).await,
        "hdc.fs.downloadTemp" => handle_fs_download_temp(id, invoke.args).await,
        "hdc.fs.delete" => handle_fs_delete(id, invoke.args).await,
        "hdc.hilog.subscribe" => handle_hilog_subscribe(id, invoke.args, session).await,
        "hdc.hilog.unsubscribe" => handle_hilog_unsubscribe(id, invoke.args, session).await,
        "emulator.getEnvironment" => match emulator_get_environment().await {
            Ok(environment) => host_message(
                id,
                "event",
                json!({
                    "name": "emulator.getEnvironment.result",
                    "data": environment
                }),
            ),
            Err(message) => emulator_error(id, message),
        },
        "emulator.listImages" => match emulator_list_images(&session.emulator_session).await {
            Ok(images) => host_message(
                id,
                "event",
                json!({
                    "name": "emulator.listImages.result",
                    "data": {
                        "images": images
                    }
                }),
            ),
            Err(message) => emulator_error(id, message),
        },
        "emulator.listDownloadJobs" => host_message(
            id,
            "event",
            json!({
                "name": "emulator.listDownloadJobs.result",
                "data": {
                    "jobs": emulator_list_download_jobs(&session.emulator_session).await
                }
            }),
        ),
        "emulator.getCreateDeviceOptions" => {
            let relative_path = match required_string_arg(&invoke.args, "relativePath") {
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

            match emulator_get_create_device_options(&relative_path).await {
                Ok(options) => host_message(
                    id,
                    "event",
                    json!({
                        "name": "emulator.getCreateDeviceOptions.result",
                        "data": options
                    }),
                ),
                Err(message) => emulator_error(id, message),
            }
        }
        "emulator.downloadImage" => {
            let relative_path = match required_string_arg(&invoke.args, "relativePath") {
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

            match emulator_download_image(
                &relative_path,
                &session.emulator_session,
                session.outbound_tx.clone(),
            )
            .await
            {
                Ok(job_id) => host_message(
                    id,
                    "event",
                    json!({
                        "name": "emulator.downloadImage.result",
                        "data": {
                            "jobId": job_id
                        }
                    }),
                ),
                Err(message) => emulator_error(id, message),
            }
        }
        "emulator.listDevices" => match emulator_list_devices().await {
            Ok(devices) => host_message(
                id,
                "event",
                json!({
                    "name": "emulator.listDevices.result",
                    "data": {
                        "devices": devices
                    }
                }),
            ),
            Err(message) => emulator_error(id, message),
        },
        "emulator.createDevice" => {
            let relative_path = match required_string_arg(&invoke.args, "relativePath") {
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
            let product_device_type = match required_string_arg(&invoke.args, "productDeviceType") {
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
            let product_name = match required_string_arg(&invoke.args, "productName") {
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
            let name = match required_string_arg(&invoke.args, "name") {
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
            let cpu_cores = match required_positive_u32_arg(&invoke.args, "cpuCores") {
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
            let memory_ram_mb = match required_positive_u32_arg(&invoke.args, "memoryRamMb") {
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
            let data_disk_mb = match required_positive_u32_arg(&invoke.args, "dataDiskMb") {
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
            let vendor_country = match optional_string_arg(&invoke.args, "vendorCountry") {
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
            let is_public = match optional_bool_arg(&invoke.args, "isPublic") {
                Ok(value) => value.unwrap_or(true),
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

            match emulator_create_device(EmulatorCreateDeviceArgs {
                relative_path,
                product_device_type,
                product_name,
                name,
                cpu_cores,
                memory_ram_mb,
                data_disk_mb,
                vendor_country,
                is_public,
            })
            .await
            {
                Ok(device) => host_message(
                    id,
                    "event",
                    json!({
                        "name": "emulator.createDevice.result",
                        "data": {
                            "device": device
                        }
                    }),
                ),
                Err(message) => emulator_error(id, message),
            }
        }
        "emulator.startDevice" => {
            let name = match required_string_arg(&invoke.args, "name") {
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

            match emulator_start_device(&name).await {
                Ok(name) => host_message(
                    id,
                    "event",
                    json!({
                        "name": "emulator.startDevice.result",
                        "data": {
                            "name": name
                        }
                    }),
                ),
                Err(message) => emulator_error(id, message),
            }
        }
        "emulator.stopDevice" => {
            let name = match required_string_arg(&invoke.args, "name") {
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

            match emulator_stop_device(&name).await {
                Ok(name) => host_message(
                    id,
                    "event",
                    json!({
                        "name": "emulator.stopDevice.result",
                        "data": {
                            "name": name
                        }
                    }),
                ),
                Err(message) => emulator_error(id, message),
            }
        }
        "emulator.deleteDevice" => {
            let name = match required_string_arg(&invoke.args, "name") {
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

            match emulator_delete_device(&name).await {
                Ok(name) => host_message(
                    id,
                    "event",
                    json!({
                        "name": "emulator.deleteDevice.result",
                        "data": {
                            "name": name
                        }
                    }),
                ),
                Err(message) => emulator_error(id, message),
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

async fn run_websocket_bridge(ws_addr: &str) -> Result<(), String> {
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

pub async fn run_bridge_with_mcp(ws_addr: &str, mcp_http_addr: Option<&str>) -> Result<(), String> {
    let resolved_mcp_http_addr = match mcp_http_addr {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err("mcp http address must be a non-empty string".to_string());
            }
            trimmed.to_string()
        }
        None => derive_default_mcp_http_addr(ws_addr)?,
    };

    tokio::try_join!(
        run_websocket_bridge(ws_addr),
        run_mcp_http_server(&resolved_mcp_http_addr)
    )?;

    Ok(())
}

pub async fn run_bridge(ws_addr: &str) -> Result<(), String> {
    run_bridge_with_mcp(ws_addr, None).await
}

#[cfg(test)]
mod tests {
    use super::{
        build_fs_delete_shell_command, build_fs_download_local_path,
        build_fs_download_temp_local_path, build_fs_list_shell_command,
        build_fs_upload_remote_path, derive_default_mcp_http_addr, ensure_fs_download_temp_utf8,
        ensure_fs_download_temp_within_limit, format_hilog_entry, fs_download_temp_root_dir,
        handle_invoke, join_device_path, kind_to_char, level_to_ansi, level_to_char,
        optional_bool_arg, optional_positive_i64_arg, optional_string_arg, parse_fs_delete_output,
        parse_fs_list_output, parse_hidumper_processes, remove_temp_file_if_exists,
        required_absolute_local_path_arg, shell_single_quote, ClientSession, HdcFsListEntry,
        HilogBatcher, HilogPidOption, BATCH_MAX_BYTES, BATCH_MAX_LINES,
        FS_DELETE_EXIT_SENTINEL_PREFIX, FS_LIST_EXIT_SENTINEL_PREFIX, OUTBOUND_QUEUE_CAPACITY,
        QUEUE_MAX_LINES,
    };
    use hdckit_rs::HilogEntry;
    use serde_json::json;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tokio::sync::mpsc;

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
    fn optional_bool_accepts_and_rejects_types() {
        let args = json!({ "includeHidden": true });
        let parsed = optional_bool_arg(&args, "includeHidden").expect("valid bool");
        assert_eq!(parsed, Some(true));

        let args = json!({ "includeHidden": false });
        let parsed = optional_bool_arg(&args, "includeHidden").expect("valid bool");
        assert_eq!(parsed, Some(false));

        let args = json!({ "includeHidden": "yes" });
        let error = optional_bool_arg(&args, "includeHidden").expect_err("invalid bool");
        assert_eq!(error, "`includeHidden` must be a boolean when provided");
    }

    #[test]
    fn shell_single_quote_escapes_single_quotes() {
        assert_eq!(shell_single_quote("/data/log"), "'/data/log'");
        assert_eq!(shell_single_quote("/data/it's/log"), "'/data/it'\\''s/log'");
    }

    #[test]
    fn build_fs_list_shell_command_uses_expected_flags() {
        let hidden_cmd = build_fs_list_shell_command("/data", true);
        assert!(hidden_cmd.starts_with("ls -A1p -- '/data' 2>&1; echo "));
        assert!(hidden_cmd.contains(FS_LIST_EXIT_SENTINEL_PREFIX));

        let visible_cmd = build_fs_list_shell_command("/data", false);
        assert!(visible_cmd.starts_with("ls -1p -- '/data' 2>&1; echo "));
        assert!(visible_cmd.contains(FS_LIST_EXIT_SENTINEL_PREFIX));
    }

    #[test]
    fn build_fs_delete_shell_command_contains_sentinel() {
        let cmd = build_fs_delete_shell_command("/data/log.txt");
        assert!(cmd.starts_with("rm -rf -- '/data/log.txt' 2>&1; echo "));
        assert!(cmd.contains(FS_DELETE_EXIT_SENTINEL_PREFIX));
    }

    #[test]
    fn join_device_path_handles_root_and_nested_paths() {
        assert_eq!(join_device_path("/", "system"), "/system");
        assert_eq!(join_device_path("/data", "local"), "/data/local");
        assert_eq!(join_device_path("/data/", "local"), "/data/local");
    }

    #[test]
    fn required_absolute_local_path_arg_validates_input() {
        let absolute_path = std::env::current_dir()
            .expect("cwd should resolve")
            .join("logs")
            .display()
            .to_string();

        let args = json!({ "localPath": absolute_path });
        let parsed = required_absolute_local_path_arg(&args, "localPath")
            .expect("expected valid absolute path");
        assert_eq!(
            parsed,
            std::env::current_dir()
                .expect("cwd should resolve")
                .join("logs")
                .display()
                .to_string()
        );

        let args = json!({ "localPath": "relative/file.txt" });
        let error = required_absolute_local_path_arg(&args, "localPath")
            .expect_err("relative local path must fail");
        assert_eq!(
            error,
            "`localPath` must be an absolute local filesystem path"
        );

        let args = json!({ "localPath": format!("{absolute_path}\n") });
        let error = required_absolute_local_path_arg(&args, "localPath")
            .expect_err("path with newline must fail");
        assert_eq!(error, "`localPath` must not contain newline characters");
    }

    #[test]
    fn build_fs_upload_remote_path_uses_local_basename() {
        let remote_path = build_fs_upload_remote_path("/data/log", "/Users/demo/output.log")
            .expect("upload remote path should resolve");
        assert_eq!(remote_path, "/data/log/output.log");
    }

    #[test]
    fn build_fs_upload_remote_path_rejects_missing_local_basename() {
        let error = build_fs_upload_remote_path("/data/log", "/")
            .expect_err("missing basename should fail");
        assert_eq!(error, "`localPath` must include a file name");
    }

    #[test]
    fn build_fs_download_local_path_uses_remote_basename() {
        let local_path = build_fs_download_local_path("/tmp/downloads", "/data/log/hilog.log")
            .expect("download local path should resolve");
        assert_eq!(local_path, Path::new("/tmp/downloads").join("hilog.log"));
    }

    #[test]
    fn build_fs_download_local_path_rejects_missing_remote_basename() {
        let error = build_fs_download_local_path("/tmp/downloads", "/")
            .expect_err("remote path without basename should fail");
        assert_eq!(error, "`remotePath` must include a file name");
    }

    #[test]
    fn build_fs_download_temp_local_path_uses_temp_root_and_remote_basename() {
        let resolved = build_fs_download_temp_local_path("/data/log/hilog.log")
            .expect("temp download path should resolve");
        let expected_root = fs_download_temp_root_dir();
        assert!(resolved.starts_with(&expected_root));
        let file_name = resolved
            .file_name()
            .and_then(|value| value.to_str())
            .expect("file name should be utf-8");
        assert!(file_name.ends_with("-hilog.log"));
    }

    #[test]
    fn ensure_fs_download_temp_within_limit_rejects_large_files() {
        let error =
            ensure_fs_download_temp_within_limit(11, 10).expect_err("size check should fail");
        assert_eq!(
            error,
            "File is too large to open in editor (11 bytes > 10 bytes limit)"
        );
    }

    #[test]
    fn ensure_fs_download_temp_utf8_rejects_non_utf8_bytes() {
        let error =
            ensure_fs_download_temp_utf8(&[0xff, 0xfe]).expect_err("utf-8 check should fail");
        assert_eq!(
            error,
            "Only UTF-8 text files are supported for Open in Editor"
        );
    }

    #[tokio::test]
    async fn remove_temp_file_if_exists_deletes_existing_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let temp_file_path = std::env::temp_dir().join(format!("harmony-remove-temp-{unique}.txt"));

        std::fs::write(&temp_file_path, b"temporary").expect("temp file should be writable");
        assert!(temp_file_path.exists());

        remove_temp_file_if_exists(&temp_file_path).await;
        assert!(!temp_file_path.exists());
    }

    #[test]
    fn parse_fs_list_output_parses_successful_listing() {
        let output = ".\n..\nlog/\nentry.txt\n.hidden\n__HARMONY_FS_EXIT:0\n";
        let parsed = parse_fs_list_output("/data", output).expect("valid parse");

        assert_eq!(
            parsed,
            vec![
                HdcFsListEntry {
                    path: "/data/log".to_string(),
                    name: "log".to_string(),
                    kind: "directory".to_string(),
                },
                HdcFsListEntry {
                    path: "/data/entry.txt".to_string(),
                    name: "entry.txt".to_string(),
                    kind: "file".to_string(),
                },
                HdcFsListEntry {
                    path: "/data/.hidden".to_string(),
                    name: ".hidden".to_string(),
                    kind: "file".to_string(),
                }
            ]
        );
    }

    #[test]
    fn parse_fs_list_output_handles_root_path_join() {
        let output = "system/\ninit.cfg\n__HARMONY_FS_EXIT:0\n";
        let parsed = parse_fs_list_output("/", output).expect("valid parse");

        assert_eq!(
            parsed,
            vec![
                HdcFsListEntry {
                    path: "/system".to_string(),
                    name: "system".to_string(),
                    kind: "directory".to_string(),
                },
                HdcFsListEntry {
                    path: "/init.cfg".to_string(),
                    name: "init.cfg".to_string(),
                    kind: "file".to_string(),
                }
            ]
        );
    }

    #[test]
    fn parse_fs_list_output_returns_shell_error_message() {
        let output = "ls: /data/secret: Permission denied\n__HARMONY_FS_EXIT:2\n";
        let error = parse_fs_list_output("/data/secret", output).expect_err("expected shell error");
        assert_eq!(error, "ls: /data/secret: Permission denied");
    }

    #[test]
    fn parse_fs_list_output_rejects_missing_sentinel() {
        let output = "system/\ninit.cfg\n";
        let error = parse_fs_list_output("/", output).expect_err("missing sentinel should fail");
        assert_eq!(
            error,
            "failed to parse filesystem listing result (missing exit sentinel)"
        );
    }

    #[test]
    fn parse_fs_delete_output_accepts_success() {
        let output = "__HARMONY_FS_DELETE_EXIT:0\n";
        let parsed = parse_fs_delete_output("/data/log.txt", output);
        assert!(parsed.is_ok());
    }

    #[test]
    fn parse_fs_delete_output_returns_shell_error_message() {
        let output = "rm: cannot remove '/data/nope': No such file or directory\n__HARMONY_FS_DELETE_EXIT:1\n";
        let error =
            parse_fs_delete_output("/data/nope", output).expect_err("expected delete error");
        assert_eq!(
            error,
            "rm: cannot remove '/data/nope': No such file or directory"
        );
    }

    #[test]
    fn parse_fs_delete_output_rejects_missing_sentinel() {
        let output = "rm: output without sentinel\n";
        let error =
            parse_fs_delete_output("/data/nope", output).expect_err("missing sentinel should fail");
        assert_eq!(
            error,
            "failed to parse filesystem delete result (missing exit sentinel)"
        );
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

    #[tokio::test]
    async fn handle_invoke_reports_mcp_list_tools_capability() {
        let (outbound_tx, _outbound_rx) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
        let mut session = ClientSession::new(outbound_tx);

        let response = handle_invoke(
            "capabilities".to_string(),
            json!({
                "action": "host.getCapabilities",
                "args": {}
            }),
            &mut session,
        )
        .await;

        assert_eq!(response.kind, "event");
        assert_eq!(response.payload["name"], "host.getCapabilities.result");
        assert_eq!(
            response.payload["data"]["capabilities"]["mcp.listTools"],
            json!(true)
        );
    }

    #[tokio::test]
    async fn handle_invoke_reports_emulator_capability() {
        let (outbound_tx, _outbound_rx) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
        let mut session = ClientSession::new(outbound_tx);

        let response = handle_invoke(
            "capabilities-emulator".to_string(),
            json!({
                "action": "host.getCapabilities",
                "args": {}
            }),
            &mut session,
        )
        .await;

        assert_eq!(response.kind, "event");
        assert_eq!(
            response.payload["data"]["capabilities"]["emulator.listImages"],
            json!(true)
        );
    }

    #[tokio::test]
    async fn handle_invoke_returns_builtin_mcp_tool_summaries() {
        let (outbound_tx, _outbound_rx) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
        let mut session = ClientSession::new(outbound_tx);

        let response = handle_invoke(
            "mcp-tools".to_string(),
            json!({
                "action": "mcp.listTools",
                "args": {}
            }),
            &mut session,
        )
        .await;

        assert_eq!(response.kind, "event");
        assert_eq!(response.payload["name"], "mcp.listTools.result");

        let tools = response.payload["data"]["tools"]
            .as_array()
            .expect("tools should be an array");
        assert!(tools
            .iter()
            .any(|tool| tool["name"] == "hdc.search_hilog_logs"));
    }

    #[tokio::test]
    async fn handle_invoke_returns_empty_emulator_download_jobs_by_default() {
        let (outbound_tx, _outbound_rx) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
        let mut session = ClientSession::new(outbound_tx);

        let response = handle_invoke(
            "emulator-jobs".to_string(),
            json!({
                "action": "emulator.listDownloadJobs",
                "args": {}
            }),
            &mut session,
        )
        .await;

        assert_eq!(response.kind, "event");
        assert_eq!(response.payload["name"], "emulator.listDownloadJobs.result");
        assert_eq!(response.payload["data"]["jobs"], json!([]));
    }

    #[test]
    fn derive_default_mcp_http_addr_uses_ws_port_plus_offset() {
        assert_eq!(
            derive_default_mcp_http_addr("127.0.0.1:8787").expect("derive should succeed"),
            "127.0.0.1:8887"
        );
        assert_eq!(
            derive_default_mcp_http_addr("localhost:8788").expect("derive should succeed"),
            "localhost:8888"
        );
    }

    #[test]
    fn derive_default_mcp_http_addr_rejects_invalid_format() {
        let error =
            derive_default_mcp_http_addr("127.0.0.1").expect_err("invalid address must fail");
        assert_eq!(
            error,
            "invalid websocket address `127.0.0.1`: expected <host>:<port>"
        );
    }

    #[test]
    fn derive_default_mcp_http_addr_rejects_port_overflow() {
        let error = derive_default_mcp_http_addr("127.0.0.1:65535")
            .expect_err("overflowing derived port must fail");
        assert_eq!(
            error,
            "cannot derive MCP HTTP address from websocket address `127.0.0.1:65535`: port overflow"
        );
    }
}
