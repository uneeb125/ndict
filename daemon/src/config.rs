use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default = "Config::default")]
pub struct Config {
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub vad: VadConfig,
    #[serde(default)]
    pub whisper: WhisperConfig,
    #[serde(default)]
    pub streaming: StreamingConfig,
    #[serde(default)]
    pub buffer: BufferConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub timeouts: TimeoutsConfig,
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
    #[serde(default = "default_channels")]
    pub channels: u16,
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
fn default_channels() -> u16 {
    1
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
    #[serde(default)]
    pub model_checksum: Option<String>,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_n_thread")]
    pub n_thread: u32,
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_streaming_mode")]
    pub streaming_mode: bool,
    #[serde(default = "default_min_audio_samples")]
    pub min_audio_samples: usize,
    #[serde(default = "default_sampling_strategy")]
    pub sampling_strategy: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct StreamingConfig {
    #[serde(default = "default_streaming_step_ms")]
    pub step_ms: u32,
    #[serde(default = "default_streaming_length_ms")]
    pub length_ms: u32,
    #[serde(default = "default_streaming_keep_ms")]
    pub keep_ms: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BufferConfig {
    #[serde(default)]
    pub broadcast_capacity: usize,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            broadcast_capacity: default_broadcast_capacity(),
        }
    }
}

fn default_model_url() -> String {
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_n_thread() -> u32 {
    4
}

fn default_backend() -> String {
    "cpu".to_string()
}

fn default_streaming_mode() -> bool {
    false
}

fn default_min_audio_samples() -> usize {
    18000
}

fn default_sampling_strategy() -> String {
    "greedy".to_string()
}

fn default_streaming_step_ms() -> u32 {
    3000
}

fn default_streaming_length_ms() -> u32 {
    10000
}

fn default_streaming_keep_ms() -> u32 {
    500
}

fn default_broadcast_capacity() -> usize {
    100
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct OutputConfig {
    #[serde(default = "default_typing_mode")]
    pub typing_mode: String,
}

fn default_typing_mode() -> String {
    "instant".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct RateLimitConfig {
    #[serde(default = "default_commands_per_second")]
    pub commands_per_second: u32,
    #[serde(default = "default_burst_capacity")]
    pub burst_capacity: u32,
    #[serde(default = "default_rate_limit_enabled")]
    pub enabled: bool,
}

fn default_commands_per_second() -> u32 {
    10
}

fn default_burst_capacity() -> u32 {
    20
}

fn default_rate_limit_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TimeoutsConfig {
    #[serde(default = "default_whisper_timeout")]
    pub whisper_timeout_seconds: u64,
    #[serde(default = "default_keyboard_timeout")]
    pub keyboard_timeout_seconds: u64,
    #[serde(default = "default_socket_connect_timeout")]
    pub socket_connect_timeout_seconds: u64,
    #[serde(default = "default_socket_operation_timeout")]
    pub socket_operation_timeout_seconds: u64,
    #[serde(default = "default_model_download_timeout")]
    pub model_download_timeout_seconds: u64,
}

impl Default for TimeoutsConfig {
    fn default() -> Self {
        Self {
            whisper_timeout_seconds: default_whisper_timeout(),
            keyboard_timeout_seconds: default_keyboard_timeout(),
            socket_connect_timeout_seconds: default_socket_connect_timeout(),
            socket_operation_timeout_seconds: default_socket_operation_timeout(),
            model_download_timeout_seconds: default_model_download_timeout(),
        }
    }
}

fn default_whisper_timeout() -> u64 {
    30
}

fn default_keyboard_timeout() -> u64 {
    5
}

fn default_socket_connect_timeout() -> u64 {
    5
}

fn default_socket_operation_timeout() -> u64 {
    10
}

fn default_model_download_timeout() -> u64 {
    300
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig {
                device: "default".to_string(),
                sample_rate: 16000,
                chunk_size: 512,
                gain: 1.0,
                channels: 1,
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
                model_checksum: None,
                language: "en".to_string(),
                n_thread: 4,
                backend: "cpu".to_string(),
                streaming_mode: false,
                min_audio_samples: 18000,
                sampling_strategy: "greedy".to_string(),
            },
            streaming: StreamingConfig {
                step_ms: 3000,
                length_ms: 10000,
                keep_ms: 500,
            },
            buffer: BufferConfig {
                broadcast_capacity: 100,
            },
            output: OutputConfig {
                typing_mode: "instant".to_string(),
            },
            rate_limit: RateLimitConfig {
                commands_per_second: 10,
                burst_capacity: 20,
                enabled: true,
            },
            timeouts: TimeoutsConfig {
                whisper_timeout_seconds: 30,
                keyboard_timeout_seconds: 5,
                socket_connect_timeout_seconds: 5,
                socket_operation_timeout_seconds: 10,
                model_download_timeout_seconds: 300,
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
        assert_eq!(config.audio.channels, 1);

        assert_eq!(config.vad.threshold_start, 0.02);
        assert_eq!(config.vad.threshold_stop, 0.01);
        assert_eq!(config.vad.min_speech_duration_ms, 250);
        assert_eq!(config.vad.min_silence_duration_ms, 1000);

        assert_eq!(
            config.whisper.model_url,
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
        );
        assert_eq!(config.whisper.model_checksum, None);
        assert_eq!(config.whisper.language, "en");
        assert_eq!(config.whisper.backend, "cpu");
        assert_eq!(config.whisper.n_thread, 4);
        assert_eq!(config.whisper.streaming_mode, false);
        assert_eq!(config.whisper.min_audio_samples, 18000);
        assert_eq!(config.whisper.sampling_strategy, "greedy");

        assert_eq!(config.streaming.step_ms, 3000);
        assert_eq!(config.streaming.length_ms, 10000);
        assert_eq!(config.streaming.keep_ms, 500);

        assert_eq!(config.buffer.broadcast_capacity, 100);

        assert_eq!(config.output.typing_mode, "instant");

        assert_eq!(config.rate_limit.commands_per_second, 10);
        assert_eq!(config.rate_limit.burst_capacity, 20);
        assert_eq!(config.rate_limit.enabled, true);

        assert_eq!(config.timeouts.whisper_timeout_seconds, 30);
        assert_eq!(config.timeouts.keyboard_timeout_seconds, 5);
        assert_eq!(config.timeouts.socket_connect_timeout_seconds, 5);
        assert_eq!(config.timeouts.socket_operation_timeout_seconds, 10);
        assert_eq!(config.timeouts.model_download_timeout_seconds, 300);
    }

    #[test]
    fn test_config_toml_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();

        assert!(toml_str.contains("[audio]"));
        assert!(toml_str.contains("[vad]"));
        assert!(toml_str.contains("[whisper]"));
        assert!(toml_str.contains("[streaming]"));
        assert!(toml_str.contains("[buffer]"));
        assert!(toml_str.contains("[output]"));
        assert!(toml_str.contains("[rate_limit]"));
        assert!(toml_str.contains("[timeouts]"));
    }

    #[test]
    fn test_config_toml_round_trip() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.audio, parsed.audio);
        assert_eq!(config.vad, parsed.vad);
        assert_eq!(config.whisper, parsed.whisper);
        assert_eq!(config.streaming, parsed.streaming);
        assert_eq!(config.buffer, parsed.buffer);
        assert_eq!(config.output, parsed.output);
        assert_eq!(config.timeouts, parsed.timeouts);
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

    #[test]
    fn test_model_checksum_with_value() {
        let toml_str = r#"
            [whisper]
            model_checksum = "abc123def456"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.whisper.model_checksum,
            Some("abc123def456".to_string())
        );
    }

    #[test]
    fn test_model_checksum_none_by_default() {
        let config = Config::default();
        assert!(config.whisper.model_checksum.is_none());
    }

    #[test]
    fn test_default_rate_limit_config() {
        let config = Config::default();
        assert_eq!(config.rate_limit.commands_per_second, 10);
        assert_eq!(config.rate_limit.burst_capacity, 20);
        assert_eq!(config.rate_limit.enabled, true);
    }

    #[test]
    fn test_rate_limit_with_custom_values() {
        let toml_str = r#"
            [rate_limit]
            commands_per_second = 5
            burst_capacity = 10
            enabled = false
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.rate_limit.commands_per_second, 5);
        assert_eq!(config.rate_limit.burst_capacity, 10);
        assert_eq!(config.rate_limit.enabled, false);
    }

    #[test]
    fn test_default_commands_per_second() {
        assert_eq!(default_commands_per_second(), 10);
    }

    #[test]
    fn test_default_burst_capacity() {
        assert_eq!(default_burst_capacity(), 20);
    }

    #[test]
    fn test_default_rate_limit_enabled() {
        assert_eq!(default_rate_limit_enabled(), true);
    }

    #[test]
    fn test_default_timeouts_config() {
        let config = Config::default();
        assert_eq!(config.timeouts.whisper_timeout_seconds, 30);
        assert_eq!(config.timeouts.keyboard_timeout_seconds, 5);
        assert_eq!(config.timeouts.socket_connect_timeout_seconds, 5);
        assert_eq!(config.timeouts.socket_operation_timeout_seconds, 10);
        assert_eq!(config.timeouts.model_download_timeout_seconds, 300);
    }

    #[test]
    fn test_timeouts_with_custom_values() {
        let toml_str = r#"
            [timeouts]
            whisper_timeout_seconds = 60
            keyboard_timeout_seconds = 10
            socket_connect_timeout_seconds = 15
            socket_operation_timeout_seconds = 20
            model_download_timeout_seconds = 600
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.timeouts.whisper_timeout_seconds, 60);
        assert_eq!(config.timeouts.keyboard_timeout_seconds, 10);
        assert_eq!(config.timeouts.socket_connect_timeout_seconds, 15);
        assert_eq!(config.timeouts.socket_operation_timeout_seconds, 20);
        assert_eq!(config.timeouts.model_download_timeout_seconds, 600);
    }

    #[test]
    fn test_timeouts_with_partial_values() {
        let toml_str = r#"
            [timeouts]
            whisper_timeout_seconds = 45
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.timeouts.whisper_timeout_seconds, 45);
        assert_eq!(config.timeouts.keyboard_timeout_seconds, 5); // default
        assert_eq!(config.timeouts.socket_connect_timeout_seconds, 5); // default
        assert_eq!(config.timeouts.socket_operation_timeout_seconds, 10); // default
        assert_eq!(config.timeouts.model_download_timeout_seconds, 300); // default
    }

    #[test]
    fn test_default_whisper_timeout() {
        assert_eq!(default_whisper_timeout(), 30);
    }

    #[test]
    fn test_default_keyboard_timeout() {
        assert_eq!(default_keyboard_timeout(), 5);
    }

    #[test]
    fn test_default_socket_connect_timeout() {
        assert_eq!(default_socket_connect_timeout(), 5);
    }

    #[test]
    fn test_default_socket_operation_timeout() {
        assert_eq!(default_socket_operation_timeout(), 10);
    }

    #[test]
    fn test_default_model_download_timeout() {
        assert_eq!(default_model_download_timeout(), 300);
    }

    #[test]
    fn test_config_with_missing_timeouts_section() {
        let toml_str = r#"
            [audio]
            device = "test"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // Should use defaults for timeouts section
        assert_eq!(config.timeouts.whisper_timeout_seconds, 30);
        assert_eq!(config.timeouts.keyboard_timeout_seconds, 5);
    }

    #[test]
    fn test_default_channels() {
        assert_eq!(default_channels(), 1);
    }

    #[test]
    fn test_default_broadcast_capacity() {
        assert_eq!(default_broadcast_capacity(), 100);
    }

    #[test]
    fn test_default_min_audio_samples() {
        assert_eq!(default_min_audio_samples(), 18000);
    }

    #[test]
    fn test_default_sampling_strategy() {
        assert_eq!(default_sampling_strategy(), "greedy");
    }

    #[test]
    fn test_config_with_new_audio_channels() {
        let toml_str = r#"
            [audio]
            device = "test"
            channels = 2
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.audio.channels, 2);
    }

    #[test]
    fn test_config_with_new_buffer_fields() {
        let toml_str = r#"
            [buffer]
            broadcast_capacity = 200
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.buffer.broadcast_capacity, 200);
    }

    #[test]
    fn test_config_with_new_whisper_fields() {
        let toml_str = r#"
            [whisper]
            min_audio_samples = 20000
            sampling_strategy = "beam"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.whisper.min_audio_samples, 20000);
        assert_eq!(config.whisper.sampling_strategy, "beam");
    }

    #[test]
    fn test_config_backwards_compatibility_buffer() {
        let toml_str = r#"
            [audio]
            device = "test"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // Should use default value for missing buffer section
        assert_eq!(config.buffer.broadcast_capacity, 100);
    }

    #[test]
    fn test_config_backwards_compatibility_whisper_fields() {
        let toml_str = r#"
            [whisper]
            language = "en"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // Should use default values for missing fields
        assert_eq!(config.whisper.min_audio_samples, 18000);
        assert_eq!(config.whisper.sampling_strategy, "greedy");
    }
}
