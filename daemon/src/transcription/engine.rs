use anyhow::Result;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub struct WhisperEngine {
    context: Option<WhisperContext>,
    state: Option<WhisperState>,
    model_loaded: bool,
    model_path: PathBuf,
    model_url: String,
    backend: String,
}

impl WhisperEngine {
    pub fn new(model_url: String, backend: String) -> Result<Self> {
        let model_path = Self::find_model_path(&model_url)?;

        Ok(Self {
            context: None,
            state: None,
            model_loaded: false,
            model_path,
            model_url,
            backend,
        })
    }

    pub async fn load_model(&mut self) -> Result<()> {
        info!("Loading Whisper model from: {:?}", self.model_path);

        if !self.model_path.exists() {
            warn!(
                "Model file not found at {:?}. Attempting to download...",
                self.model_path
            );
            self.download_model().await?;
        }

        let use_gpu = match self.backend.to_lowercase().as_str() {
            "gpu" => true,
            "cuda" => true,
            "cpu" => false,
            _ => {
                warn!(
                    "Invalid backend value '{}', defaulting to CPU. Valid options: cpu, gpu, cuda",
                    self.backend
                );
                false
            }
        };

        let mut params = WhisperContextParameters::default();
        if use_gpu {
            info!("Attempting to use GPU backend for Whisper");
            params.use_gpu(true);
        } else {
            info!("Using CPU backend for Whisper");
            params.use_gpu(false);
        }

        let ctx = if use_gpu {
            match WhisperContext::new_with_params(self.model_path.to_str().unwrap(), params) {
                Ok(ctx) => ctx,
                Err(e) => {
                    warn!(
                        "GPU initialization failed: {}. Falling back to CPU backend. \
                        Note: whisper-rs GPU support on ROCm/AMD may not be fully stable. \
                        See: https://github.com/tazz4843/whisper-rs/issues/135",
                        e
                    );
                    let mut cpu_params = WhisperContextParameters::default();
                    cpu_params.use_gpu(false);
                    WhisperContext::new_with_params(self.model_path.to_str().unwrap(), cpu_params)
                        .map_err(|e| {
                        anyhow::anyhow!("Failed to load Whisper model (CPU fallback): {}", e)
                    })?
                }
            }
        } else {
            WhisperContext::new_with_params(self.model_path.to_str().unwrap(), params)?
        };

        let state = ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {}", e))?;

        self.context = Some(ctx);
        self.state = Some(state);
        self.model_loaded = true;

        let backend_name = if use_gpu { "GPU" } else { "CPU" };
        if use_gpu {
            info!(
                "Whisper model and state loaded successfully ({} backend attempted, CPU fallback used)",
                backend_name
            );
        } else {
            info!(
                "Whisper model and state loaded successfully ({} backend, stable memory usage)",
                backend_name
            );
        }
        Ok(())
    }

    pub async fn transcribe(&mut self, audio: &[f32]) -> Result<String> {
        if !self.model_loaded {
            return Err(anyhow::anyhow!("Model not loaded"));
        }

        debug!("Transcribing {} audio samples", audio.len());

        let audio = self.pad_audio(audio, 18000);

        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("WhisperState not initialized"))?;

        debug!("Setting transcription parameters...");
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some("en"));

        debug!("Running Whisper transcription...");
        state
            .full(params, &audio)
            .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

        debug!("Whisper transcription complete, getting segments...");
        let num_segments = state.full_n_segments();

        debug!("Extracting {} text segments...", num_segments);
        let mut transcription = String::new();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(text) = segment.to_str() {
                    transcription.push_str(text);
                    transcription.push(' ');
                }
            }
        }

        let cleaned = transcription.trim().to_string();
        let duration_ms = (audio.len() * 1000) / 16000;

        debug!("Transcription: '{}' ({} ms)", cleaned, duration_ms);

        Ok(cleaned)
    }

    fn pad_audio(&self, audio: &[f32], sample_rate: u32) -> Vec<f32> {
        let min_samples = sample_rate as usize;
        if audio.len() >= min_samples {
            return audio.to_vec();
        }

        let padding_len = min_samples - audio.len();
        debug!(
            "Padding audio: {} samples + {} samples of silence = {} samples ({} ms)",
            audio.len(),
            padding_len,
            min_samples,
            (min_samples * 1000) / sample_rate as usize
        );

        let mut padded = audio.to_vec();
        padded.extend(std::iter::repeat(0.0).take(padding_len));
        padded
    }

    pub fn find_model_path(model_url: &str) -> Result<PathBuf> {
        let model_filename = model_url
            .rsplit('/')
            .next()
            .ok_or_else(|| anyhow::anyhow!("Invalid model URL: cannot extract filename"))?;

        info!("Extracted model filename from URL: {}", model_filename);

        let possible_paths: Vec<Option<PathBuf>> = vec![
            dirs::home_dir().map(|p| p.join(".local/share/ndict/").join(model_filename)),
            Some(PathBuf::from("/usr/share/whisper/").join(model_filename)),
            Some(PathBuf::from("./models/").join(model_filename)),
            Some(PathBuf::from(model_filename)),
        ];

        for path in possible_paths {
            if let Some(p) = path {
                let path: PathBuf = p;
                if path.exists() {
                    info!("Found model at: {:?}", path);
                    return Ok(path);
                }
            }
        }

        let default_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".local/share/ndict/")
            .join(model_filename);

        warn!("Model not found, will use default path: {:?}", default_path);
        Ok(default_path)
    }

    async fn download_model(&mut self) -> Result<()> {
        let model_url = &self.model_url;
        let model_dir = self
            .model_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid model path"))?;

        info!("Creating model directory: {:?}", model_dir);
        tokio::fs::create_dir_all(model_dir).await?;

        info!("Downloading model from: {}", model_url);

        let response = reqwest::get(model_url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to download model: {}", e))?;

        let total_bytes = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        let mut file = tokio::fs::File::create(&self.model_path).await?;

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow::anyhow!("Download error: {}", e))?;
            downloaded += chunk.len() as u64;

            if total_bytes > 0 {
                let progress = (downloaded * 100) / total_bytes;
                info!("Download progress: {}%", progress);
            }

            file.write_all(&chunk).await?;
        }

        info!("Model downloaded successfully to: {:?}", self.model_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_model_path_existing() {
        let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
        let path = WhisperEngine::find_model_path(url).unwrap();

        assert!(path.to_str().unwrap().contains("ggml-base.bin"));
        assert!(path.extension().unwrap() == "bin");
    }

    #[test]
    fn test_find_model_path_fallback() {
        let url = "https://example.com/models/ggml-nonexistent.bin";
        let path = WhisperEngine::find_model_path(url).unwrap();

        assert!(path.to_str().unwrap().contains("ggml-nonexistent.bin"));
        assert!(path.to_str().unwrap().contains(".local/share/ndict"));
    }

    #[test]
    fn test_find_model_path_from_different_url() {
        let url = "https://custom-host.com/path/to/ggml-tiny.en.bin";
        let path = WhisperEngine::find_model_path(url).unwrap();

        assert!(path.to_str().unwrap().contains("ggml-tiny.en.bin"));
        assert!(path.extension().unwrap() == "bin");
    }

    #[test]
    fn test_pad_audio_no_padding_needed() {
        let engine = WhisperEngine::new(
            "https://example.com/model.bin".to_string(),
            "cpu".to_string(),
        )
        .unwrap();

        let audio = vec![0.0f32; 20000];
        let padded = engine.pad_audio(&audio, 16000);

        assert_eq!(padded.len(), 20000);
    }

    #[test]
    fn test_pad_audio_with_padding() {
        let engine = WhisperEngine::new(
            "https://example.com/model.bin".to_string(),
            "cpu".to_string(),
        )
        .unwrap();

        let audio = vec![0.0f32; 10000];
        let padded = engine.pad_audio(&audio, 16000);

        assert_eq!(padded.len(), 16000);
        assert_eq!(padded[..10000], audio);
        assert_eq!(padded[10000..], vec![0.0f32; 6000]);
    }

    #[test]
    fn test_pad_audio_exact_length() {
        let engine = WhisperEngine::new(
            "https://example.com/model.bin".to_string(),
            "cpu".to_string(),
        )
        .unwrap();

        let audio = vec![0.0f32; 16000];
        let padded = engine.pad_audio(&audio, 16000);

        assert_eq!(padded.len(), 16000);
        assert_eq!(padded, audio);
    }

    #[test]
    fn test_pad_audio_empty() {
        let engine = WhisperEngine::new(
            "https://example.com/model.bin".to_string(),
            "cpu".to_string(),
        )
        .unwrap();

        let audio = vec![];
        let padded = engine.pad_audio(&audio, 16000);

        assert_eq!(padded.len(), 16000);
        assert!(padded.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_new_whisper_engine() {
        let engine = WhisperEngine::new(
            "https://huggingface.co/model.bin".to_string(),
            "cpu".to_string(),
        )
        .unwrap();

        assert!(engine.model_url.contains("huggingface.co"));
        assert_eq!(engine.backend, "cpu");
        assert_eq!(engine.model_loaded, false);
        assert!(engine.context.is_none());
        assert!(engine.state.is_none());
    }

    #[test]
    fn test_new_whisper_engine_custom_url() {
        let custom_url = "http://custom.com/model.bin".to_string();
        let engine = WhisperEngine::new(custom_url.clone(), "gpu".to_string()).unwrap();

        assert_eq!(engine.model_url, custom_url);
        assert_eq!(engine.backend, "gpu");
    }
}
