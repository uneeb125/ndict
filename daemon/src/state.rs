use crate::audio::capture::AudioCapture;
use crate::output::VirtualKeyboard;
use crate::transcription;
use crate::transcription::engine::WhisperEngine;
use crate::vad::speech_detector::SpeechDetector;
use shared::ipc::StatusInfo;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

const VAD_THRESHOLD: f32 = 0.01;
const SILENCE_DURATION_MS: u32 = 1000;

pub struct DaemonState {
    pub is_active: Arc<Mutex<bool>>,
    pub audio_capture: Arc<Mutex<Option<AudioCapture>>>,
    pub speech_detector: Arc<Mutex<Option<SpeechDetector>>>,
    pub audio_rx: Arc<Mutex<Option<broadcast::Receiver<Vec<f32>>>>>,
    pub whisper_engine: Arc<Mutex<Option<WhisperEngine>>>,
    pub virtual_keyboard: Arc<Mutex<Option<VirtualKeyboard>>>,
    pub vad_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
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
            language: "auto".to_string(),
        }
    }

    pub async fn start_vad_processing(&self) -> anyhow::Result<()> {
        let audio_rx_option: Option<broadcast::Receiver<Vec<f32>>> =
            self.audio_rx.lock().await.take();
        let whisper_engine = self.whisper_engine.clone();
        let virtual_keyboard = self.virtual_keyboard.clone();

        if audio_rx_option.is_none() {
            return Err(anyhow::anyhow!("Audio receiver not available"));
        }

        let mut audio_rx = audio_rx_option.unwrap();
        let vad_task = tokio::spawn(async move {
            tracing::info!("VAD processing task started");

            let mut speech_detector =
                SpeechDetector::new(VAD_THRESHOLD, SILENCE_DURATION_MS).unwrap();

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
                                "Speech detected, would transcribe: {} samples",
                                speech_audio.len()
                            );

                            let engine_ref = whisper_engine.clone();
                            let keyboard_ref = virtual_keyboard.clone();
                            tokio::spawn(async move {
                                let mut engine_lock = engine_ref.lock().await;
                                if let Some(ref mut engine) = *engine_lock {
                                    match engine.transcribe(&speech_audio).await {
                                        Ok(text) => {
                                            let post_processed =
                                                transcription::post_process_transcription(&text);
                                            tracing::info!(
                                                "Transcription result: '{}'",
                                                post_processed
                                            );

                                            let mut keyboard_lock = keyboard_ref.lock().await;
                                            if let Some(ref mut keyboard) = *keyboard_lock {
                                                if let Err(e) = keyboard.type_text(&post_processed)
                                                {
                                                    tracing::error!("Keyboard typing error: {}", e);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("Transcription error: {}", e);
                                        }
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
