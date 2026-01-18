use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::detector::VoiceActivityDetector;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeechState {
    Idle,
    Speaking,
    SilenceDetected,
}

pub struct SpeechDetector {
    state: SpeechState,
    vad: VoiceActivityDetector,
    speech_start_time: Option<Instant>,
    silence_start_time: Option<Instant>,
    speech_buffer: Vec<f32>,
    silence_duration_ms: u32,
    gain: f32,
}

impl SpeechDetector {
    pub fn new(
        threshold_start: f32,
        threshold_stop: f32,
        silence_duration_ms: u32,
        gain: f32,
    ) -> anyhow::Result<Self> {
        let vad = VoiceActivityDetector::new(threshold_start, threshold_stop)?;
        tracing::info!(
            "SpeechDetector initialized: threshold_start={:.4}, threshold_stop={:.4}, silence_duration_ms={}, gain={:.2}",
            threshold_start,
            threshold_stop,
            silence_duration_ms,
            gain
        );

        Ok(Self {
            state: SpeechState::Idle,
            vad,
            speech_start_time: None,
            silence_start_time: None,
            speech_buffer: Vec::new(),
            silence_duration_ms,
            gain,
        })
    }

    pub fn process_audio(&mut self, samples: &[f32]) -> Option<Vec<f32>> {
        let audio_level = self.vad.calculate_audio_level(samples);
        let is_speaking = self.state == SpeechState::Speaking;
        let vad_result = self.vad.detect(audio_level, is_speaking);

        match self.state {
            SpeechState::Idle => {
                if vad_result.is_speech {
                    self.transition_to_speaking();
                    self.speech_buffer.extend_from_slice(samples);
                    info!("State transition: Idle → Speaking");
                    debug!("Speech detected, buffer size: {}", self.speech_buffer.len());
                }
            }
            SpeechState::Speaking => {
                self.speech_buffer.extend_from_slice(samples);

                if !vad_result.is_speech {
                    self.transition_to_silence_detected();
                    warn!("State transition: Speaking → SilenceDetected");
                    debug!(
                        "Silence detected, waiting {}ms confirmation",
                        self.silence_duration_ms
                    );
                }
            }
            SpeechState::SilenceDetected => {
                self.speech_buffer.extend_from_slice(samples);

                if vad_result.is_speech {
                    self.transition_to_speaking();
                    info!("State transition: SilenceDetected → Speaking (false alarm)");
                    debug!(
                        "False alarm, still speaking. Buffer size: {}",
                        self.speech_buffer.len()
                    );
                } else if self.silence_duration_exceeded() {
                    let speech = std::mem::take(&mut self.speech_buffer);
                    self.reset();
                    let duration_ms = self.calculate_duration_ms(&speech);
                    info!("State transition: SilenceDetected → Idle");
                    info!(
                        "Speech complete: {} ms, {} samples",
                        duration_ms,
                        speech.len()
                    );
                    // Apply gain before sending to Whisper
                    let amplified_speech: Vec<f32> =
                        speech.iter().map(|&s| s * self.gain).collect();
                    return Some(amplified_speech);
                }
            }
        }

        None
    }

    fn transition_to_speaking(&mut self) {
        self.state = SpeechState::Speaking;
        self.speech_start_time = Some(Instant::now());
        self.silence_start_time = None;
    }

    fn transition_to_silence_detected(&mut self) {
        self.state = SpeechState::SilenceDetected;
        self.silence_start_time = Some(Instant::now());
    }

    fn silence_duration_exceeded(&self) -> bool {
        self.silence_start_time
            .map(|t| t.elapsed() >= Duration::from_millis(self.silence_duration_ms as u64))
            .unwrap_or(false)
    }

    fn calculate_duration_ms(&self, samples: &[f32]) -> u32 {
        let sample_count = samples.len();
        let sample_rate = 16000u32;
        let duration_ms = (sample_count as u32 * 1000) / sample_rate;
        duration_ms
    }

    fn reset(&mut self) {
        self.state = SpeechState::Idle;
        self.speech_start_time = None;
        self.silence_start_time = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speech_detector_new() {
        let detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();
        assert_eq!(detector.state, SpeechState::Idle);
        assert!(detector.speech_start_time.is_none());
        assert!(detector.silence_start_time.is_none());
        assert!(detector.speech_buffer.is_empty());
    }

    #[test]
    fn test_idle_state_no_speech_below_threshold() {
        let mut detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();

        let samples = vec![0.01, 0.01, 0.01];
        let result = detector.process_audio(&samples);

        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::Idle);
        assert!(detector.speech_buffer.is_empty());
    }

    #[test]
    fn test_idle_state_transition_to_speaking() {
        let mut detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();

        let samples = vec![0.03, 0.03, 0.03];
        let result = detector.process_audio(&samples);

        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::Speaking);
        assert!(!detector.speech_buffer.is_empty());
    }

    #[test]
    fn test_speaking_state_accumulates_buffer() {
        let mut detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();

        let samples1 = vec![0.03, 0.03];
        detector.process_audio(&samples1);

        let samples2 = vec![0.04, 0.04];
        detector.process_audio(&samples2);

        assert_eq!(detector.state, SpeechState::Speaking);
        assert_eq!(detector.speech_buffer.len(), 4);
    }

    #[test]
    fn test_speaking_to_silence_detected_transition() {
        let mut detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();

        let samples_speech = vec![0.03, 0.03];
        detector.process_audio(&samples_speech);

        let samples_silence = vec![0.005, 0.005];
        let result = detector.process_audio(&samples_silence);

        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::SilenceDetected);
        assert!(detector.silence_start_time.is_some());
    }

    #[test]
    fn test_silence_detected_to_speaking_false_alarm() {
        let mut detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();

        detector.process_audio(&vec![0.03, 0.03]);
        detector.process_audio(&vec![0.005, 0.005]);
        let result = detector.process_audio(&vec![0.03, 0.03]);

        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::Speaking);
        assert!(detector.silence_start_time.is_none());
    }

    #[test]
    fn test_hysteresis_prevents_oscillation() {
        let mut detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();

        detector.process_audio(&vec![0.03, 0.03]);
        assert_eq!(detector.state, SpeechState::Speaking);

        detector.process_audio(&vec![0.015, 0.015]);
        assert_eq!(detector.state, SpeechState::Speaking);

        detector.process_audio(&vec![0.005, 0.005]);
        assert_eq!(detector.state, SpeechState::SilenceDetected);
    }

    #[test]
    fn test_empty_samples_does_not_crash() {
        let mut detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();

        let result = detector.process_audio(&[]);
        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::Idle);
    }

    #[test]
    fn test_duration_calculation() {
        let detector = SpeechDetector::new(0.02, 0.01, 100, 1.0).unwrap();

        let samples = vec![0.0f32; 1600];
        let calculated = detector.calculate_duration_ms(&samples);

        assert_eq!(calculated, 100);
    }
}
