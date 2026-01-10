use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::detector::{VADResult, VoiceActivityDetector};

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
}

impl SpeechDetector {
    pub fn new(threshold: f32, silence_duration_ms: u32) -> anyhow::Result<Self> {
        let vad = VoiceActivityDetector::new(threshold)?;
        tracing::info!(
            "SpeechDetector initialized: threshold={:.4}, silence_duration_ms={}",
            threshold,
            silence_duration_ms
        );

        Ok(Self {
            state: SpeechState::Idle,
            vad,
            speech_start_time: None,
            silence_start_time: None,
            speech_buffer: Vec::new(),
            silence_duration_ms,
        })
    }

    pub fn process_audio(&mut self, samples: &[f32]) -> Option<Vec<f32>> {
        let vad_result = self.vad.detect(self.vad.calculate_audio_level(samples));

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
                    return Some(speech);
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

    pub fn is_speaking(&self) -> bool {
        matches!(
            self.state,
            SpeechState::Speaking | SpeechState::SilenceDetected
        )
    }
}
