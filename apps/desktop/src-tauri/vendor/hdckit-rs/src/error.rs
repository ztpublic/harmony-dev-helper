use std::io;

#[derive(Debug, thiserror::Error)]
pub enum HdcError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error(
        "subprocess failed: command={command}, code={code:?}, stdout={stdout}, stderr={stderr}"
    )]
    SubprocessFailure {
        command: String,
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("connection closed")]
    ClosedConnection,
}
