use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::client::Client;
use crate::error::HdcError;
use crate::exec::{resolve_path, ExecRunner};
use crate::hilog::HilogStream;
use crate::parsers::parse_parameters;
use crate::shell::ShellSession;
use crate::types::{ForwardMapping, Parameters};

#[derive(Debug, Clone)]
pub struct Target {
    client: Client,
    connect_key: String,
    ready: Arc<AtomicBool>,
    ready_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HilogQueryOptions {
    pub regex: String,
    pub head_lines: Option<u32>,
    pub tail_lines: Option<u32>,
    pub log_types: Option<String>,
    pub level: Option<String>,
    pub domain: Option<String>,
    pub tag: Option<String>,
    pub pid: Option<i64>,
}

impl HilogQueryOptions {
    pub fn to_shell_args(&self) -> Result<Vec<String>, HdcError> {
        let regex = normalize_required_hilog_filter(&self.regex, "regex")?;

        if self.head_lines.is_some() && self.tail_lines.is_some() {
            return Err(HdcError::InvalidInput(
                "head_lines and tail_lines are mutually exclusive".to_string(),
            ));
        }

        let mut args = vec!["hilog".to_string()];

        if let Some(head_lines) = self.head_lines {
            if head_lines == 0 {
                return Err(HdcError::InvalidInput(
                    "head_lines must be a positive integer".to_string(),
                ));
            }
            args.push("-a".to_string());
            args.push(head_lines.to_string());
        }

        if let Some(tail_lines) = self.tail_lines {
            if tail_lines == 0 {
                return Err(HdcError::InvalidInput(
                    "tail_lines must be a positive integer".to_string(),
                ));
            }
            args.push("-z".to_string());
            args.push(tail_lines.to_string());
        }

        if let Some(log_types) = normalize_optional_hilog_filter(self.log_types.as_deref()) {
            args.push("-t".to_string());
            args.push(log_types);
        }

        if let Some(level) = normalize_optional_hilog_filter(self.level.as_deref()) {
            args.push("-L".to_string());
            args.push(level);
        }

        if let Some(domain) = normalize_optional_hilog_filter(self.domain.as_deref()) {
            args.push("-D".to_string());
            args.push(domain);
        }

        if let Some(tag) = normalize_optional_hilog_filter(self.tag.as_deref()) {
            args.push("-T".to_string());
            args.push(tag);
        }

        if let Some(pid) = self.pid {
            if pid <= 0 {
                return Err(HdcError::InvalidInput(
                    "pid must be a positive integer".to_string(),
                ));
            }
            args.push("-P".to_string());
            args.push(pid.to_string());
        }

        args.push("-e".to_string());
        args.push(regex);
        args.push("-v".to_string());
        args.push("epoch".to_string());
        args.push("-v".to_string());
        args.push("msec".to_string());

        Ok(args)
    }
}

impl Target {
    pub(crate) fn new(client: Client, connect_key: String) -> Self {
        Self {
            client,
            connect_key,
            ready: Arc::new(AtomicBool::new(false)),
            ready_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn get_parameters(&self) -> Result<Parameters, HdcError> {
        let mut transport = self.transport().await?;
        transport.send(b"shell param get").await?;

        let data = transport.read_all().await?;
        Ok(parse_parameters(&String::from_utf8_lossy(&data)))
    }

    pub async fn shell(&self, command: &str) -> Result<ShellSession, HdcError> {
        let mut transport = self.transport().await?;
        transport
            .send(format!("shell {command}").as_bytes())
            .await?;

        Ok(ShellSession::new(transport))
    }

    pub async fn send_file(&self, local: &Path, remote: &str) -> Result<(), HdcError> {
        let runner = ExecRunner::new(self.client.bin(), &self.connect_key);
        let local = resolve_path(local)?;

        let stdout = runner
            .run(&[
                "file".to_string(),
                "send".to_string(),
                local.display().to_string(),
                remote.to_string(),
            ])
            .await?;

        if !stdout.contains("finish") {
            return Err(HdcError::Protocol("send file failed".to_string()));
        }

        Ok(())
    }

    pub async fn recv_file(&self, remote: &str, local: &Path) -> Result<(), HdcError> {
        let runner = ExecRunner::new(self.client.bin(), &self.connect_key);
        let local = resolve_path(local)?;

        let stdout = runner
            .run(&[
                "file".to_string(),
                "recv".to_string(),
                remote.to_string(),
                local.display().to_string(),
            ])
            .await?;

        if !stdout.contains("finish") {
            return Err(HdcError::Protocol("recv file failed".to_string()));
        }

        Ok(())
    }

    pub async fn install(&self, hap: &Path) -> Result<(), HdcError> {
        let runner = ExecRunner::new(self.client.bin(), &self.connect_key);
        let hap = resolve_path(hap)?;

        let stdout = runner
            .run(&["install".to_string(), hap.display().to_string()])
            .await?;

        if !stdout.contains("install bundle successfully") {
            return Err(HdcError::Protocol(stdout));
        }

        Ok(())
    }

    pub async fn uninstall(&self, bundle_name: &str) -> Result<(), HdcError> {
        let runner = ExecRunner::new(self.client.bin(), &self.connect_key);

        let stdout = runner
            .run(&["uninstall".to_string(), bundle_name.to_string()])
            .await?;

        if !stdout.contains("uninstall bundle successfully") {
            return Err(HdcError::Protocol("uninstall bundle failed".to_string()));
        }

        Ok(())
    }

    pub async fn forward(&self, local: &str, remote: &str) -> Result<(), HdcError> {
        let mut transport = self.transport().await?;
        transport
            .send(format!("fport {local} {remote}").as_bytes())
            .await?;

        let result = String::from_utf8_lossy(&transport.read_value().await?).to_string();
        if !result.contains("OK") {
            return Err(HdcError::Protocol(result));
        }

        Ok(())
    }

    pub async fn list_forwards(&self) -> Result<Vec<ForwardMapping>, HdcError> {
        let forwards = self.client.list_forwards().await?;
        Ok(forwards
            .into_iter()
            .filter(|forward| forward.target == self.connect_key)
            .collect())
    }

    pub async fn remove_forward(&self, local: &str, remote: &str) -> Result<(), HdcError> {
        let mut transport = self.transport().await?;
        transport
            .send(format!("fport rm {local} {remote}").as_bytes())
            .await?;

        let result = String::from_utf8_lossy(&transport.read_value().await?).to_string();
        if !result.contains("success") {
            return Err(HdcError::Protocol(result));
        }

        Ok(())
    }

    pub async fn reverse(&self, remote: &str, local: &str) -> Result<(), HdcError> {
        let mut transport = self.transport().await?;
        transport
            .send(format!("rport {remote} {local}").as_bytes())
            .await?;

        let result = String::from_utf8_lossy(&transport.read_value().await?).to_string();
        if !result.contains("OK") {
            return Err(HdcError::Protocol(result));
        }

        Ok(())
    }

    pub async fn list_reverses(&self) -> Result<Vec<ForwardMapping>, HdcError> {
        let reverses = self.client.list_reverses().await?;
        Ok(reverses
            .into_iter()
            .filter(|reverse| reverse.target == self.connect_key)
            .collect())
    }

    pub async fn remove_reverse(&self, remote: &str, local: &str) -> Result<(), HdcError> {
        // Keep behavior parity with the TypeScript client quirk.
        let mut transport = self.transport().await?;
        transport
            .send(format!("fport rm {remote} {local}").as_bytes())
            .await?;

        let result = String::from_utf8_lossy(&transport.read_value().await?).to_string();
        if !result.contains("success") {
            return Err(HdcError::Protocol(result));
        }

        Ok(())
    }

    pub async fn open_hilog(&self, clear: bool) -> Result<HilogStream, HdcError> {
        self.open_hilog_with_filters(clear, None, None).await
    }

    pub async fn open_hilog_with_level(
        &self,
        clear: bool,
        level: Option<&str>,
    ) -> Result<HilogStream, HdcError> {
        self.open_hilog_with_filters(clear, level, None).await
    }

    pub async fn open_hilog_with_filters(
        &self,
        clear: bool,
        level: Option<&str>,
        pid: Option<i64>,
    ) -> Result<HilogStream, HdcError> {
        if clear {
            let mut clear_session = self.shell("hilog -r").await?;
            let _ = clear_session.read_all().await?;
        }

        let mut transport = self.transport().await?;
        let command = format!("shell {}", build_hilog_command(level, pid));
        transport.send(command.as_bytes()).await?;
        Ok(HilogStream::new(transport))
    }

    pub async fn query_hilog(&self, options: &HilogQueryOptions) -> Result<String, HdcError> {
        let shell_args = options.to_shell_args()?;
        let command = build_hilog_query_command(&shell_args);
        let mut session = self.shell(&command).await?;
        let output = session.read_all_string().await?;
        Ok(strip_hilog_transport_noise(&output))
    }

    async fn transport(&self) -> Result<crate::connection::Connection, HdcError> {
        if !self.ready.load(Ordering::SeqCst) {
            self.wait_until_ready().await?;
        }

        self.client.connection(Some(&self.connect_key)).await
    }

    async fn wait_until_ready(&self) -> Result<(), HdcError> {
        let _guard = self.ready_lock.lock().await;

        if self.ready.load(Ordering::SeqCst) {
            return Ok(());
        }

        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            if self.check_ready_once().await? {
                self.ready.store(true, Ordering::SeqCst);
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(HdcError::Timeout(
                    "target readiness check timed out".to_string(),
                ));
            }

            sleep(Duration::from_secs(1)).await;
        }
    }

    async fn check_ready_once(&self) -> Result<bool, HdcError> {
        let mut transport = self.client.connection(Some(&self.connect_key)).await?;
        transport.send(b"shell echo ready\n").await?;

        let data = transport.read_all().await?;
        Ok(!String::from_utf8_lossy(&data).contains("E000004"))
    }
}

fn build_hilog_command(level: Option<&str>, pid: Option<i64>) -> String {
    let mut command = String::from("hilog -v wrap -v epoch");

    if let Some(level) = level.map(str::trim).filter(|value| !value.is_empty()) {
        command.push_str(" -L ");
        command.push_str(&shell_single_quote(level));
    }

    if let Some(pid) = pid.filter(|value| *value > 0) {
        command.push_str(" -P ");
        command.push_str(&shell_single_quote(&pid.to_string()));
    }

    command
}

fn build_hilog_query_command(args: &[String]) -> String {
    let mut iter = args.iter();
    let mut command = iter.next().cloned().unwrap_or_default();

    for arg in iter {
        command.push(' ');
        command.push_str(&shell_single_quote(arg));
    }

    command
}

fn strip_hilog_transport_noise(output: &str) -> String {
    output
        .lines()
        .filter(|line| !is_hilog_transport_noise(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_hilog_transport_noise(line: &str) -> bool {
    line.contains("/hdcd/HDC_LOG:")
        && (line.contains("ExecuteCommand cmd:hilog")
            || line.contains("[FetchCommand:")
            || line.contains("[BeginRemoveTask:")
            || line.contains("[ClearOwnTasks:")
            || line.contains("taskClassDeleteRetry")
            || line.contains("[DoRelease:"))
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn normalize_required_hilog_filter(value: &str, field_name: &str) -> Result<String, HdcError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(HdcError::InvalidInput(format!(
            "{field_name} must be a non-empty string"
        )));
    }

    Ok(normalized.to_string())
}

fn normalize_optional_hilog_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{
        build_hilog_command, build_hilog_query_command, is_hilog_transport_noise,
        strip_hilog_transport_noise, HilogQueryOptions,
    };
    use crate::error::HdcError;

    #[test]
    fn build_hilog_command_without_level_filter() {
        assert_eq!(build_hilog_command(None, None), "hilog -v wrap -v epoch");
        assert_eq!(
            build_hilog_command(Some("   "), None),
            "hilog -v wrap -v epoch"
        );
    }

    #[test]
    fn build_hilog_command_with_level_filter() {
        assert_eq!(
            build_hilog_command(Some("I,W,E"), None),
            "hilog -v wrap -v epoch -L 'I,W,E'"
        );
        assert_eq!(
            build_hilog_command(Some(" INFO,ERROR "), None),
            "hilog -v wrap -v epoch -L 'INFO,ERROR'"
        );
    }

    #[test]
    fn build_hilog_command_escapes_single_quote() {
        assert_eq!(
            build_hilog_command(Some("I,'; echo pwn"), None),
            "hilog -v wrap -v epoch -L 'I,'\"'\"'; echo pwn'"
        );
    }

    #[test]
    fn build_hilog_command_with_pid_filter() {
        assert_eq!(
            build_hilog_command(None, Some(1234)),
            "hilog -v wrap -v epoch -P '1234'"
        );
        assert_eq!(
            build_hilog_command(Some("I,W,E"), Some(1234)),
            "hilog -v wrap -v epoch -L 'I,W,E' -P '1234'"
        );
    }

    #[test]
    fn hilog_query_args_include_filters_and_epoch_format() {
        let options = HilogQueryOptions {
            regex: "NullPointerException".to_string(),
            tail_lines: Some(120),
            level: Some("E,F".to_string()),
            tag: Some("AbilityManager".to_string()),
            pid: Some(4242),
            ..Default::default()
        };

        assert_eq!(
            options.to_shell_args().unwrap(),
            vec![
                "hilog",
                "-z",
                "120",
                "-L",
                "E,F",
                "-T",
                "AbilityManager",
                "-P",
                "4242",
                "-e",
                "NullPointerException",
                "-v",
                "epoch",
                "-v",
                "msec",
            ]
        );
    }

    #[test]
    fn hilog_query_args_reject_invalid_inputs() {
        let empty_regex = HilogQueryOptions {
            regex: "   ".to_string(),
            ..Default::default()
        };
        assert!(matches!(
            empty_regex.to_shell_args(),
            Err(HdcError::InvalidInput(message)) if message.contains("regex")
        ));

        let invalid_window = HilogQueryOptions {
            regex: "panic".to_string(),
            head_lines: Some(10),
            tail_lines: Some(10),
            ..Default::default()
        };
        assert!(matches!(
            invalid_window.to_shell_args(),
            Err(HdcError::InvalidInput(message)) if message.contains("mutually exclusive")
        ));

        let invalid_pid = HilogQueryOptions {
            regex: "panic".to_string(),
            pid: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            invalid_pid.to_shell_args(),
            Err(HdcError::InvalidInput(message)) if message.contains("pid")
        ));
    }

    #[test]
    fn hilog_query_command_quotes_each_argument_for_shell_transport() {
        assert_eq!(
            build_hilog_query_command(&[
                "hilog".to_string(),
                "-z".to_string(),
                "20".to_string(),
                "-e".to_string(),
                "panic 'quoted'".to_string(),
            ]),
            "hilog '-z' '20' '-e' 'panic '\"'\"'quoted'\"'\"''"
        );
    }

    #[test]
    fn hilog_transport_noise_filter_removes_shell_execution_artifacts() {
        let output = "\
1760766955.654 10654 27842 I C02D13/hdcd/HDC_LOG: [FetchCommand:1004] FetchCommand channelId:239872155 command:2\n\
1760766955.655 10654 27842 I C02D13/hdcd/HDC_LOG: [ExecuteCommand:280] ExecuteCommand cmd:hilog -z 20 -e panic fd:47 pid:41225\n\
1760766955.700  1689 24066 E C05741/softbus_server/TransSvc: NearbySocketReject# destroyList is empty.\n";

        assert!(is_hilog_transport_noise(
            "1760766955.655 10654 27842 I C02D13/hdcd/HDC_LOG: [ExecuteCommand:280] ExecuteCommand cmd:hilog -z 20 -e panic fd:47 pid:41225"
        ));
        assert_eq!(
            strip_hilog_transport_noise(output),
            "1760766955.700  1689 24066 E C05741/softbus_server/TransSvc: NearbySocketReject# destroyList is empty."
        );
    }
}
