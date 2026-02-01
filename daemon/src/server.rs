 use shared::ipc::{Command, Response};
 use std::path::PathBuf;
 use std::sync::Arc;
 use tokio::io::{AsyncReadExt, AsyncWriteExt};
 use tokio::net::UnixListener;
 use tokio::sync::Mutex;
 use tokio::time::{timeout, Duration};
 use tracing::{debug, error, info, warn};

use crate::audio::capture::AudioCapture;
use crate::output::keyboard::VirtualKeyboard;
use crate::state::DaemonState;
 use crate::transcription::engine::WhisperEngine;
 use crate::transcription::streaming_engine::StreamingEngine;

 /// Timeout for accepting new connections (10 seconds)
 const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);

 /// Timeout for read/write operations on connections (10 seconds)
 const IO_TIMEOUT: Duration = Duration::from_secs(10);

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

        // Set restrictive permissions on the socket (read/write for owner only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&socket_path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&socket_path, perms)?;
            debug!("Set socket permissions to 0600");
        }

        loop {
            debug!("Waiting for connection...");
            let state = Arc::clone(&self.state);

            match timeout(ACCEPT_TIMEOUT, listener.accept()).await {
                Ok(Ok((stream, _addr))) => {
                    debug!("Connection accepted");
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(state, stream).await {
                            error!("Error handling connection: {}", e);
                        } else {
                            debug!("Connection handled successfully");
                        }
                    });
                }
                Ok(Err(e)) => {
                    error!("Error accepting connection: {}", e);
                }
                Err(_) => {
                    // Timeout - continue waiting for connections
                    debug!("Accept timeout, continuing to wait...");
                    continue;
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
                    state_guard.config.audio.sample_rate,
                );
                streaming_engine.load_model(&model_path_str).await?;
                *state_guard.streaming_engine.lock().await = Some(streaming_engine);
                info!("Streaming engine loaded");
            }
        } else {
            if state_guard.whisper_engine.lock().await.is_none() {
                let mut whisper_engine = WhisperEngine::new_with_checksum_and_params(
                    state_guard.config.whisper.model_url.clone(),
                    state_guard.config.whisper.backend.clone(),
                    state_guard.config.whisper.model_checksum.clone(),
                    state_guard.config.whisper.min_audio_samples,
                    state_guard.config.whisper.sampling_strategy.clone(),
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

        let (audio_tx, audio_rx) = tokio::sync::broadcast::channel(state_guard.config.buffer.broadcast_capacity);
        let sample_rate = state_guard.config.audio.sample_rate;
        let channels = state_guard.config.audio.channels;
        let mut new_capture = AudioCapture::new_with_channels(sample_rate, channels)?;
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

    /// Helper to handle the logic for pausing transcription.
    /// Stops VAD processing and sets is_active to false, but keeps audio capture running.
    async fn handle_pause(state: Arc<Mutex<DaemonState>>) -> anyhow::Result<Response> {
        let mut state_guard = state.lock().await;

        let is_active = *state_guard.is_active.lock().await;
        if !is_active {
            return Err(anyhow::anyhow!("Already paused or not started"));
        }

        state_guard.stop_vad_processing().await;
        state_guard.deactivate().await?;
        info!("Paused transcription, audio capture continues");
        Ok(Response::Ok)
    }

    /// Helper to handle the logic for resuming transcription.
    /// Sets is_active to true and restarts VAD or streaming processing.
    async fn handle_resume(state: Arc<Mutex<DaemonState>>) -> anyhow::Result<Response> {
        let mut state_guard = state.lock().await;

        let is_active = *state_guard.is_active.lock().await;
        if is_active {
            return Err(anyhow::anyhow!("Already active, cannot resume"));
        }

        let has_audio = state_guard.audio_capture.lock().await.is_some();
        if !has_audio {
            return Err(anyhow::anyhow!("Cannot resume: audio capture not running. Use Start instead."));
        }

        let use_streaming = state_guard.config.whisper.streaming_mode;

        if use_streaming {
            state_guard.start_streaming_processing().await?;
        } else {
            state_guard.start_vad_processing().await?;
        }

        state_guard.activate().await?;
        info!("Resumed transcription");
        Ok(Response::Ok)
    }

    /// Helper to handle the logic for setting language.
    /// Validates and stores the language in DaemonState.
    async fn handle_set_language(state: Arc<Mutex<DaemonState>>, lang: String) -> anyhow::Result<Response> {
        // Validate language code (basic validation: 2-3 letter ISO 639-1 codes)
        if lang.len() < 2 || lang.len() > 3 {
            return Err(anyhow::anyhow!("Invalid language code: '{}'. Expected 2-3 letter ISO 639-1 code (e.g., 'en', 'es', 'fr')", lang));
        }

        if !lang.chars().all(|c| c.is_ascii_lowercase()) {
            return Err(anyhow::anyhow!("Invalid language code: '{}'. Must be lowercase ASCII letters only", lang));
        }

        let state_guard = state.lock().await;
        *state_guard.language.lock().await = lang.clone();

        // Update streaming engine language if it's loaded
        if let Some(ref mut engine) = *state_guard.streaming_engine.lock().await {
            engine.set_language(lang.clone());
        }

        info!("Language set to: {}", lang);
        Ok(Response::Ok)
    }

    pub async fn execute_command(
        state: Arc<Mutex<DaemonState>>,
        command: Command,
    ) -> anyhow::Result<Response> {
        info!("Received command: {:?}", command);

        // Check rate limit before processing the command
        let rate_limiter = {
            let state_guard = state.lock().await;
            state_guard.get_rate_limiter()
        };

        if !rate_limiter.check() {
            warn!("Command rate limited: {:?}", command);
            return Ok(Response::Error(
                "Rate limit exceeded. Please wait before sending more commands.".to_string(),
            ));
        }

        let response = match command {
            Command::Start => Self::handle_start(state).await?,
            Command::Stop => Self::handle_stop(state).await?,
            Command::Pause => Self::handle_pause(state).await?,
            Command::Resume => Self::handle_resume(state).await?,
            Command::Status => {
                let status = state.lock().await.get_status().await;
                Response::Status(status)
            }
            Command::SetLanguage(lang) => Self::handle_set_language(state, lang).await?,
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
        // Read command with timeout
        let mut buffer = vec![0u8; 1024];
        let n = match timeout(IO_TIMEOUT, stream.read(&mut buffer)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
                warn!("Connection read error: {}", e);
                return Err(e.into());
            }
            Err(_) => {
                warn!("Read timeout: failed to read command from client within {:?}", IO_TIMEOUT);
                return Err(anyhow::anyhow!("Connection timeout during read"));
            }
        };

        if n == 0 {
            debug!("Connection closed by client");
            return Ok(());
        }

        buffer.truncate(n);

        let command: Command = match serde_json::from_slice(&buffer) {
            Ok(cmd) => cmd,
            Err(e) => {
                warn!("Failed to deserialize command: {}", e);
                return Err(e.into());
            }
        };

        let response = Self::execute_command(state.clone(), command).await?;

        let response_json = serde_json::to_vec(&response)?;

        // Write response with timeout
        if timeout(IO_TIMEOUT, stream.write_all(&response_json)).await.is_err() {
            warn!("Write timeout: failed to send response to client within {:?}", IO_TIMEOUT);
            return Err(anyhow::anyhow!("Connection timeout during write"));
        }

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

        // Pause when not active should fail
        let result = DaemonServer::execute_command(state.clone(), Command::Pause).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Already paused"));

        // Activate first
        {
            let mut state_guard = state.lock().await;
            state_guard.activate().await.unwrap();
        }

        // Now pause should succeed
        let result = DaemonServer::execute_command(state.clone(), Command::Pause).await;
        assert!(matches!(result, Ok(Response::Ok)));

        // Verify it's no longer active
        let status = state.lock().await.get_status().await;
        assert!(!status.is_active);
    }

    #[tokio::test]
    async fn test_execute_command_resume() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        // Resume when already active should fail
        {
            let mut state_guard = state.lock().await;
            state_guard.activate().await.unwrap();
        }
        let result = DaemonServer::execute_command(state.clone(), Command::Resume).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Already active"));

        // Deactivate
        {
            let mut state_guard = state.lock().await;
            state_guard.deactivate().await.unwrap();
        }

        // Resume without audio capture should fail
        let result = DaemonServer::execute_command(state.clone(), Command::Resume).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("audio capture not running"));
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

        // Verify language was set
        let status = state.lock().await.get_status().await;
        assert_eq!(status.language, "es");
    }

    #[tokio::test]
    async fn test_execute_command_set_language_multiple() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        let languages = vec!["en", "es", "fr", "de", "jp", "zh"];
        for lang in languages {
            let result = DaemonServer::execute_command(
                state.clone(),
                Command::SetLanguage(lang.to_string()),
            )
            .await;
            assert!(matches!(result, Ok(Response::Ok)));

            // Verify language was updated
            let status = state.lock().await.get_status().await;
            assert_eq!(status.language, lang);
        }
    }

    #[tokio::test]
    async fn test_execute_command_set_language_invalid() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        // Too short
        let result =
            DaemonServer::execute_command(state.clone(), Command::SetLanguage("a".to_string()))
                .await;
        assert!(result.is_err());

        // Too long
        let result =
            DaemonServer::execute_command(state.clone(), Command::SetLanguage("abcd".to_string()))
                .await;
        assert!(result.is_err());

        // Uppercase
        let result =
            DaemonServer::execute_command(state.clone(), Command::SetLanguage("EN".to_string()))
                .await;
        assert!(result.is_err());

        // Invalid characters
        let result =
            DaemonServer::execute_command(state.clone(), Command::SetLanguage("e1".to_string()))
                .await;
        assert!(result.is_err());
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

    #[tokio::test]
    async fn test_rate_limit_allows_normal_usage() {
        let mut config = Config::default();
        config.rate_limit.commands_per_second = 10;
        config.rate_limit.burst_capacity = 20;
        config.rate_limit.enabled = true;

        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        // Send 5 commands, all should be allowed
        for _ in 0..5 {
            let result = DaemonServer::execute_command(state.clone(), Command::Status).await;
            assert!(matches!(result, Ok(Response::Status(_))));
        }
    }

    #[tokio::test]
    async fn test_rate_limit_allows_burst() {
        let mut config = Config::default();
        config.rate_limit.commands_per_second = 10;
        config.rate_limit.burst_capacity = 5;
        config.rate_limit.enabled = true;

        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        // Send 5 commands in burst, all should be allowed
        for _ in 0..5 {
            let result = DaemonServer::execute_command(state.clone(), Command::Status).await;
            assert!(matches!(result, Ok(Response::Status(_))), "Burst capacity should allow 5 requests");
        }

        // Next request should be rate limited
        let result = DaemonServer::execute_command(state.clone(), Command::Status).await;
        assert!(matches!(result, Ok(Response::Error(_))), "Should be rate limited after burst exhausted");
    }

    #[tokio::test]
    async fn test_rate_limit_disabled() {
        let mut config = Config::default();
        config.rate_limit.commands_per_second = 1;
        config.rate_limit.burst_capacity = 1;
        config.rate_limit.enabled = false;

        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        // Even with very low limits, disabled rate limiting should allow all requests
        for _ in 0..50 {
            let result = DaemonServer::execute_command(state.clone(), Command::Status).await;
            assert!(matches!(result, Ok(Response::Status(_))), "Disabled rate limiting should allow all requests");
        }
    }

    #[tokio::test]
    async fn test_rate_limit_returns_error_message() {
        let mut config = Config::default();
        config.rate_limit.commands_per_second = 10;
        config.rate_limit.burst_capacity = 2;
        config.rate_limit.enabled = true;

        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        // Exhaust burst capacity
        let _ = DaemonServer::execute_command(state.clone(), Command::Status).await;
        let _ = DaemonServer::execute_command(state.clone(), Command::Status).await;

        // Next request should be rate limited with error message
        let result = DaemonServer::execute_command(state.clone(), Command::Status).await;

        if let Ok(Response::Error(msg)) = result {
            assert!(msg.contains("Rate limit exceeded"), "Error message should mention rate limiting");
            assert!(msg.contains("wait"), "Error message should mention waiting");
        } else {
            panic!("Expected Error response when rate limited, got {:?}", result);
        }
    }

    #[tokio::test]
    async fn test_rate_limit_affects_all_commands() {
        let mut config = Config::default();
        config.rate_limit.commands_per_second = 10;
        config.rate_limit.burst_capacity = 3;
        config.rate_limit.enabled = true;

        let state = Arc::new(Mutex::new(DaemonState::new(config)));

        // Mix of commands should all be rate limited together
        let _ = DaemonServer::execute_command(state.clone(), Command::Status).await;
        let _ = DaemonServer::execute_command(state.clone(), Command::Status).await;
        let _ = DaemonServer::execute_command(state.clone(), Command::Status).await;

        // All commands share the same rate limiter
        let result = DaemonServer::execute_command(state.clone(), Command::Status).await;
        assert!(matches!(result, Ok(Response::Error(_))), "All commands should be rate limited together");

        let result = DaemonServer::execute_command(state.clone(), Command::SetLanguage("es".to_string())).await;
        assert!(matches!(result, Ok(Response::Error(_))), "SetLanguage should also be rate limited");
    }
}
