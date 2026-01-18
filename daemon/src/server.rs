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
use crate::transcription::streaming_engine::StreamingEngine;

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

    /// Helper to handle the logic for starting audio processing.
    /// Used by Command::Start and Command::Toggle.
    async fn handle_start(state: Arc<Mutex<DaemonState>>) -> anyhow::Result<Response> {
        let mut state_guard = state.lock().await;
        state_guard.activate().await?;

        if *state_guard.is_processing.lock().await {
            return Err(anyhow::anyhow!("Already processing audio"));
        }

        let use_streaming = state_guard.config.whisper.streaming_mode;

        if use_streaming {
            if state_guard.streaming_engine.lock().await.is_none() {
                let model_path = crate::transcription::engine::WhisperEngine::find_model_path(
                    &state_guard.config.whisper.model_url,
                )?;

                let model_path_str = model_path.to_string_lossy().to_string();

                let mut streaming_engine = StreamingEngine::new(
                    model_path_str.clone(),
                    state_guard.config.whisper.language.clone(),
                    state_guard.config.streaming.step_ms,
                    state_guard.config.streaming.length_ms,
                    state_guard.config.streaming.keep_ms,
                );
                streaming_engine.load_model(&model_path_str).await?;
                *state_guard.streaming_engine.lock().await = Some(streaming_engine);
                info!("Streaming engine loaded");
            }
        } else {
            if state_guard.whisper_engine.lock().await.is_none() {
                let mut whisper_engine = WhisperEngine::new(
                    state_guard.config.whisper.model_url.clone(),
                    state_guard.config.whisper.backend.clone(),
                )?;
                whisper_engine.load_model().await?;
                *state_guard.whisper_engine.lock().await = Some(whisper_engine);
                info!("Whisper engine loaded into memory");
            }
        }

        if state_guard.virtual_keyboard.lock().await.is_none() {
            let virtual_keyboard = VirtualKeyboard::new()?;
            *state_guard.virtual_keyboard.lock().await = Some(virtual_keyboard);
        }

        let (audio_tx, audio_rx) = tokio::sync::broadcast::channel(100);
        let mut new_capture = AudioCapture::new()?;
        new_capture.start(audio_tx)?;
        *state_guard.audio_capture.lock().await = Some(new_capture);
        *state_guard.audio_rx.lock().await = Some(audio_rx);

        debug!("Audio capture started, VAD, Whisper, and Keyboard ready");

        if use_streaming {
            let mut engine_lock = state_guard.streaming_engine.lock().await;
            if let Some(ref mut engine) = *engine_lock {
                engine.start()?;
                info!("Streaming engine started");
            }
            debug!("Audio capture started, starting streaming processing");
            if let Err(e) = state_guard.start_streaming_processing().await {
                error!("Failed to start streaming processing: {}", e);
                return Err(anyhow::anyhow!("{}", e));
            }
        } else {
            debug!("Audio capture started, starting VAD and Whisper processing");
            if let Err(e) = state_guard.start_vad_processing().await {
                error!("Failed to start VAD and Whisper processing: {}", e);
                return Err(anyhow::anyhow!("{}", e));
            }
        }

        let mode = if use_streaming { "streaming" } else { "batch" };
        info!("Activated audio capture ({} mode)", mode);
        Ok(Response::Ok)
    }

    /// Helper to handle the logic for stopping audio processing.
    /// Used by Command::Stop and Command::Toggle.
    async fn handle_stop(state: Arc<Mutex<DaemonState>>) -> anyhow::Result<Response> {
        let mut state_guard = state.lock().await;
        state_guard.stop_vad_processing().await;
        if let Some(capture) = state_guard.audio_capture.lock().await.as_mut() {
            capture.stop().await?;
        }
        *state_guard.audio_capture.lock().await = None;
        *state_guard.audio_rx.lock().await = None;
        state_guard.deactivate().await?;
        info!("Stopped audio processing, model kept in memory");
        Ok(Response::Ok)
    }

    pub async fn execute_command(
        state: Arc<Mutex<DaemonState>>,
        command: Command,
    ) -> anyhow::Result<Response> {
        info!("Received command: {:?}", command);

        let response = match command {
            Command::Start => Self::handle_start(state).await?,
            Command::Stop => Self::handle_stop(state).await?,
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
            Command::Toggle => {
                let status = state.lock().await.get_status().await;

                if status.is_active {
                    info!("Toggling: active -> stopping");
                    Self::handle_stop(state).await?
                } else {
                    info!("Toggling: inactive -> starting");
                    Self::handle_start(state).await?
                }
            }
        };

        Ok(response)
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

        let response = Self::execute_command(state.clone(), command).await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_daemon_server_new() {
        let socket_path = PathBuf::from("/tmp/test.sock");
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        let server = DaemonServer::new(socket_path.clone(), state);

        assert_eq!(server.socket_path, socket_path);
    }

    #[tokio::test]
    async fn test_execute_command_pause() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        let result = DaemonServer::execute_command(state.clone(), Command::Pause).await;
        assert!(matches!(result, Ok(Response::Ok)));
    }

    #[tokio::test]
    async fn test_execute_command_resume() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        let result = DaemonServer::execute_command(state.clone(), Command::Resume).await;
        assert!(matches!(result, Ok(Response::Ok)));
    }

    #[tokio::test]
    async fn test_execute_command_status() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config.clone())));

        let result = DaemonServer::execute_command(state.clone(), Command::Status).await;

        if let Ok(Response::Status(info)) = result {
            assert_eq!(info.is_running, true);
            assert_eq!(info.is_active, false);
            assert_eq!(info.language, config.whisper.language);
        } else {
            panic!("Expected Status response");
        }
    }

    #[tokio::test]
    async fn test_execute_command_status_active() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config.clone())));

        {
            let mut state_guard = state.lock().await;
            state_guard.activate().await.unwrap();
        }

        let result = DaemonServer::execute_command(state.clone(), Command::Status).await;

        if let Ok(Response::Status(info)) = result {
            assert_eq!(info.is_running, true);
            assert_eq!(info.is_active, true);
            assert_eq!(info.language, config.whisper.language);
        } else {
            panic!("Expected Status response");
        }
    }

    #[tokio::test]
    async fn test_execute_command_set_language() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        let result =
            DaemonServer::execute_command(state.clone(), Command::SetLanguage("es".to_string()))
                .await;
        assert!(matches!(result, Ok(Response::Ok)));
    }

    #[tokio::test]
    async fn test_execute_command_set_language_multiple() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        let languages = vec!["en", "es", "fr", "de"];
        for lang in languages {
            let result = DaemonServer::execute_command(
                state.clone(),
                Command::SetLanguage(lang.to_string()),
            )
            .await;
            assert!(matches!(result, Ok(Response::Ok)));
        }
    }

    #[tokio::test]
    #[ignore = "Toggle command requires real audio/hardware, cannot test reliably without mocking"]
    async fn test_execute_command_toggle_inactive() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        let result = DaemonServer::execute_command(state.clone(), Command::Toggle).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore = "Toggle command requires real audio/hardware, cannot test reliably without mocking"]
    async fn test_execute_command_toggle_active() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        let mut state_guard = state.lock().await;
        state_guard.activate().await.unwrap();

        let result = DaemonServer::execute_command(state.clone(), Command::Toggle).await;

        assert!(result.is_err());
    }
}
