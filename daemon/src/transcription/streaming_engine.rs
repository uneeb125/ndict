use anyhow::Result;
use tracing::{debug, info};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub struct StreamingEngine {
    context: Option<WhisperContext>,
    state: Option<WhisperState>,
    buffer: Vec<f32>,
    model_loaded: bool,
    length_samples: usize,
    keep_samples: usize,
    last_text: String,
    is_running: bool,
    language: String,
}

impl StreamingEngine {
    pub fn new(
        _model_path: String,
        language: String,
        _step_ms: u32,
        length_ms: u32,
        keep_ms: u32,
        sample_rate: u32,
    ) -> Self {
        let length_samples = (length_ms as usize * sample_rate as usize) / 1000;
        let keep_samples = (keep_ms as usize * sample_rate as usize) / 1000;

        Self {
            context: None,
            state: None,
            buffer: Vec::with_capacity(length_samples),
            model_loaded: false,
            length_samples,
            keep_samples,
            last_text: String::new(),
            is_running: false,
            language,
        }
    }

    pub async fn load_model(&mut self, model_path: &str) -> Result<()> {
        info!("Loading Whisper model from: {}", model_path);

        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {}", e))?;

        let state = ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {}", e))?;

        self.context = Some(ctx);
        self.state = Some(state);
        self.model_loaded = true;

        info!("Whisper model loaded successfully for streaming");
        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        if !self.model_loaded {
            return Err(anyhow::anyhow!("Model not loaded"));
        }

        self.buffer.clear();
        self.last_text.clear();
        self.is_running = true;

        info!("Streaming engine started");
        Ok(())
    }

    pub fn send_audio(&mut self, audio_chunk: &[f32]) -> Result<Option<String>> {
        if !self.is_running || self.state.is_none() {
            return Ok(None);
        }

        self.buffer.extend(audio_chunk);

        if self.buffer.len() < self.length_samples {
            debug!(
                "Buffer not yet full: {}/{} samples",
                self.buffer.len(),
                self.length_samples
            );
            return Ok(None);
        }

        let transcription = self.process_window()?;

        self.buffer = self
            .buffer
            .iter()
            .skip(self.length_samples - self.keep_samples)
            .copied()
            .collect();

        Ok(transcription)
    }

    pub async fn stop(&mut self) {
        info!("Stopping streaming engine");
        self.is_running = false;
        self.buffer.clear();
        self.last_text.clear();
    }

    pub fn set_language(&mut self, language: String) {
        self.language = language;
        info!("Streaming engine language updated to: {}", self.language);
    }

    fn process_window(&mut self) -> Result<Option<String>> {
        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("WhisperState not initialized"))?;

        debug!("Processing window with {} samples", self.buffer.len());

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some(&self.language));
        params.set_single_segment(true);

        state
            .full(params, &self.buffer)
            .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

        let num_segments = state.full_n_segments();
        let mut transcription = String::new();

        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(text) = segment.to_str() {
                    transcription.push_str(text);
                }
            }
        }

        let trimmed = transcription.trim().to_string();

        if !trimmed.is_empty() && trimmed != self.last_text {
            self.last_text = trimmed.clone();
            debug!("New transcription: '{}'", trimmed);
            return Ok(Some(trimmed));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_engine_new() {
        let engine = StreamingEngine::new(
            "test_model.bin".to_string(),
            "en".to_string(),
            3000,
            10000,
            500,
            16000,
        );

        assert_eq!(engine.length_samples, 160000);
        assert_eq!(engine.keep_samples, 8000);
        assert!(!engine.is_running);
    }

    #[test]
    fn test_streaming_engine_custom_params() {
        let engine =
            StreamingEngine::new("custom.bin".to_string(), "es".to_string(), 1500, 5000, 500, 16000);

        assert_eq!(engine.length_samples, 80000);
        assert_eq!(engine.keep_samples, 8000);
    }

    #[test]
    fn test_streaming_engine_not_running() {
        let mut engine =
            StreamingEngine::new("test.bin".to_string(), "en".to_string(), 3000, 10000, 200, 16000);

        let result = engine.send_audio(&[0.0f32; 512]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_streaming_engine_set_language() {
        let mut engine =
            StreamingEngine::new("test.bin".to_string(), "en".to_string(), 3000, 10000, 200, 16000);

        assert_eq!(engine.language, "en");

        engine.set_language("es".to_string());
        assert_eq!(engine.language, "es");

        engine.set_language("fr".to_string());
        assert_eq!(engine.language, "fr");
    }
}
