use crate::{
    audio::capture::AudioCapture, vad::speech_detector::SpeechDetector,
    whisper::engine::WhisperEngine,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct DaemonState {
    pub is_active: Arc<Mutex<bool>>,
    pub audio_capture: Arc<Mutex<Option<AudioCapture>>>,
    pub speech_detector: Arc<Mutex<Option<SpeechDetector>>>,
    pub audio_rx: Arc<Mutex<Option<tokio::sync::broadcast::Receiver<Vec<f32>>>>>,
    pub whisper_engine: Arc<Mutex<Option<engine::WhisperEngine>>>,
    pub vad_task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(Mutex::new(false)),
            audio_capture: Arc::new(Mutex::new(None)),
            speech_detector: Arc::new(Mutex::new(None)),
            audio_rx: Arc::new(Mutex::new(None)),
            whisper_engine: Arc::new(Mutex::new(None)),
            vad_task_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn activate(&self) -> anyhow::Result<()> {
        let mut is_active = self.is_active.lock().await;
        if *is_active {
            return Ok(());
        }

        let mut audio_capture = self.audio_capture.lock().await;
        if audio_capture.is_some() {
            return Ok(());
        }

        *audio_capture = Some(AudioCapture::new()?);
        *is_active = true;

        drop(is_active);
        drop(audio_capture);

        tracing::info!("Activation complete, VAD processing loop will start");

        Ok(())
    }

    pub async fn deactivate(&self) -> anyhow::Result<()> {
        let mut is_active = self.is_active.lock().await;
        if !*is_active {
            return Ok(());
        }

        if let Some(mut capture) = self.audio_capture.lock().await.take() {
            capture.stop()?;
        }

        *self.speech_detector.lock().await = None;
        *self.audio_rx.lock().await = None;
        *self.whisper_engine.lock().await = None;
        *is_active = false;

        drop(is_active);

        Ok(())
    }

    pub async fn get_status(&self) -> shared::ipc::StatusInfo {
        let is_active = *self.is_active.lock().await;

        shared::ipc::StatusInfo {
            is_running: true,
            is_active,
            language: String::from("auto"),
        }
    }

    pub async fn start_vad_processing(&self) -> anyhow::Result<()> {
        let mut audio_rx = self.audio_rx.lock().await;
        let mut speech_detector = self.speech_detector.lock().await;
        let mut whisper_engine = self.whisper_engine.lock().await;
        let vad_task_handle = self.vad_task_handle.lock().await;

        if audio_rx.is_none() || speech_detector.is_none() || whisper_engine.is_none() {
            tracing::warn!(
                "Cannot start VAD processing: audio_rx, speech_detector, or whisper_engine is None"
            );
            return Ok(());
        }

        if vad_task_handle.is_some() {
            tracing::warn!("VAD processing already running");
            return Ok(());
        }

        let audio_rx = audio_rx.take().unwrap();
        let mut speech_detector = speech_detector.take().unwrap();
        let mut whisper_engine = whisper_engine.take().unwrap();

        let is_active = Arc::clone(&self.is_active);

        tracing::info!("Starting VAD and Whisper processing loop");

        let handle = tokio::spawn(async move {
            while *is_active.lock().await {
                tokio::select! {
                    _ = audio_rx.recv() => {
                        if let Some(samples) = audio_rx.recv().await {
                            if let Some(speech) = speech_detector.process_audio(&samples) {
                                debug!("Speech complete, transcribing...");
                                if let Ok(text) = whisper_engine.transcribe(&speech) {
                                    info!("Transcribed text: '{}'", text);
                                } else {
                                    error!("Transcription failed: {:?}", text);
                                }
                            }
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    }
                }
            }

            tracing::info!("VAD and Whisper processing loop ended");
        });

        *self.vad_task_handle.lock().await = Some(handle);

        drop(audio_rx);
        drop(speech_detector);
        drop(whisper_engine);

        Ok(())
    }

    pub async fn stop_vad_processing(&self) {
        let mut handle = self.vad_task_handle.lock().await.take();
        if let Some(handle) = handle {
            handle.abort();
            tracing::info!("VAD and Whisper processing loop stopped");
        }
    }
}
