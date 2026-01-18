use crate::audio::capture::AudioCapture;
use crate::config::Config;
use crate::output::VirtualKeyboard;
use crate::transcription;
use crate::transcription::engine::WhisperEngine;
use crate::transcription::streaming_engine::StreamingEngine;
use crate::vad::speech_detector::SpeechDetector;
use shared::ipc::StatusInfo;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct DaemonState {
    pub config: Config,
    pub is_active: Arc<Mutex<bool>>,
    pub is_processing: Arc<Mutex<bool>>,
    pub audio_capture: Arc<Mutex<Option<AudioCapture>>>,
    pub audio_rx: Arc<Mutex<Option<broadcast::Receiver<Vec<f32>>>>>,
    pub whisper_engine: Arc<Mutex<Option<WhisperEngine>>>,
    pub streaming_engine: Arc<Mutex<Option<StreamingEngine>>>,
    pub virtual_keyboard: Arc<Mutex<Option<VirtualKeyboard>>>,
    pub vad_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub streaming_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl DaemonState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            is_active: Arc::new(Mutex::new(false)),
            is_processing: Arc::new(Mutex::new(false)),
            audio_capture: Arc::new(Mutex::new(None)),
            audio_rx: Arc::new(Mutex::new(None)),
            whisper_engine: Arc::new(Mutex::new(None)),
            streaming_engine: Arc::new(Mutex::new(None)),
            virtual_keyboard: Arc::new(Mutex::new(None)),
            vad_task_handle: Arc::new(Mutex::new(None)),
            streaming_task_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn activate(&mut self) -> anyhow::Result<()> {
        *self.is_active.lock().await = true;
        tracing::info!("Daemon activated");
        Ok(())
    }

    pub async fn deactivate(&mut self) -> anyhow::Result<()> {
        *self.is_active.lock().await = false;
        tracing::info!("Daemon deactivated");
        Ok(())
    }

    pub async fn get_status(&self) -> StatusInfo {
        let is_active = *self.is_active.lock().await;
        StatusInfo {
            is_running: true,
            is_active,
            language: self.config.whisper.language.clone(),
        }
    }

    pub async fn start_vad_processing(&self) -> anyhow::Result<()> {
        let is_processing = *self.is_processing.lock().await;
        if is_processing {
            return Err(anyhow::anyhow!("Already processing audio"));
        }

        let audio_rx_option: Option<broadcast::Receiver<Vec<f32>>> =
            self.audio_rx.lock().await.take();
        let whisper_engine = self.whisper_engine.clone();
        let virtual_keyboard = self.virtual_keyboard.clone();
        let vad_threshold_start = self.config.vad.threshold_start;
        let vad_threshold_stop = self.config.vad.threshold_stop;
        let silence_duration_ms = self.config.vad.min_silence_duration_ms;
        let gain = self.config.audio.gain;

        if audio_rx_option.is_none() {
            return Err(anyhow::anyhow!("Audio receiver not available"));
        }

        let mut audio_rx = audio_rx_option.unwrap();
        let is_processing_flag = self.is_processing.clone();

        let vad_task = tokio::spawn(async move {
            *is_processing_flag.lock().await = true;

            tracing::info!("VAD processing task started");

            let mut speech_detector = SpeechDetector::new(
                vad_threshold_start,
                vad_threshold_stop,
                silence_duration_ms,
                gain,
            )
            .unwrap();

            loop {
                match audio_rx.recv().await {
                    Ok(samples) => {
                        tracing::debug!(
                            "Received audio chunk: {} samples, first 3 values: {:.4}, {:.4}, {:.4}",
                            samples.len(),
                            samples.first().unwrap_or(&0.0),
                            samples.get(1).unwrap_or(&0.0),
                            samples.get(2).unwrap_or(&0.0)
                        );
                        if let Some(speech_audio) = speech_detector.process_audio(&samples) {
                            tracing::info!(
                                "Speech detected, starting transcription: {} samples",
                                speech_audio.len()
                            );

                            let engine_ref = whisper_engine.clone();
                            let keyboard_ref = virtual_keyboard.clone();
                            tokio::spawn(async move {
                                tracing::debug!(
                                    "Starting Whisper transcription for {} samples",
                                    speech_audio.len()
                                );

                                let transcription_result = tokio::time::timeout(
                                    tokio::time::Duration::from_secs(30),
                                    async {
                                        let mut engine_lock = engine_ref.lock().await;
                                        if let Some(ref mut engine) = *engine_lock {
                                            engine.transcribe(&speech_audio).await
                                        } else {
                                            Err(anyhow::anyhow!("Whisper engine not available"))
                                        }
                                    },
                                )
                                .await;

                                match transcription_result {
                                    Ok(Ok(text)) => {
                                        tracing::debug!("Finished Whisper transcription");
                                        let post_processed =
                                            transcription::post_process_transcription(&text);
                                        tracing::info!(
                                            "Transcription result: '{}'",
                                            post_processed
                                        );

                                        let mut keyboard_lock = keyboard_ref.lock().await;
                                        if let Some(ref mut keyboard) = *keyboard_lock {
                                            tracing::debug!(
                                                "Starting keyboard typing for: '{}'",
                                                post_processed
                                            );
                                            let typing_result = tokio::time::timeout(
                                                tokio::time::Duration::from_secs(5),
                                                async {
                                                    let result =
                                                        keyboard.type_text(&post_processed);
                                                    Ok::<_, anyhow::Error>(result)
                                                },
                                            )
                                            .await;

                                            match typing_result {
                                                Ok(Ok(_)) => {
                                                    tracing::info!("Successfully typed text");
                                                    tracing::debug!("Finished keyboard typing");
                                                }
                                                Ok(Err(e)) => {
                                                    tracing::error!("Keyboard typing error: {}", e);
                                                }
                                                Err(_) => {
                                                    tracing::error!(
                                                        "Keyboard typing operation timed out after 5 seconds"
                                                    );
                                                }
                                            }
                                        } else {
                                            tracing::warn!("Virtual keyboard not available");
                                        }
                                    }
                                    Ok(Err(e)) => {
                                        tracing::error!("Transcription error: {}", e);
                                        tracing::debug!("Whisper transcription failed");
                                    }
                                    Err(_) => {
                                        tracing::error!(
                                            "Whisper transcription operation timed out after 30 seconds"
                                        );
                                    }
                                }
                            });
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("VAD lagged, dropped {} audio chunks", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("Audio receiver closed, stopping VAD processing");
                        break;
                    }
                }
            }

            *is_processing_flag.lock().await = false;
        });

        *self.vad_task_handle.lock().await = Some(vad_task);
        Ok(())
    }

    pub async fn start_streaming_processing(&self) -> anyhow::Result<()> {
        let is_processing = *self.is_processing.lock().await;
        if is_processing {
            return Err(anyhow::anyhow!("Already processing audio"));
        }

        let audio_rx_option: Option<broadcast::Receiver<Vec<f32>>> =
            self.audio_rx.lock().await.take();
        let streaming_engine = self.streaming_engine.clone();
        let virtual_keyboard = self.virtual_keyboard.clone();

        if audio_rx_option.is_none() {
            return Err(anyhow::anyhow!("Audio receiver not available"));
        }

        let mut audio_rx = audio_rx_option.unwrap();
        let is_processing_flag = self.is_processing.clone();

        let streaming_task = tokio::spawn(async move {
            *is_processing_flag.lock().await = true;

            tracing::info!("Streaming processing task started");

            loop {
                match audio_rx.recv().await {
                    Ok(samples) => {
                        tracing::debug!("Received audio chunk: {} samples", samples.len());

                        let mut engine_lock = streaming_engine.lock().await;
                        if let Some(ref mut engine) = *engine_lock {
                            match engine.send_audio(&samples) {
                                Ok(Some(text)) => {
                                    tracing::info!("Streaming transcription: '{}'", text);

                                    let post_processed =
                                        transcription::post_process_transcription(&text);

                                    let mut keyboard_lock = virtual_keyboard.lock().await;
                                    if let Some(ref mut keyboard) = *keyboard_lock {
                                        if let Err(e) = keyboard.type_text(&post_processed) {
                                            tracing::error!("Keyboard typing error: {}", e);
                                        }
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to send audio to streaming engine: {}",
                                        e
                                    );
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Streaming lagged, dropped {} audio chunks", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("Audio receiver closed, stopping streaming processing");
                        break;
                    }
                }
            }

            *is_processing_flag.lock().await = false;
        });

        *self.streaming_task_handle.lock().await = Some(streaming_task);
        Ok(())
    }

    pub async fn stop_vad_processing(&self) {
        *self.is_processing.lock().await = false;

        if let Some(mut streaming_engine) = self.streaming_engine.lock().await.take() {
            streaming_engine.stop().await;
            tracing::info!("Streaming engine stopped");
        }

        if let Some(handle) = self.vad_task_handle.lock().await.take() {
            handle.abort();
            tracing::info!("VAD processing task stopped");
        }

        if let Some(handle) = self.streaming_task_handle.lock().await.take() {
            handle.abort();
            tracing::info!("Streaming task stopped");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daemon_state_new() {
        let config = Config::default();
        let state = DaemonState::new(config.clone());

        assert_eq!(state.config, config);
        assert!(!*state.is_active.lock().await);
        assert!(!*state.is_processing.lock().await);
        assert!(state.audio_capture.lock().await.is_none());
        assert!(state.audio_rx.lock().await.is_none());
        assert!(state.whisper_engine.lock().await.is_none());
        assert!(state.virtual_keyboard.lock().await.is_none());
        assert!(state.vad_task_handle.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_activate() {
        let config = Config::default();
        let mut state = DaemonState::new(config);

        assert!(!*state.is_active.lock().await);

        state.activate().await.unwrap();

        assert!(*state.is_active.lock().await);
    }

    #[tokio::test]
    async fn test_deactivate() {
        let config = Config::default();
        let mut state = DaemonState::new(config);

        state.activate().await.unwrap();
        assert!(*state.is_active.lock().await);

        state.deactivate().await.unwrap();

        assert!(!*state.is_active.lock().await);
    }

    #[tokio::test]
    async fn test_get_status() {
        let config = Config::default();
        let state = DaemonState::new(config.clone());

        let status = state.get_status().await;

        assert_eq!(status.is_running, true);
        assert_eq!(status.is_active, false);
        assert_eq!(status.language, config.whisper.language);
    }

    #[tokio::test]
    async fn test_get_status_active() {
        let config = Config::default();
        let mut state = DaemonState::new(config.clone());

        state.activate().await.unwrap();

        let status = state.get_status().await;

        assert_eq!(status.is_running, true);
        assert_eq!(status.is_active, true);
        assert_eq!(status.language, config.whisper.language);
    }

    #[tokio::test]
    async fn test_stop_vad_processing() {
        let config = Config::default();
        let state = DaemonState::new(config);

        let handle = tokio::spawn(async {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });
        *state.vad_task_handle.lock().await = Some(handle);

        assert!(state.vad_task_handle.lock().await.is_some());

        state.stop_vad_processing().await;

        assert!(state.vad_task_handle.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_stop_vad_processing_no_task() {
        let config = Config::default();
        let state = DaemonState::new(config);

        assert!(state.vad_task_handle.lock().await.is_none());

        state.stop_vad_processing().await;

        assert!(state.vad_task_handle.lock().await.is_none());
    }
}
