use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub vad: VadConfig,
    #[serde(default)]
    pub whisper: WhisperConfig,
    #[serde(default)]
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct AudioConfig {
    #[serde(default)]
    pub device: String,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
    #[serde(default = "default_gain")]
    pub gain: f32,
}

fn default_sample_rate() -> u32 {
    16000
}
fn default_chunk_size() -> u32 {
    512
}
fn default_gain() -> f32 {
    1.0
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct VadConfig {
    #[serde(default = "default_threshold_start")]
    pub threshold_start: f32,
    #[serde(default = "default_threshold_stop")]
    pub threshold_stop: f32,
    #[serde(default = "default_min_speech_duration")]
    pub min_speech_duration_ms: u32,
    #[serde(default = "default_min_silence_duration")]
    pub min_silence_duration_ms: u32,
}

fn default_min_speech_duration() -> u32 {
    250
}
fn default_min_silence_duration() -> u32 {
    1000
}

fn default_threshold_start() -> f32 {
    0.02
}

fn default_threshold_stop() -> f32 {
    0.01
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct WhisperConfig {
    #[serde(default)]
    pub model_path: Option<String>,
    #[serde(default = "default_model_url")]
    pub model_url: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_backend")]
    pub backend: String,
}

fn default_model_url() -> String {
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".to_string()
}
fn default_language() -> String {
    "auto".to_string()
}

fn default_backend() -> String {
    "cpu".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct OutputConfig {
    #[serde(default = "default_typing_mode")]
    pub typing_mode: String,
}

fn default_typing_mode() -> String {
    "instant".to_string()
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
                threshold_start: 0.02,
                threshold_stop: 0.01,
                min_speech_duration_ms: 250,
                min_silence_duration_ms: 1000,
            },
            whisper: WhisperConfig {
                model_path: None,
                model_url:
                    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
                        .to_string(),
                language: "auto".to_string(),
                backend: "cpu".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert_eq!(config.audio.device, "default");
        assert_eq!(config.audio.sample_rate, 16000);
        assert_eq!(config.audio.chunk_size, 512);
        assert_eq!(config.audio.gain, 1.0);

        assert_eq!(config.vad.threshold_start, 0.02);
        assert_eq!(config.vad.threshold_stop, 0.01);
        assert_eq!(config.vad.min_speech_duration_ms, 250);
        assert_eq!(config.vad.min_silence_duration_ms, 1000);

        assert_eq!(
            config.whisper.model_url,
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
        );
        assert_eq!(config.whisper.language, "auto");
        assert_eq!(config.whisper.backend, "cpu");

        assert_eq!(config.output.typing_mode, "instant");
    }

    #[test]
    fn test_config_toml_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();

        assert!(toml_str.contains("[audio]"));
        assert!(toml_str.contains("[vad]"));
        assert!(toml_str.contains("[whisper]"));
        assert!(toml_str.contains("[output]"));
    }

    #[test]
    fn test_config_toml_round_trip() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.audio, parsed.audio);
        assert_eq!(config.vad, parsed.vad);
        assert_eq!(config.whisper, parsed.whisper);
        assert_eq!(config.output, parsed.output);
    }

    #[test]
    fn test_config_with_custom_audio() {
        let toml_str = r#"
            [audio]
            device = "custom_device"
            sample_rate = 48000
            chunk_size = 1024
            gain = 2.5

            [vad]
            threshold_start = 0.05
            threshold_stop = 0.02
            min_speech_duration_ms = 500
            min_silence_duration_ms = 2000

            [whisper]
            model_url = "http://example.com/model.bin"
            language = "en"
            backend = "gpu"

            [output]
            typing_mode = "delayed"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();

        assert_eq!(config.audio.device, "custom_device");
        assert_eq!(config.audio.sample_rate, 48000);
        assert_eq!(config.audio.chunk_size, 1024);
        assert_eq!(config.audio.gain, 2.5);
        assert_eq!(config.vad.threshold_start, 0.05);
        assert_eq!(config.vad.threshold_stop, 0.02);
        assert_eq!(config.vad.min_speech_duration_ms, 500);
        assert_eq!(config.vad.min_silence_duration_ms, 2000);
        assert_eq!(config.whisper.model_url, "http://example.com/model.bin");
        assert_eq!(config.whisper.language, "en");
        assert_eq!(config.whisper.backend, "gpu");
        assert_eq!(config.output.typing_mode, "delayed");
    }

    #[test]
    fn test_config_with_missing_fields_uses_defaults() {
        let toml_str = r#"
            [audio]
            device = "partial"

            [vad]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();

        assert_eq!(config.audio.device, "partial");
        assert_eq!(config.audio.sample_rate, 16000);
        assert_eq!(config.audio.chunk_size, 512);
        assert_eq!(config.audio.gain, 1.0);
        assert_eq!(config.vad.threshold_start, 0.02);
    }

    #[test]
    fn test_config_with_invalid_toml() {
        let toml_str = "invalid toml content [unclosed";
        let result: Result<Config, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_invalid_types() {
        let toml_str = r#"
            [audio]
            sample_rate = "not_a_number"
        "#;
        let result: Result<Config, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_threshold_start() {
        let value = default_threshold_start();
        assert_eq!(value, 0.02);
    }

    #[test]
    fn test_default_threshold_stop() {
        let value = default_threshold_stop();
        assert_eq!(value, 0.01);
    }

    #[test]
    fn test_default_backend() {
        let value = default_backend();
        assert_eq!(value, "cpu");
    }

    #[test]
    fn test_audio_config_partial_specification() {
        let toml_str = r#"
            [audio]
            device = "test"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.audio.device, "test");
        assert_eq!(config.audio.sample_rate, 16000);
    }

    #[test]
    fn test_model_path_none_by_default() {
        let config = Config::default();
        assert!(config.whisper.model_path.is_none());
    }

    #[test]
    fn test_model_path_with_value() {
        let toml_str = r#"
            [whisper]
            model_path = "/custom/path/model.bin"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.whisper.model_path,
            Some("/custom/path/model.bin".to_string())
        );
    }
}
