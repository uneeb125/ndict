use anyhow::Result;
use tracing::{debug, info};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub struct StreamingWrapper {
    context: Option<WhisperContext>,
    state: Option<WhisperState>,
    buffer: Vec<f32>,
    window_samples: usize,
    overlap_samples: usize,
    accumulated_text: String,
    is_active: bool,
}

impl StreamingWrapper {
    pub fn new(sample_rate: u32) -> Self {
        let window_ms = 3000;
        let overlap_ms = 500;

        let window_samples = (window_ms as usize * sample_rate as usize) / 1000;
        let overlap_samples = (overlap_ms as usize * sample_rate as usize) / 1000;

        Self {
            context: None,
            state: None,
            buffer: Vec::with_capacity(window_samples),
            window_samples,
            overlap_samples,
            accumulated_text: String::new(),
            is_active: false,
        }
    }

    pub async fn load_model(&mut self, model_path: &str) -> Result<()> {
        info!("Loading Whisper model for streaming: {}", model_path);

        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {}", e))?;

        let state = ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {}", e))?;

        self.context = Some(ctx);
        self.state = Some(state);

        info!("Whisper model loaded for streaming wrapper");
        Ok(())
    }

    pub fn activate(&mut self) {
        self.buffer.clear();
        self.accumulated_text.clear();
        self.is_active = true;
        debug!("Streaming wrapper activated");
    }

    pub fn process_chunk(&mut self, chunk: &[f32]) -> Result<Option<String>> {
        if !self.is_active || self.state.is_none() {
            return Ok(None);
        }

        self.buffer.extend(chunk);

        if self.buffer.len() < self.window_samples {
            debug!(
                "Buffer not full: {}/{} samples",
                self.buffer.len(),
                self.window_samples
            );
            return Ok(None);
        }

        let new_text = self.transcribe_window()?;

        self.buffer = self
            .buffer
            .iter()
            .skip(self.window_samples - self.overlap_samples)
            .copied()
            .collect();

        if !new_text.is_empty() && new_text != self.accumulated_text {
            self.accumulated_text = new_text.clone();
            debug!("Streaming transcription: '{}'", new_text);
            return Ok(Some(new_text));
        }

        Ok(None)
    }

    pub fn finalize(&mut self) -> Result<Option<String>> {
        if !self.is_active {
            return Ok(None);
        }

        debug!("Finalizing streaming transcription");

        let final_text = if !self.buffer.is_empty() {
            Some(self.transcribe_window()?)
        } else {
            None
        };

        let result = final_text.filter(|t| !t.is_empty() && t != &self.accumulated_text);

        self.deactivate();

        if let Some(ref text) = result {
            info!("Final streaming transcription: '{}'", text);
        }

        Ok(result)
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.buffer.clear();
        debug!("Streaming wrapper deactivated");
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    fn transcribe_window(&mut self) -> Result<String> {
        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("WhisperState not initialized"))?;

        debug!("Transcribing window with {} samples", self.buffer.len());

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some("en"));
        params.set_single_segment(true);

        state
            .full(params, &self.buffer)
            .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

        let num_segments = state
            .full_n_segments()
            .map_err(|e| anyhow::anyhow!("Failed to get segment count: {}", e))?;
        let mut transcription = String::new();

        for i in 0..num_segments {
            if let Ok(text) = state.full_get_segment_text(i) {
                transcription.push_str(&text);
            }
        }

        let trimmed = transcription.trim().to_string();
        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_wrapper_new() {
        let wrapper = StreamingWrapper::new(16000);

        assert_eq!(wrapper.window_samples, 48000);
        assert_eq!(wrapper.overlap_samples, 8000);
        assert!(!wrapper.is_active);
        assert!(wrapper.buffer.is_empty());
        assert!(wrapper.accumulated_text.is_empty());
    }

    #[test]
    fn test_streaming_wrapper_activate() {
        let mut wrapper = StreamingWrapper::new(16000);

        wrapper.activate();

        assert!(wrapper.is_active);
        assert!(wrapper.buffer.is_empty());
        assert!(wrapper.accumulated_text.is_empty());
    }

    #[test]
    fn test_streaming_wrapper_deactivate() {
        let mut wrapper = StreamingWrapper::new(16000);

        wrapper.activate();
        wrapper.deactivate();

        assert!(!wrapper.is_active);
        assert!(wrapper.buffer.is_empty());
    }

    #[test]
    fn test_streaming_wrapper_process_not_active() {
        let mut wrapper = StreamingWrapper::new(16000);

        let result = wrapper.process_chunk(&[0.0f32; 512]);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_streaming_wrapper_not_enough_audio() {
        let mut wrapper = StreamingWrapper::new(16000);
        wrapper.activate();

        let chunk = vec![0.0f32; 100];
        let result = wrapper.process_chunk(&chunk);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        assert_eq!(wrapper.buffer.len(), 100);
    }
}
