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
    model_name: String,
}

impl WhisperEngine {
    pub fn new(model_url: String, model_name: String) -> Result<Self> {
        let model_path = Self::find_model_path(&model_name)?;

        Ok(Self {
            context: None,
            state: None,
            model_loaded: false,
            model_path,
            model_url,
            model_name,
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

        // --- CHANGE START: Configure Parameters for GPU ---
        let mut params = WhisperContextParameters::default();
        // This tells whisper.cpp to attempt to use the GPU offload
        params.use_gpu(true);
        // Optional: specific GPU device index (defaults to 0)
        // params.gpu_device(0);
        // --- CHANGE END ---

        let ctx = WhisperContext::new_with_params(
            self.model_path.to_str().unwrap(),
            params, // Pass the modified params here
        )
        .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {}", e))?;

        // Create WhisperState once to maintain stable GPU memory usage
        // This keeps compute/scratch buffers (KV cache, activation buffers) allocated
        let state = ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {}", e))?;

        self.context = Some(ctx);
        self.state = Some(state);
        self.model_loaded = true;

        info!("Whisper model and state loaded successfully (GPU requested, stable memory usage)");
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

    fn find_model_path(model_name: &str) -> Result<PathBuf> {
        let model_filename = format!("ggml-{}.bin", model_name);
        let model_filename_en = format!("ggml-{}.en.bin", model_name);

        let possible_paths: Vec<Option<PathBuf>> = vec![
            dirs::home_dir().map(|p| p.join(".local/share/ndict/").join(&model_filename)),
            dirs::home_dir().map(|p| p.join(".local/share/ndict/").join(&model_filename_en)),
            Some(PathBuf::from("/usr/share/whisper/").join(&model_filename)),
            Some(PathBuf::from("/usr/share/whisper/").join(&model_filename_en)),
            Some(PathBuf::from("./models/").join(&model_filename)),
            Some(PathBuf::from("./models/").join(&model_filename_en)),
            Some(PathBuf::from(&model_filename)),
            Some(PathBuf::from(&model_filename_en)),
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
            .join(&model_filename);

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
