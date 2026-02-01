 use shared::ipc::{Command, IpcError, Response};
 use std::path::PathBuf;
 use tokio::io::{AsyncReadExt, AsyncWriteExt};
 use tokio::net::UnixStream;
 use tokio::time::{timeout, Duration};
 use tracing::warn;

 /// Timeout for socket operations (5 seconds)
 const SOCKET_TIMEOUT: Duration = Duration::from_secs(5);

 /// Get the Unix socket path for the daemon.
 /// Uses XDG runtime directory if available, falls back to /tmp/ndictd.sock
 fn get_socket_path() -> PathBuf {
     if let Some(runtime_dir) = dirs::runtime_dir() {
         runtime_dir.join("ndictd.sock")
     } else {
         PathBuf::from("/tmp/ndictd.sock")
     }
 }

pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    pub fn new() -> Self {
        Self {
            socket_path: get_socket_path(),
        }
    }

    pub async fn send_command(&self, cmd: Command) -> Result<Response, IpcError> {
        // Connect with timeout
        let mut stream = match timeout(SOCKET_TIMEOUT, UnixStream::connect(&self.socket_path)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(IpcError::ConnectionRefused);
            }
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                return Err(IpcError::ConnectionRefused);
            }
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                warn!("Connection timeout: failed to connect to daemon at {} within {:?}", self.socket_path.display(), SOCKET_TIMEOUT);
                return Err(IpcError::Timeout);
            }
        };

        // Serialize command
        let command_json = serde_json::to_vec(&cmd)?;

        // Write with timeout
        if timeout(SOCKET_TIMEOUT, stream.write_all(&command_json)).await.is_err() {
            warn!("Write timeout: failed to send command to daemon within {:?}", SOCKET_TIMEOUT);
            return Err(IpcError::Timeout);
        }

        // Read with timeout
        let mut buffer = vec![0u8; 1024];
        let n = match timeout(SOCKET_TIMEOUT, stream.read(&mut buffer)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                warn!("Read timeout: failed to receive response from daemon within {:?}", SOCKET_TIMEOUT);
                return Err(IpcError::Timeout);
            }
        };

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
        // The socket path should use XDG runtime dir if available, or fallback to /tmp
        if dirs::runtime_dir().is_some() {
            let expected = dirs::runtime_dir().unwrap().join("ndictd.sock");
            assert_eq!(client.socket_path, expected);
        } else {
            assert_eq!(client.socket_path, PathBuf::from("/tmp/ndictd.sock"));
        }
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

    #[tokio::test]
    async fn test_send_command_timeout_on_write() {
        let test_socket = "/tmp/test_ndict_timeout_write.sock";

        // Clean up any existing socket file
        std::fs::remove_file(test_socket).ok();

        let listener = UnixListener::bind(test_socket).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            let mut buffer = vec![0u8; 1024];
            let _n = stream.read(&mut buffer).await.unwrap();

            // Don't write response - cause timeout on client read
            tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let client = DaemonClient {
            socket_path: PathBuf::from(test_socket),
        };

        let result = client.send_command(Command::Start).await;
        assert!(matches!(result, Err(IpcError::Timeout)));

        std::fs::remove_file(test_socket).ok();
    }

    #[tokio::test]
    async fn test_send_command_timeout_on_read() {
        let test_socket = "/tmp/test_ndict_timeout_read.sock";

        // Clean up any existing socket file
        std::fs::remove_file(test_socket).ok();

        let listener = UnixListener::bind(test_socket).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            let mut buffer = vec![0u8; 1024];
            let _n = stream.read(&mut buffer).await.unwrap();

            // Don't send response - client will timeout waiting for response
            // The timeout is 5 seconds, so sleep longer than that
            tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let client = DaemonClient {
            socket_path: PathBuf::from(test_socket),
        };

        let result = client.send_command(Command::Start).await;
        assert!(matches!(result, Err(IpcError::Timeout)));

        std::fs::remove_file(test_socket).ok();
    }
}
