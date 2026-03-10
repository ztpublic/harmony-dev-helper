use std::path::PathBuf;

use thiserror::Error;

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
pub enum ImageManagerError {
    #[error("home directory is unavailable")]
    HomeDirectoryUnavailable,

    #[error("invalid default config source for {label}: {message}")]
    DefaultConfig {
        label: &'static str,
        message: String,
    },

    #[error("invalid json in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid json5 in {label}: {source}")]
    Json5 {
        label: &'static str,
        #[source]
        source: serde_json5::Error,
    },

    #[error("invalid ini in {path}: {message}")]
    Ini { path: PathBuf, message: String },

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

    #[error("checksum mismatch for {path}")]
    ChecksumMismatch { path: PathBuf },

    #[error("archive at {path} is invalid: {message}")]
    Archive { path: PathBuf, message: String },

    #[error("path is outside extraction root: {path}")]
    UnsafeArchivePath { path: PathBuf },

    #[error("process launch failed for {program}: {source}")]
    Process {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("validation failed: {0}")]
    Validation(String),
}
