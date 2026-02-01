use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub struct WhisperEngine {
    context: Option<WhisperContext>,
    state: Option<WhisperState>,
    model_loaded: bool,
    model_path: PathBuf,
    model_url: String,
    model_checksum: Option<String>,
    backend: String,
    min_audio_samples: usize,
    sampling_strategy: String,
}

impl WhisperEngine {
    pub fn new(model_url: String, backend: String) -> Result<Self> {
        Self::new_with_checksum_and_params(model_url, backend, None, 18000, "greedy".to_string())
    }

    pub fn new_with_checksum(
        model_url: String,
        backend: String,
        model_checksum: Option<String>,
    ) -> Result<Self> {
        Self::new_with_checksum_and_params(model_url, backend, model_checksum, 18000, "greedy".to_string())
    }

    pub fn new_with_checksum_and_params(
        model_url: String,
        backend: String,
        model_checksum: Option<String>,
        min_audio_samples: usize,
        sampling_strategy: String,
    ) -> Result<Self> {
        let model_path = Self::find_model_path(&model_url)?;

        Ok(Self {
            context: None,
            state: None,
            model_loaded: false,
            model_path,
            model_url,
            model_checksum,
            backend,
            min_audio_samples,
            sampling_strategy,
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
        } else {
            // Verify existing model if checksum is configured
            if let Some(ref expected_checksum) = self.model_checksum {
                info!("Model file exists, verifying checksum...");
                let actual_checksum = self.compute_file_checksum(&self.model_path)?;
                if &actual_checksum == expected_checksum {
                    info!("Model checksum verification passed: {}", actual_checksum);
                } else {
                    error!(
                        "Model checksum mismatch! Expected: {}, Got: {}",
                        expected_checksum, actual_checksum
                    );
                    warn!("Re-downloading model due to checksum mismatch...");
                    self.download_model().await?;
                }
            }
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

        let (ctx, actually_using_gpu) = if use_gpu {
            match WhisperContext::new_with_params(self.model_path.to_str().unwrap(), params) {
                Ok(ctx) => (ctx, true),
                Err(e) => {
                    warn!(
                        "GPU initialization failed: {}. Falling back to CPU backend. \
                        Note: whisper-rs GPU support on ROCm/AMD may not be fully stable. \
                        See: https://github.com/tazz4843/whisper-rs/issues/135",
                        e
                    );
                    let mut cpu_params = WhisperContextParameters::default();
                    cpu_params.use_gpu(false);
                    let ctx = WhisperContext::new_with_params(self.model_path.to_str().unwrap(), cpu_params)
                        .map_err(|e| {
                        anyhow::anyhow!("Failed to load Whisper model (CPU fallback): {}", e)
                    })?;
                    (ctx, false)
                }
            }
        } else {
            (WhisperContext::new_with_params(self.model_path.to_str().unwrap(), params)?, false)
        };

        let state = ctx
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {}", e))?;

        self.context = Some(ctx);
        self.state = Some(state);
        self.model_loaded = true;

        let backend_name = if actually_using_gpu { "GPU" } else { "CPU" };
        if use_gpu && !actually_using_gpu {
            warn!(
                "Whisper model loaded successfully using CPU backend (GPU fallback activated)"
            );
        } else {
            info!(
                "Whisper model and state loaded successfully ({} backend)",
                backend_name
            );
        }
        Ok(())
    }

    pub async fn transcribe(&mut self, audio: &[f32], language: &str) -> Result<String> {
        if !self.model_loaded {
            return Err(anyhow::anyhow!("Model not loaded"));
        }

        debug!("Transcribing {} audio samples with language: {}", audio.len(), language);

        let audio = self.pad_audio(audio, self.min_audio_samples as u32);

        debug!("Setting transcription parameters...");
        let sampling_strategy = self.parse_sampling_strategy();

        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("WhisperState not initialized"))?;

        let mut params = FullParams::new(sampling_strategy);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some(language));

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

    fn parse_sampling_strategy(&self) -> SamplingStrategy {
        match self.sampling_strategy.to_lowercase().as_str() {
            "greedy" => SamplingStrategy::Greedy { best_of: 1 },
            "beam" => SamplingStrategy::BeamSearch {
                beam_size: 5,
                patience: 1.0,
            },
            _ => {
                tracing::warn!(
                    "Unknown sampling strategy '{}', defaulting to greedy",
                    self.sampling_strategy
                );
                SamplingStrategy::Greedy { best_of: 1 }
            }
        }
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

        // Create temporary file for atomic write
        let temp_path = format!("{}.tmp", self.model_path.display());
        let temp_path = PathBuf::from(&temp_path);

        // Clean up any existing temporary file
        if temp_path.exists() {
            warn!("Removing existing temporary file: {:?}", temp_path);
            tokio::fs::remove_file(&temp_path).await?;
        }

        // Retry logic with exponential backoff
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 1..=max_retries {
            debug!("Download attempt {}/{}", attempt, max_retries);

            match self
                .download_model_with_checksum(&temp_path, model_url, attempt, max_retries)
                .await
            {
                Ok(()) => {
                    // Verify checksum if provided
                    if let Some(ref expected_checksum) = self.model_checksum {
                        info!("Verifying model checksum...");
                        let actual_checksum = self.compute_file_checksum(&temp_path)?;
                        if &actual_checksum != expected_checksum {
                            error!(
                                "Checksum verification failed! Expected: {}, Got: {}",
                                expected_checksum, actual_checksum
                            );
                            // Clean up the failed download
                            tokio::fs::remove_file(&temp_path).await?;
                            last_error = Some(anyhow::anyhow!(
                                "Checksum mismatch: expected {}, got {}",
                                expected_checksum,
                                actual_checksum
                            ));
                            continue;
                        }
                        info!("Checksum verification passed: {}", actual_checksum);
                    }

                    // Atomic rename from temp to final path
                    info!("Atomic rename: {:?} -> {:?}", temp_path, self.model_path);
                    tokio::fs::rename(&temp_path, &self.model_path).await?;
                    info!("Model downloaded and verified successfully to: {:?}", self.model_path);
                    return Ok(());
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    error!("Download attempt {} failed: {}", attempt, error_msg);
                    last_error = Some(anyhow::anyhow!(error_msg));

                    // Clean up partial download
                    if temp_path.exists() {
                        warn!("Cleaning up partial download: {:?}", temp_path);
                        if let Err(cleanup_err) = tokio::fs::remove_file(&temp_path).await {
                            warn!("Failed to clean up temporary file: {}", cleanup_err);
                        }
                    }

                    // Exponential backoff before next retry
                    if attempt < max_retries {
                        let delay_ms = 1000 * 2_u64.pow(attempt as u32);
                        info!(
                            "Waiting {} ms before retry (attempt {}/{})...",
                            delay_ms, attempt + 1, max_retries
                        );
                        sleep(Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("Failed to download model after {} attempts", max_retries)
        }))
    }

    async fn download_model_with_checksum(
        &self,
        temp_path: &PathBuf,
        model_url: &str,
        attempt: usize,
        max_attempts: usize,
    ) -> Result<()> {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        info!(
            "Starting download (attempt {}/{}): {}",
            attempt, max_attempts, model_url
        );

        // Configure client with timeouts
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute total timeout for download
            .connect_timeout(Duration::from_secs(30)) // 30 second connect timeout
            .redirect(reqwest::redirect::Policy::limited(5)) // Max 5 redirects
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

        // Send HEAD request to get file size
        let head_response = client
            .head(model_url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("HEAD request failed: {}", e))?;

        let expected_size = head_response.content_length();

        // Check for ETag (optional, for HuggingFace)
        let etag = head_response.headers().get("etag").and_then(|v| v.to_str().ok());
        if let Some(etag) = etag {
            info!("Server ETag: {}", etag);
        }

        // Start streaming download
        let response = client
            .get(model_url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("GET request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error: {}",
                response.status()
            ));
        }

        let total_bytes = response.content_length();
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        // Create SHA256 hasher
        let mut hasher = Sha256::new();

        // Open temp file for writing
        let mut file = tokio::fs::File::create(temp_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create temp file: {}", e))?;

        let start_time = std::time::Instant::now();

        // Download chunks with streaming checksum calculation
        loop {
            // Add 30-second timeout to each chunk read
            let chunk_result = timeout(Duration::from_secs(30), stream.next()).await;

            let chunk = match chunk_result {
                Ok(Some(Ok(c))) => c,
                Ok(Some(Err(e))) => {
                    return Err(anyhow::anyhow!("Download error: {}", e));
                }
                Ok(None) => {
                    // End of stream
                    break;
                }
                Err(_) => {
                    // Timeout occurred
                    return Err(anyhow::anyhow!(
                        "Download chunk read timeout: server did not send data within 30 seconds"
                    ));
                }
            };
            let chunk_len = chunk.len();
            downloaded += chunk_len as u64;

            // Update SHA256 hash with this chunk
            hasher.update(&chunk);

            // Write to file
            file.write_all(&chunk).await.map_err(|e| {
                anyhow::anyhow!("Failed to write to temp file: {}", e)
            })?;

            // Log progress every 10% or every 10 seconds
            if total_bytes.is_some() {
                let total = total_bytes.unwrap();
                let progress = (downloaded * 100) / total;
                let elapsed = start_time.elapsed().as_secs();

                if progress % 10 == 0 || elapsed % 10 == 0 {
                    let speed = if elapsed > 0 {
                        downloaded / elapsed
                    } else {
                        0
                    };
                    info!(
                        "Download progress: {}% ({}/{} bytes, {} bytes/s)",
                        progress,
                        Self::pretty_bytes(downloaded),
                        Self::pretty_bytes(total),
                        Self::pretty_bytes(speed)
                    );
                }
            } else {
                let elapsed = start_time.elapsed().as_secs();
                if elapsed % 10 == 0 {
                    info!(
                        "Download progress: {} bytes downloaded...",
                        Self::pretty_bytes(downloaded)
                    );
                }
            }
        }

        // Flush and close the file
        file.flush().await.map_err(|e| {
            anyhow::anyhow!("Failed to flush temp file: {}", e)
        })?;
        drop(file);

        // Verify file size matches expected size from HEAD request
        if let Some(expected) = expected_size {
            let metadata = tokio::fs::metadata(temp_path).await?;
            let actual_size = metadata.len();

            if actual_size != expected {
                return Err(anyhow::anyhow!(
                    "File size mismatch: expected {} bytes, got {} bytes",
                    Self::pretty_bytes(expected),
                    Self::pretty_bytes(actual_size)
                ));
            }

            info!(
                "File size verification passed: {} bytes",
                Self::pretty_bytes(actual_size)
            );
        }

        // Note: Checksum verification is handled by the caller
        debug!("Download streaming complete, file written to: {:?}", temp_path);

        Ok(())
    }

    fn compute_file_checksum(&self, file_path: &PathBuf) -> Result<String> {
        use std::fs::File;
        use std::io::Read;

        info!("Computing SHA256 checksum for: {:?}", file_path);

        let mut file = File::open(file_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        let result = hasher.finalize();
        let checksum = hex::encode(result);

        info!("Computed SHA256 checksum: {}", checksum);

        Ok(checksum)
    }

    /// Helper function to format bytes in human-readable format
    fn pretty_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_index])
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

    #[test]
    fn test_new_whisper_engine_with_checksum() {
        let custom_url = "http://custom.com/model.bin".to_string();
        let checksum = Some("abc123def456".to_string());
        let engine =
            WhisperEngine::new_with_checksum(custom_url.clone(), "gpu".to_string(), checksum.clone())
                .unwrap();

        assert_eq!(engine.model_url, custom_url);
        assert_eq!(engine.backend, "gpu");
        assert_eq!(engine.model_checksum, checksum);
    }
}
