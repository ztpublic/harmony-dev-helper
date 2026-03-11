use std::path::PathBuf;

use thiserror::Error;

use crate::{SdkArch, SdkOs, SdkVersion};

#[derive(Debug, Clone)]
pub struct RemoteApiError {
    pub endpoint: &'static str,
    pub status: Option<u16>,
    pub body: Option<String>,
    pub message: String,
}

impl std::fmt::Display for RemoteApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(status) => write!(f, "{} failed with status {}", self.endpoint, status),
            None => write!(f, "{} failed", self.endpoint),
        }
    }
}

impl std::error::Error for RemoteApiError {}

#[derive(Debug, Error)]
pub enum SdkManagerError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("{0}")]
    RemoteApi(#[from] RemoteApiError),

    #[error("unsupported sdk combination: version={version}, arch={arch}, os={os}")]
    UnsupportedSdk {
        version: SdkVersion,
        arch: SdkArch,
        os: SdkOs,
    },

    #[error("invalid url: {0}")]
    InvalidUrl(String),

    #[error("invalid sha256 checksum: {0}")]
    InvalidChecksum(String),

    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("archive at {path} is invalid: {message}")]
    Archive { path: PathBuf, message: String },

    #[error("path is outside extraction root: {path}")]
    UnsafeArchivePath { path: PathBuf },

    #[error("no nested sdk zip archives found in {path} for host {host_os}")]
    MissingNestedArchive { path: PathBuf, host_os: SdkOs },
}
