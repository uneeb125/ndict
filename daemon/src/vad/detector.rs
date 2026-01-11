use anyhow::Result;
use tracing::info;

pub struct VoiceActivityDetector {
    threshold_start: f32,
    threshold_stop: f32,
}

impl VoiceActivityDetector {
    pub fn new(threshold_start: f32, threshold_stop: f32) -> Result<Self> {
        info!(
            "VAD initialized with threshold_start: {}, threshold_stop: {}",
            threshold_start, threshold_stop
        );

        Ok(Self {
            threshold_start,
            threshold_stop,
        })
    }

    pub fn detect(&self, audio_level: f32, is_speaking: bool) -> VADResult {
        let is_speech = if is_speaking {
            audio_level > self.threshold_stop
        } else {
            audio_level > self.threshold_start
        };

        tracing::debug!(
            "Audio level: {:.4}, threshold_start: {:.4}, threshold_stop: {:.4}, is_speaking: {}, is_speech: {}",
            audio_level,
            self.threshold_start,
            self.threshold_stop,
            is_speaking,
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

        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_squares / samples.len() as f32).sqrt();

        rms
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VADResult {
    pub is_speech: bool,
    pub probability: f32,
}
