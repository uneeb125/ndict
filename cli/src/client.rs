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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::StatusInfo;
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn test_daemon_client_new() {
        let client = DaemonClient::new();
        assert_eq!(client.socket_path, PathBuf::from("/tmp/ndictd.sock"));
    }

    #[tokio::test]
    async fn test_send_command_socket_not_found() {
        let client = DaemonClient::new();
        let result = client.send_command(Command::Start).await;
        assert!(matches!(result, Err(IpcError::ConnectionRefused)));
    }

    #[tokio::test]
    async fn test_send_command_serialization() {
        let cmd = Command::SetLanguage("test".to_string());
        let json = serde_json::to_vec(&cmd).unwrap();
        assert!(json.len() > 0);

        let parsed: Command = serde_json::from_slice(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[tokio::test]
    async fn test_send_command_with_mock_server() {
        let test_socket = "/tmp/test_ndict.sock";

        let listener = UnixListener::bind(test_socket).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            let mut buffer = vec![0u8; 1024];
            let n = stream.read(&mut buffer).await.unwrap();
            buffer.truncate(n);

            let command: Command = serde_json::from_slice(&buffer).unwrap();

            let response = match command {
                Command::Start => Response::Ok,
                Command::Status => Response::Status(StatusInfo {
                    is_running: true,
                    is_active: false,
                    language: "en".to_string(),
                }),
                _ => Response::Error("unknown".to_string()),
            };

            let response_json = serde_json::to_vec(&response).unwrap();
            stream.write_all(&response_json).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let client = DaemonClient {
            socket_path: PathBuf::from(test_socket),
        };

        let result = client.send_command(Command::Start).await;
        assert!(matches!(result, Ok(Response::Ok)));

        std::fs::remove_file(test_socket).ok();
    }

    #[tokio::test]
    async fn test_send_command_status() {
        let test_socket = "/tmp/test_ndict_status.sock";

        let listener = UnixListener::bind(test_socket).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            let mut buffer = vec![0u8; 1024];
            let n = stream.read(&mut buffer).await.unwrap();
            buffer.truncate(n);

            let command: Command = serde_json::from_slice(&buffer).unwrap();
            assert!(matches!(command, Command::Status));

            let response = Response::Status(StatusInfo {
                is_running: true,
                is_active: false,
                language: "en".to_string(),
            });

            let response_json = serde_json::to_vec(&response).unwrap();
            stream.write_all(&response_json).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let client = DaemonClient {
            socket_path: PathBuf::from(test_socket),
        };

        let result = client.send_command(Command::Status).await;
        assert!(matches!(result, Ok(Response::Status(_))));

        if let Ok(Response::Status(info)) = result {
            assert_eq!(info.is_running, true);
            assert_eq!(info.is_active, false);
            assert_eq!(info.language, "en");
        }

        std::fs::remove_file(test_socket).ok();
    }

    #[tokio::test]
    async fn test_send_command_error_response() {
        let test_socket = "/tmp/test_ndict_error.sock";

        let listener = UnixListener::bind(test_socket).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            let mut buffer = vec![0u8; 1024];
            let n = stream.read(&mut buffer).await.unwrap();
            buffer.truncate(n);

            let response = Response::Error("test error".to_string());
            let response_json = serde_json::to_vec(&response).unwrap();
            stream.write_all(&response_json).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let client = DaemonClient {
            socket_path: PathBuf::from(test_socket),
        };

        let result = client.send_command(Command::Start).await;
        assert!(matches!(result, Ok(Response::Error(_))));

        std::fs::remove_file(test_socket).ok();
    }

    #[tokio::test]
    async fn test_send_command_all_variants() {
        let commands = vec![
            Command::Start,
            Command::Stop,
            Command::Pause,
            Command::Resume,
            Command::Status,
            Command::SetLanguage("test".to_string()),
            Command::Toggle,
        ];

        for cmd in commands {
            let json = serde_json::to_vec(&cmd).unwrap();
            let parsed: Command = serde_json::from_slice(&json).unwrap();
            assert_eq!(cmd, parsed);
        }
    }
}
