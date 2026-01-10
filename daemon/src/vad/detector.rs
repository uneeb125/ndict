use anyhow::Result;
use tracing::info;

pub struct VoiceActivityDetector {
    threshold: f32,
}

impl VoiceActivityDetector {
    pub fn new(threshold: f32) -> Result<Self> {
        info!("VAD initialized with threshold: {}", threshold);

        Ok(Self { threshold })
    }

    pub fn detect(&self, audio_level: f32) -> VADResult {
        let is_speech = audio_level > self.threshold;

        tracing::debug!(
            "Audio level: {:.4}, threshold: {:.4}, is_speech: {}",
            audio_level,
            self.threshold,
            is_speech
        );

        VADResult {
            is_speech,
            probability: audio_level,
        }
    }

    pub fn calculate_audio_level(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        let sum: f32 = samples.iter().map(|s| s.abs()).sum();
        let rms = (sum / samples.len() as f32).sqrt();

        rms
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VADResult {
    pub is_speech: bool,
    pub probability: f32,
}
