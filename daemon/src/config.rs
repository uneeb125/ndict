use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub audio: AudioConfig,
    pub vad: VadConfig,
    pub whisper: WhisperConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    pub device: String,
    pub sample_rate: u32,
    pub chunk_size: u32,
    pub gain: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VadConfig {
    pub threshold: f32,
    pub min_speech_duration_ms: u32,
    pub min_silence_duration_ms: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WhisperConfig {
    pub model: String,
    pub model_path: Option<String>,
    pub language: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    pub typing_mode: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig {
                device: "default".to_string(),
                sample_rate: 16000,
                chunk_size: 512,
                gain: 1.0,
            },
            vad: VadConfig {
                threshold: 0.01,
                min_speech_duration_ms: 250,
                min_silence_duration_ms: 1000,
            },
            whisper: WhisperConfig {
                model: "base".to_string(),
                model_path: None,
                language: "auto".to_string(),
            },
            output: OutputConfig {
                typing_mode: "instant".to_string(),
            },
        }
    }
}

pub fn load_config() -> Result<Config> {
    let config_path = get_config_path();

    if !config_path.exists() {
        tracing::info!("Config file not found at {:?}, using defaults", config_path);
        return Ok(Config::default());
    }

    tracing::info!("Loading config from {:?}", config_path);
    let config_str = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file: {}", e))?;

    let config: Config = toml::from_str(&config_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse config file: {}", e))?;

    tracing::info!("Config loaded successfully");
    Ok(config)
}

fn get_config_path() -> PathBuf {
    dirs::config_dir()
        .expect("Failed to get config directory")
        .join("ndict")
        .join("config.toml")
}
