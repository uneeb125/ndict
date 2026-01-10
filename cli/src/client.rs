use shared::ipc::{Command, IpcError, Response};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/ndictd.sock";

pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    pub fn new() -> Self {
        Self {
            socket_path: PathBuf::from(SOCKET_PATH),
        }
    }

    pub async fn send_command(&self, cmd: Command) -> Result<Response, IpcError> {
        let mut stream = match UnixStream::connect(&self.socket_path).await {
            Ok(stream) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(IpcError::ConnectionRefused);
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                return Err(IpcError::ConnectionRefused);
            }
            Err(e) => return Err(e.into()),
        };

        let command_json = serde_json::to_vec(&cmd)?;
        stream.write_all(&command_json).await?;

        let mut buffer = vec![0u8; 1024];
        let n = stream.read(&mut buffer).await?;

        buffer.truncate(n);

        let response: Response = serde_json::from_slice(&buffer)?;

        Ok(response)
    }
}
