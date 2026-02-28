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

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::build_hilog_command;

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
}
