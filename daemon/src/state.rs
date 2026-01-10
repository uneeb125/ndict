use crate::audio::capture::AudioCapture;
use crate::config::Config;
use crate::output::VirtualKeyboard;
use crate::transcription;
use crate::transcription::engine::WhisperEngine;
use crate::vad::speech_detector::SpeechDetector;
use shared::ipc::StatusInfo;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct DaemonState {
    pub config: Config,
    pub is_active: Arc<Mutex<bool>>,
    pub audio_capture: Arc<Mutex<Option<AudioCapture>>>,
    pub speech_detector: Arc<Mutex<Option<SpeechDetector>>>,
    pub audio_rx: Arc<Mutex<Option<broadcast::Receiver<Vec<f32>>>>>,
    pub whisper_engine: Arc<Mutex<Option<WhisperEngine>>>,
    pub virtual_keyboard: Arc<Mutex<Option<VirtualKeyboard>>>,
    pub vad_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl DaemonState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            is_active: Arc::new(Mutex::new(false)),
            audio_capture: Arc::new(Mutex::new(None)),
            speech_detector: Arc::new(Mutex::new(None)),
            audio_rx: Arc::new(Mutex::new(None)),
            whisper_engine: Arc::new(Mutex::new(None)),
            virtual_keyboard: Arc::new(Mutex::new(None)),
            vad_task_handle: Arc::new(Mutex::new(None)),
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
        let audio_rx_option: Option<broadcast::Receiver<Vec<f32>>> =
            self.audio_rx.lock().await.take();
        let whisper_engine = self.whisper_engine.clone();
        let virtual_keyboard = self.virtual_keyboard.clone();
        let vad_threshold = self.config.vad.threshold;
        let silence_duration_ms = self.config.vad.min_silence_duration_ms;
        let gain = self.config.audio.gain;

        if audio_rx_option.is_none() {
            return Err(anyhow::anyhow!("Audio receiver not available"));
        }

        let mut audio_rx = audio_rx_option.unwrap();
        let vad_task = tokio::spawn(async move {
            tracing::info!("VAD processing task started");

            let mut speech_detector =
                SpeechDetector::new(vad_threshold, silence_duration_ms, gain).unwrap();

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
        });

        *self.vad_task_handle.lock().await = Some(vad_task);
        Ok(())
    }

    pub async fn stop_vad_processing(&self) {
        if let Some(handle) = self.vad_task_handle.lock().await.take() {
            handle.abort();
            tracing::info!("VAD processing task stopped");
        }
    }
}
