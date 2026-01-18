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
            audio_level >= self.threshold_stop
        } else {
            audio_level >= self.threshold_start
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_new() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        assert_eq!(vad.threshold_start, 0.02);
        assert_eq!(vad.threshold_stop, 0.01);
    }

    #[test]
    fn test_vad_new_with_equal_thresholds() {
        let vad = VoiceActivityDetector::new(0.02, 0.02).unwrap();
        assert_eq!(vad.threshold_start, 0.02);
        assert_eq!(vad.threshold_stop, 0.02);
    }

    #[test]
    fn test_calculate_audio_level_empty() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let level = vad.calculate_audio_level(&[]);
        assert_eq!(level, 0.0);
    }

    #[test]
    fn test_calculate_audio_level_silence() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![0.0, 0.0, 0.0, 0.0];
        let level = vad.calculate_audio_level(&samples);
        assert_eq!(level, 0.0);
    }

    #[test]
    fn test_calculate_audio_level_full_scale() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![1.0, 1.0, 1.0, 1.0];
        let level = vad.calculate_audio_level(&samples);
        assert_eq!(level, 1.0);
    }

    #[test]
    fn test_calculate_audio_level_mixed() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![0.0, 0.5, 1.0, 0.5];
        let level = vad.calculate_audio_level(&samples);
        let expected = 0.612;
        assert!((level - expected).abs() < 0.001);
    }

    #[test]
    fn test_calculate_audio_level_negative_values() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![-0.5, -0.5, -0.5, -0.5];
        let level = vad.calculate_audio_level(&samples);
        assert_eq!(level, 0.5);
    }

    #[test]
    fn test_calculate_audio_level_mixed_sign() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![-1.0, 0.0, 1.0, 0.0];
        let level = vad.calculate_audio_level(&samples);
        let expected = 0.707;
        assert!((level - expected).abs() < 0.001);
    }

    #[test]
    fn test_detect_speech_idle_above_threshold_start() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = false;
        let result = vad.detect(0.03, is_speaking);
        assert!(result.is_speech);
        assert_eq!(result.probability, 0.03);
    }

    #[test]
    fn test_detect_speech_idle_below_threshold_start() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = false;
        let result = vad.detect(0.015, is_speaking);
        assert!(!result.is_speech);
        assert_eq!(result.probability, 0.015);
    }

    #[test]
    fn test_detect_speech_idle_exactly_threshold_start() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = false;
        let result = vad.detect(0.02, is_speaking);
        assert!(result.is_speech);
    }

    #[test]
    fn test_detect_speech_speaking_above_threshold_stop() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = true;
        let result = vad.detect(0.015, is_speaking);
        assert!(result.is_speech);
    }

    #[test]
    fn test_detect_speech_speaking_below_threshold_stop() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = true;
        let result = vad.detect(0.005, is_speaking);
        assert!(!result.is_speech);
    }

    #[test]
    fn test_detect_speech_speaking_exactly_threshold_stop() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = true;
        let result = vad.detect(0.01, is_speaking);
        assert!(result.is_speech);
    }

    #[test]
    fn test_detect_hysteresis() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let audio_level = 0.015;

        let result_idle = vad.detect(audio_level, false);
        assert!(!result_idle.is_speech);

        let result_speaking = vad.detect(audio_level, true);
        assert!(result_speaking.is_speech);
    }

    #[test]
    fn test_vad_result_contains_probability() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let result = vad.detect(0.05, false);
        assert_eq!(result.probability, 0.05);
    }

    #[test]
    fn test_detect_with_zero_audio_level() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let result = vad.detect(0.0, false);
        assert!(!result.is_speech);
        assert_eq!(result.probability, 0.0);
    }
}
