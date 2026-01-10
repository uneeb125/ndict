use shared::ipc::{Command, Response};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::audio::capture::AudioCapture;
use crate::output::keyboard::VirtualKeyboard;
use crate::state::DaemonState;
use crate::transcription::engine::WhisperEngine;

const SOCKET_PATH: &str = "/tmp/ndictd.sock";

pub struct DaemonServer {
    socket_path: PathBuf,
    state: Arc<Mutex<DaemonState>>,
}

impl DaemonServer {
    pub fn new(socket_path: PathBuf, state: Arc<Mutex<DaemonState>>) -> Self {
        Self { socket_path, state }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let socket_path = self.socket_path.clone();

        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }

        info!("Starting socket server at {}", socket_path.display());

        let listener = UnixListener::bind(&socket_path)?;
        debug!("Listener bound successfully");

        loop {
            debug!("Waiting for connection...");
            let state = Arc::clone(&self.state);
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    debug!("Connection accepted");
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(state, stream).await {
                            error!("Error handling connection: {}", e);
                        } else {
                            debug!("Connection handled successfully");
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        state: Arc<Mutex<DaemonState>>,
        mut stream: tokio::net::UnixStream,
    ) -> anyhow::Result<()> {
        let mut buffer = vec![0u8; 1024];
        let n = stream.read(&mut buffer).await?;

        if n == 0 {
            return Ok(());
        }

        buffer.truncate(n);

        let command: Command = serde_json::from_slice(&buffer)?;

        info!("Received command: {:?}", command);

        let response = match command {
            Command::Start => {
                let mut state_guard = state.lock().await;
                state_guard.activate().await?;

                if state_guard.audio_capture.lock().await.is_some() {
                    return Err(anyhow::anyhow!("Audio capture already running"));
                }

                let mut new_capture = AudioCapture::new()?;
                let (audio_tx, audio_rx) = tokio::sync::broadcast::channel(100);
                new_capture.start(audio_tx)?;
                *state_guard.audio_capture.lock().await = Some(new_capture);
                *state_guard.audio_rx.lock().await = Some(audio_rx);
                debug!("Audio capture started, VAD, Whisper, and Keyboard ready");

                debug!("Audio capture started, starting VAD and Whisper processing");
                if let Err(e) = state_guard.start_vad_processing().await {
                    error!("Failed to start VAD and Whisper processing: {}", e);
                    return Err(anyhow::anyhow!("{}", e));
                }

                info!("Activated audio capture");
                Response::Ok
            }
            Command::Stop => {
                let mut state_guard = state.lock().await;
                state_guard.stop_vad_processing().await;
                if let Some(capture) = state_guard.audio_capture.lock().await.as_mut() {
                    capture.stop().await?;
                }
                state_guard.deactivate().await?;
                info!("Deactivated audio capture");
                Response::Ok
            }
            Command::Pause => {
                info!("Pause not yet implemented");
                Response::Ok
            }
            Command::Resume => {
                info!("Resume not yet implemented");
                Response::Ok
            }
            Command::Status => {
                let status = state.lock().await.get_status().await;
                Response::Status(status)
            }
            Command::SetLanguage(_lang) => {
                info!("SetLanguage not yet implemented");
                Response::Ok
            }
        };

        let response_json = serde_json::to_vec(&response)?;
        stream.write(&response_json).await?;

        info!("Sent response: {:?}", response);

        Ok(())
    }
}

impl Drop for DaemonServer {
    fn drop(&mut self) {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}
