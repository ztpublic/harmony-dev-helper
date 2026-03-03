use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs;

use crate::connection::Connection;
use crate::error::HdcError;
use crate::parsers::{read_ports, read_targets};
use crate::target::Target;
use crate::tracker::TargetTracker;
use crate::types::ForwardMapping;

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub host: String,
    pub port: u16,
    pub bin: PathBuf,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8710,
            bin: PathBuf::from("hdc"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    options: Arc<ClientOptions>,
}

impl Client {
    pub fn from_env() -> Self {
        let mut options = ClientOptions::default();

        if let Ok(raw) = std::env::var("OHOS_HDC_SERVER_PORT") {
            if let Ok(port) = raw.parse::<u16>() {
                options.port = port;
            }
        }

        Self::new(options)
    }

    pub fn new(options: ClientOptions) -> Self {
        Self {
            options: Arc::new(options),
        }
    }

    pub fn get_target(&self, connect_key: impl Into<String>) -> Result<Target, HdcError> {
        let connect_key = connect_key.into();
        if connect_key.trim().is_empty() {
            return Err(HdcError::InvalidInput(
                "connect_key is required".to_string(),
            ));
        }

        Ok(Target::new(self.clone(), connect_key))
    }

    pub async fn list_targets(&self) -> Result<Vec<String>, HdcError> {
        let mut conn = self.connection(None).await?;
        conn.send(b"list targets").await?;

        let data = conn.read_value().await?;
        Ok(read_targets(&String::from_utf8_lossy(&data)))
    }

    pub async fn track_targets(&self) -> Result<TargetTracker, HdcError> {
        let mut conn = self.connection(None).await?;
        conn.send(b"alive").await?;

        Ok(TargetTracker::new(conn))
    }

    pub async fn list_forwards(&self) -> Result<Vec<ForwardMapping>, HdcError> {
        let mut conn = self.connection(None).await?;
        conn.send(b"fport ls").await?;

        let data = conn.read_value().await?;
        Ok(read_ports(&String::from_utf8_lossy(&data), false))
    }

    pub async fn list_reverses(&self) -> Result<Vec<ForwardMapping>, HdcError> {
        let mut conn = self.connection(None).await?;
        // Keep behavior parity with the TypeScript client quirk.
        conn.send(b"fport ls").await?;

        let data = conn.read_value().await?;
        Ok(read_ports(&String::from_utf8_lossy(&data), true))
    }

    pub async fn kill_server(&self) -> Result<(), HdcError> {
        let pid_path = std::env::temp_dir().join(".HDCServer.pid");

        let pid = match fs::read_to_string(pid_path).await {
            Ok(contents) => contents.trim().parse::<i32>().unwrap_or(0),
            Err(_) => 0,
        };

        if pid > 0 {
            // Keep behavior parity: ignore kill failure.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }

        Ok(())
    }

    pub(crate) async fn connection(
        &self,
        connect_key: Option<&str>,
    ) -> Result<Connection, HdcError> {
        Connection::connect(&self.options, connect_key).await
    }

    pub(crate) fn bin(&self) -> &Path {
        &self.options.bin
    }
}
