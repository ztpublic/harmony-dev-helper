use crate::connection::Connection;
use crate::error::HdcError;

#[derive(Debug)]
pub struct ShellSession {
    connection: Connection,
}

impl ShellSession {
    pub(crate) fn new(connection: Connection) -> Self {
        Self { connection }
    }

    pub async fn read_value(&mut self) -> Result<Vec<u8>, HdcError> {
        self.connection.read_value().await
    }

    pub async fn read_all(&mut self) -> Result<Vec<u8>, HdcError> {
        self.connection.read_all().await
    }

    pub async fn read_all_string(&mut self) -> Result<String, HdcError> {
        let output = self.read_all().await?;
        Ok(String::from_utf8_lossy(&output).to_string())
    }

    pub async fn end(&mut self) -> Result<(), HdcError> {
        self.connection.end().await
    }
}
