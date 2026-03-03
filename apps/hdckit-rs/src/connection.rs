use std::env;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;

use crate::client::ClientOptions;
use crate::error::HdcError;
use crate::handshake::ChannelHandshake;

const HANDSHAKE_MESSAGE: &str = "OHOS HDC";

#[derive(Debug)]
pub struct Connection {
    stream: TcpStream,
    ended: bool,
}

impl Connection {
    pub async fn connect(
        options: &ClientOptions,
        connect_key: Option<&str>,
    ) -> Result<Self, HdcError> {
        let mut tried_starting = false;

        loop {
            match Self::connect_once(options, connect_key).await {
                Ok(conn) => return Ok(conn),
                Err(HdcError::Io(err))
                    if err.kind() == std::io::ErrorKind::ConnectionRefused && !tried_starting =>
                {
                    tried_starting = true;
                    Self::start_server(options).await?;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn connect_once(
        options: &ClientOptions,
        connect_key: Option<&str>,
    ) -> Result<Self, HdcError> {
        let stream = TcpStream::connect((options.host.as_str(), options.port)).await?;
        stream.set_nodelay(true)?;

        let mut conn = Self {
            stream,
            ended: false,
        };

        conn.handshake(connect_key).await?;
        Ok(conn)
    }

    async fn start_server(options: &ClientOptions) -> Result<(), HdcError> {
        let mut command = Command::new(&options.bin);
        command.arg("start");
        command.env("OHOS_HDC_SERVER_PORT", options.port.to_string());

        for (k, v) in env::vars() {
            command.env(k, v);
        }

        let output = command.output().await?;
        if !output.status.success() {
            return Err(HdcError::SubprocessFailure {
                command: format!("{} start", options.bin.display()),
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    async fn handshake(&mut self, connect_key: Option<&str>) -> Result<(), HdcError> {
        let data = self.read_value().await?;
        let mut channel_handshake = ChannelHandshake::deserialize(&data)?;

        let banner = String::from_utf8_lossy(&channel_handshake.banner);
        if !banner.starts_with(HANDSHAKE_MESSAGE) {
            return Err(HdcError::Protocol("channel hello failed".to_string()));
        }

        if let Some(key) = connect_key {
            channel_handshake.connect_key = key.to_string();
        }

        self.send(&channel_handshake.serialize()).await
    }

    pub async fn send(&mut self, data: &[u8]) -> Result<(), HdcError> {
        let mut frame = Vec::with_capacity(4 + data.len());
        frame.extend_from_slice(&(data.len() as u32).to_be_bytes());
        frame.extend_from_slice(data);

        self.stream.write_all(&frame).await?;
        Ok(())
    }

    pub async fn read_value(&mut self) -> Result<Vec<u8>, HdcError> {
        let mut len_buf = [0u8; 4];
        if let Err(err) = self.stream.read_exact(&mut len_buf).await {
            if err.kind() == std::io::ErrorKind::UnexpectedEof {
                self.ended = true;
                return Err(HdcError::ClosedConnection);
            }
            return Err(HdcError::Io(err));
        }

        let len = u32::from_be_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        if len == 0 {
            return Ok(data);
        }

        if let Err(err) = self.stream.read_exact(&mut data).await {
            if err.kind() == std::io::ErrorKind::UnexpectedEof {
                self.ended = true;
                return Err(HdcError::ClosedConnection);
            }
            return Err(HdcError::Io(err));
        }

        Ok(data)
    }

    pub async fn read_all(&mut self) -> Result<Vec<u8>, HdcError> {
        let mut all = Vec::new();

        loop {
            match self.read_value().await {
                Ok(chunk) => all.extend_from_slice(&chunk),
                Err(HdcError::ClosedConnection) => return Ok(all),
                Err(err) => return Err(err),
            }
        }
    }

    pub async fn end(&mut self) -> Result<(), HdcError> {
        if self.ended {
            return Ok(());
        }

        self.ended = true;
        self.stream.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    use super::Connection;
    use crate::client::ClientOptions;

    #[tokio::test]
    async fn read_value_supports_fragmented_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            // handshake request
            socket
                .write_all(&[
                    0, 0, 0, 16, b'O', b'H', b'O', b'S', b' ', b'H', b'D', b'C', 0, 0, 0, 0, 0, 0,
                    0, 1,
                ])
                .await
                .unwrap();

            // read handshake response frame and payload
            let mut discard = vec![0u8; 4 + 44];
            tokio::io::AsyncReadExt::read_exact(&mut socket, &mut discard)
                .await
                .unwrap();

            // send command response frame in pieces
            socket.write_all(&[0, 0]).await.unwrap();
            socket.write_all(&[0, 5]).await.unwrap();
            socket.write_all(b"hello").await.unwrap();
        });

        let options = ClientOptions {
            host: "127.0.0.1".to_string(),
            port,
            bin: "hdc".into(),
        };

        let mut conn = Connection::connect(&options, None).await.unwrap();
        let value = conn.read_value().await.unwrap();
        assert_eq!(value, b"hello");

        server.await.unwrap();
    }
}
