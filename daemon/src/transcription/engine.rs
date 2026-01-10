use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
}

#[derive(Clone)]
pub struct WhisperEngine {
    model_loaded: bool,
}

impl WhisperEngine {
    pub fn new() -> Result<Self> {
        info!("WhisperEngine created (mock implementation)");
        Ok(Self {
            model_loaded: false,
        })
    }

    pub async fn load_model(&mut self) -> Result<()> {
        info!("Loading Whisper model (mock - using base model)");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        self.model_loaded = true;
        info!("Whisper model loaded successfully");
        Ok(())
    }

    pub async fn transcribe(&mut self, audio: &[f32]) -> Result<String> {
        if !self.model_loaded {
            return Err(anyhow::anyhow!("Model not loaded"));
        }

        debug!("Transcribing {} audio samples", audio.len());
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let sample_transcriptions = vec![
            "hello world",
            "this is a test",
            "the quick brown fox",
            "speech to text",
            "whisper transcription working",
        ];

        let transcription =
            sample_transcriptions[audio.len() % sample_transcriptions.len()].to_string();
        let duration_ms = (audio.len() * 1000) / 16000;

        debug!("Transcription: '{}' ({} ms)", transcription, duration_ms);

        Ok(transcription)
    }

    pub fn is_model_loaded(&self) -> bool {
        self.model_loaded
    }
}
