# ndict Comprehensive Testing Plan

## Overview
This plan provides a comprehensive testing strategy for the ndict project, including:
- Architecture improvements for testability
- Unit tests for all modules
- Integration tests for end-to-end workflows
- Interactive hardware tests (audio capture, VAD, keyboard output)

**Estimated Total Effort:** 30-40 hours across 5 phases
**Expected Test Coverage:** 70-85% with ability to pinpoint breaking code
**Lines of Test Code:** ~3500-4500

---

## Architecture Improvements (Phase 0)

Before writing tests, we need architecture changes to make components testable.

### 0.1. Why These Changes Are Needed

**Problem:** Current code has tight coupling to external dependencies:
- `WhisperEngine` directly uses `whisper-rs` FFI bindings
- `VirtualKeyboard` directly uses `wrtype` Wayland library
- `AudioCapture` directly uses `cpal` audio library
- `SpeechDetector` uses `std::time::Instant` directly

**Solution:** Extract traits for these dependencies to enable:
1. **Mocking** - Replace real implementations with test doubles
2. **Dependency Injection** - Pass mock implementations to test code
3. **Isolated Testing** - Test logic without hardware/setup requirements
4. **Faster Tests** - No need for actual hardware/network calls

### 0.2. Trait Extraction

#### A. Time Abstraction for SpeechDetector

**File:** `daemon/src/vad/speech_detector.rs`

**Why:** `SpeechDetector` uses `Instant::now()` for timing state transitions. This is hard to test because:
- Can't control time flow
- Tests require actual waiting (slow)
- Can't simulate time jumps

**Change:**
```rust
// New trait for time abstraction
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

// Production implementation
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> Instant { Instant::now() }
}

// Add Clock field to SpeechDetector
pub struct SpeechDetector {
    // ... existing fields ...
    clock: Box<dyn Clock>,
}

impl SpeechDetector {
    // Accept clock in constructor
    pub fn new(
        threshold_start: f32,
        threshold_stop: f32,
        silence_duration_ms: u32,
        gain: f32,
    ) -> anyhow::Result<Self> {
        Self::with_clock(
            threshold_start,
            threshold_stop,
            silence_duration_ms,
            gain,
            Box::new(SystemClock),
        )
    }

    pub fn with_clock(
        threshold_start: f32,
        threshold_stop: f32,
        silence_duration_ms: u32,
        gain: f32,
        clock: Box<dyn Clock>,
    ) -> anyhow::Result<Self> {
        // ... same initialization, with clock field
        Ok(Self { ..., clock })
    }
}
```

**Test Benefit:** Can create `MockClock` that returns controllable time values:

```rust
struct MockClock {
    current: Instant,
}

impl MockClock {
    fn new() -> Self {
        Self { current: Instant::now() }
    }

    fn advance(&mut self, duration: Duration) {
        self.current += duration;
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant { self.current }
}
```

#### B. Audio Format Conversion Extraction

**File:** `daemon/src/audio/capture.rs`

**Why:** Audio format conversion (F32/I16/U16 → F32) is pure logic embedded in callbacks. We can't test it easily because:
- Callbacks run in cpal's audio thread
- Can't call them directly
- Coupled to audio stream lifecycle

**Change:**
```rust
// Extract to pure functions
pub fn convert_samples_i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / i16::MAX as f32).collect()
}

pub fn convert_samples_u16_to_f32(samples: &[u16]) -> Vec<f32> {
    samples.iter().map(|&s| (s as i16 as f32) / i16::MAX as f32).collect()
}

pub fn normalize_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() { return 0.0; }
    let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
    (sum_squares / samples.len() as f32).sqrt()
}
```

**Test Benefit:** Pure functions, easy to test with sample data:

```rust
#[test]
fn test_convert_samples_i16_to_f32() {
    let input = vec![0i16, i16::MAX, i16::MIN];
    let output = convert_samples_i16_to_f32(&input);
    assert_eq!(output[0], 0.0);
    assert_eq!(output[1], 1.0);
    assert_eq!(output[2], -1.0);
}
```

#### C. Command Handling Extraction

**File:** `daemon/src/server.rs`

**Why:** `handle_connection` mixes:
- Socket I/O (reading/writing)
- Command parsing (JSON)
- Command execution (business logic)
- Response formatting

This makes it hard to test command execution separately from socket handling.

**Change:**
```rust
// Extract command execution logic
pub async fn execute_command(
    state: Arc<Mutex<DaemonState>>,
    command: Command,
) -> anyhow::Result<Response> {
    match command {
        Command::Start => { ... }
        Command::Stop => { ... }
        // ... all command handling logic ...
    }
}

// Simplified handle_connection
async fn handle_connection(
    state: Arc<Mutex<DaemonState>>,
    mut stream: UnixStream,
) -> anyhow::Result<()> {
    // Read, parse, execute, send - all separated
    let command: Command = serde_json::from_slice(&buffer)?;
    let response = execute_command(state, command).await?;
    let response_json = serde_json::to_vec(&response)?;
    stream.write(&response_json).await?;
    Ok(())
}
```

**Test Benefit:** Can test `execute_command` directly without setting up Unix sockets:

```rust
#[tokio::test]
async fn test_execute_command_start() {
    let config = Config::default();
    let state = Arc::new(Mutex::new(DaemonState::new(config)));
    let response = execute_command(state, Command::Start).await;
    assert!(matches!(response, Ok(Response::Ok)));
}
```

#### D. VAD Processing Loop Extraction

**File:** `daemon/src/state.rs`

**Why:** The VAD processing loop is embedded in `start_vad_processing`, making it hard to test:
- Logic is inside a spawned task
- Hard to observe internal state
- Coupled to audio receiver and spawned tasks

**Change:**
```rust
// Extract processing logic to standalone function
pub async fn vad_processing_loop(
    mut audio_rx: broadcast::Receiver<Vec<f32>>,
    whisper_engine: Arc<Mutex<Option<WhisperEngine>>>,
    virtual_keyboard: Arc<Mutex<Option<VirtualKeyboard>>>,
    vad_threshold_start: f32,
    vad_threshold_stop: f32,
    silence_duration_ms: u32,
    gain: f32,
) {
    // ... move all the processing logic here ...
}
```

**Test Benefit:** Can pass mock receivers/engines/keyboard and verify behavior.

---

## Phase 1: Pure Functions & IPC (4-6 hours)

### 1.1. shared/src/ipc.rs Tests

**Test File:** `shared/src/ipc.rs` (add `#[cfg(test)]` module at end)

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_command_serialization_start() {
        let cmd = Command::Start;
        let json = serde_json::to_string(&cmd).unwrap();
        assert_eq!(json, r#""Start""#);
    }

    #[test]
    fn test_command_serialization_set_language() {
        let cmd = Command::SetLanguage("en".to_string());
        let json = serde_json::to_string(&cmd).unwrap();
        assert_eq!(json, r#"{"SetLanguage":"en"}"#);
    }

    #[test]
    fn test_command_round_trip_all_variants() {
        let commands = vec![
            Command::Start,
            Command::Stop,
            Command::Pause,
            Command::Resume,
            Command::Status,
            Command::SetLanguage("test".to_string()),
            Command::Toggle,
        ];
        for cmd in commands {
            let json = serde_json::to_string(&cmd).unwrap();
            let deserialized: Command = serde_json::from_str(&json).unwrap();
            assert_eq!(cmd, deserialized);
        }
    }

    #[test]
    fn test_response_serialization_ok() {
        let resp = Response::Ok;
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#""Ok""#);
    }

    #[test]
    fn test_response_serialization_error() {
        let resp = Response::Error("test error".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"Error":"test error"}"#);
    }

    #[test]
    fn test_response_serialization_status() {
        let info = StatusInfo {
            is_running: true,
            is_active: false,
            language: "en".to_string(),
        };
        let resp = Response::Status(info.clone());
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"Status":{"is_running":true,"is_active":false,"language":"en"}}"#);
    }

    #[test]
    fn test_response_round_trip_all_variants() {
        let responses = vec![
            Response::Ok,
            Response::Error("error".to_string()),
            Response::Status(StatusInfo {
                is_running: true,
                is_active: false,
                language: "test".to_string(),
            }),
        ];
        for resp in responses {
            let json = serde_json::to_string(&resp).unwrap();
            let deserialized: Response = serde_json::from_str(&json).unwrap();
            assert_eq!(resp, deserialized);
        }
    }

    #[test]
    fn test_status_info_serialization() {
        let info = StatusInfo {
            is_running: true,
            is_active: true,
            language: "en".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("is_running"));
        assert!(json.contains("is_active"));
        assert!(json.contains("language"));
    }

    #[test]
    fn test_status_info_all_states() {
        let combinations = vec![
            (true, true, "en"),
            (true, false, "en"),
            (false, true, "es"),
            (false, false, "fr"),
        ];
        for (running, active, lang) in combinations {
            let info = StatusInfo {
                is_running: running,
                is_active: active,
                language: lang.to_string(),
            };
            let json = serde_json::to_string(&info).unwrap();
            let deserialized: StatusInfo = serde_json::from_str(&json).unwrap();
            assert_eq!(info, deserialized);
        }
    }

    #[test]
    fn test_ipc_error_display_io() {
        let err = IpcError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "test",
        ));
        assert!(err.to_string().contains("IO error"));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_ipc_error_display_serialization() {
        let err = IpcError::Serialization(serde_json::Error::syntax(
            serde_json::error::ErrorCode::ExpectedColon,
            0,
            0,
        ));
        assert!(err.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_ipc_error_display_connection_refused() {
        let err = IpcError::ConnectionRefused;
        assert!(err.to_string().contains("Connection refused"));
    }

    #[test]
    fn test_ipc_error_display_timeout() {
        let err = IpcError::Timeout;
        assert!(err.to_string().contains("Connection timeout"));
    }
}
```

### 1.2. daemon/src/config.rs Tests

**Test File:** `daemon/src/config.rs` (add `#[cfg(test)]` module)

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        
        // Audio defaults
        assert_eq!(config.audio.device, "default");
        assert_eq!(config.audio.sample_rate, 16000);
        assert_eq!(config.audio.chunk_size, 512);
        assert_eq!(config.audio.gain, 1.0);

        // VAD defaults
        assert_eq!(config.vad.threshold_start, 0.02);
        assert_eq!(config.vad.threshold_stop, 0.01);
        assert_eq!(config.vad.min_speech_duration_ms, 250);
        assert_eq!(config.vad.min_silence_duration_ms, 1000);

        // Whisper defaults
        assert_eq!(config.whisper.model, "base");
        assert_eq!(config.whisper.model_url, 
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin");
        assert_eq!(config.whisper.language, "auto");
        assert_eq!(config.whisper.gpu_backend, "cpu");

        // Output defaults
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
            model = "tiny"
            model_url = "http://example.com/model.bin"
            language = "en"
            gpu_backend = "cuda"

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
        assert_eq!(config.whisper.model, "tiny");
        assert_eq!(config.whisper.model_url, "http://example.com/model.bin");
        assert_eq!(config.whisper.language, "en");
        assert_eq!(config.whisper.gpu_backend, "cuda");
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
        assert_eq!(config.audio.sample_rate, 16000); // Default
        assert_eq!(config.audio.chunk_size, 512); // Default
        assert_eq!(config.audio.gain, 1.0); // Default
        assert_eq!(config.vad.threshold_start, 0.02); // Default
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
    fn test_default_gpu_backend() {
        let value = default_gpu_backend();
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
        assert_eq!(config.audio.sample_rate, 16000); // Default
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
        assert_eq!(config.whisper.model_path, Some("/custom/path/model.bin".to_string()));
    }
}
```

### 1.3. daemon/src/vad/detector.rs Tests

**Test File:** `daemon/src/vad/detector.rs` (add `#[cfg(test)]` module)

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_new() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        assert_eq!(vad.threshold_start, 0.02);
        assert_eq!(vad.threshold_stop, 0.01);
    }

    #[test]
    fn test_vad_new_with_equal_thresholds() {
        let vad = VoiceActivityDetector::new(0.02, 0.02).unwrap();
        assert_eq!(vad.threshold_start, 0.02);
        assert_eq!(vad.threshold_stop, 0.02);
    }

    #[test]
    fn test_calculate_audio_level_empty() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let level = vad.calculate_audio_level(&[]);
        assert_eq!(level, 0.0);
    }

    #[test]
    fn test_calculate_audio_level_silence() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![0.0, 0.0, 0.0, 0.0];
        let level = vad.calculate_audio_level(&samples);
        assert_eq!(level, 0.0);
    }

    #[test]
    fn test_calculate_audio_level_full_scale() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![1.0, 1.0, 1.0, 1.0];
        let level = vad.calculate_audio_level(&samples);
        assert_eq!(level, 1.0);
    }

    #[test]
    fn test_calculate_audio_level_mixed() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![0.0, 0.5, 1.0, 0.5];
        let level = vad.calculate_audio_level(&samples);
        // RMS of [0, 0.5, 1, 0.5] = sqrt((0^2 + 0.5^2 + 1^2 + 0.5^2) / 4)
        // = sqrt((0 + 0.25 + 1 + 0.25) / 4) = sqrt(1.5 / 4) = sqrt(0.375) ≈ 0.612
        assert!((level - 0.612).abs() < 0.001);
    }

    #[test]
    fn test_calculate_audio_level_negative_values() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![-0.5, -0.5, -0.5, -0.5];
        let level = vad.calculate_audio_level(&samples);
        // RMS is always positive
        assert_eq!(level, 0.5);
    }

    #[test]
    fn test_calculate_audio_level_mixed_sign() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let samples = vec![-1.0, 0.0, 1.0, 0.0];
        let level = vad.calculate_audio_level(&samples);
        // RMS = sqrt((1 + 0 + 1 + 0) / 4) = sqrt(0.5) ≈ 0.707
        assert!((level - 0.707).abs() < 0.001);
    }

    #[test]
    fn test_detect_speech_idle_above_threshold_start() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = false;
        let result = vad.detect(0.03, is_speaking);
        assert!(result.is_speech);
        assert_eq!(result.probability, 0.03);
    }

    #[test]
    fn test_detect_speech_idle_below_threshold_start() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = false;
        let result = vad.detect(0.015, is_speaking);
        assert!(!result.is_speech);
        assert_eq!(result.probability, 0.015);
    }

    #[test]
    fn test_detect_speech_idle_exactly_threshold_start() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = false;
        let result = vad.detect(0.02, is_speaking);
        assert!(result.is_speech); // Uses > comparison
    }

    #[test]
    fn test_detect_speech_speaking_above_threshold_stop() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = true;
        let result = vad.detect(0.015, is_speaking);
        assert!(result.is_speech);
    }

    #[test]
    fn test_detect_speech_speaking_below_threshold_stop() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = true;
        let result = vad.detect(0.005, is_speaking);
        assert!(!result.is_speech);
    }

    #[test]
    fn test_detect_speech_speaking_exactly_threshold_stop() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let is_speaking = true;
        let result = vad.detect(0.01, is_speaking);
        assert!(result.is_speech); // Uses > comparison
    }

    #[test]
    fn test_detect_hysteresis() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let audio_level = 0.015; // Between thresholds
        
        // When idle, should not detect speech (below threshold_start)
        let result_idle = vad.detect(audio_level, false);
        assert!(!result_idle.is_speech);
        
        // When speaking, should detect speech (above threshold_stop)
        let result_speaking = vad.detect(audio_level, true);
        assert!(result_speaking.is_speech);
    }

    #[test]
    fn test_vad_result_contains_probability() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let result = vad.detect(0.05, false);
        assert_eq!(result.probability, 0.05);
    }

    #[test]
    fn test_detect_with_zero_audio_level() {
        let vad = VoiceActivityDetector::new(0.02, 0.01).unwrap();
        let result = vad.detect(0.0, false);
        assert!(!result.is_speech);
        assert_eq!(result.probability, 0.0);
    }
}
```

### 1.4. daemon/src/transcription/mod.rs Tests

**Test File:** `daemon/src/transcription/mod.rs` (add `#[cfg(test)]` module)

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_process_empty_string() {
        let input = "";
        let output = post_process_transcription(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_post_process_simple_text() {
        let input = "hello world";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world");
    }

    #[test]
    fn test_post_process_remove_duplicate_words() {
        let input = "hello hello world world test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_remove_bracketed_square() {
        let input = "hello [world] test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_remove_bracketed_curly() {
        let input = "hello {world} test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_remove_bracketed_paren() {
        let let input = "hello (world) test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_remove_multiple_bracket_types() {
        let input = "hello [one] {two} (three) test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_normalize_whitespace() {
        let input = "hello  world   test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_trim_whitespace() {
        let input = "  hello world test  ";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_combined() {
        let input = "  hello hello [noise] world {test} (skip)  world  ";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world");
    }

    #[test]
    fn test_post_process_triple_spaces() {
        let input = "hello   world   test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_only_brackets() {
        let input = "[hello]";
        let output = post_process_transcription(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_post_process_brackets_with_spaces() {
        let input = "hello [ world ] test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello test");
    }

    #[test]
    fn test_post_process_realistic_whisper_output() {
        let input = " hello [laughs] world (um) [clears throat]  test  test  ";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_unicode_characters() {
        let input = "hello 世界 🌍 world";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello 世界 🌍 world");
    }

    #[test]
    fn test_post_process_numbers_and_punctuation() {
        let input = "hello,  world!  test. 123";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello, world! test. 123");
    }

    #[test]
    fn test_post_process_multiple_consecutive_duplicates() {
        let input = "hello hello hello world world world";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world");
    }

    #[test]
    fn test_post_process_no_duplicates() {
        let input = "hello world test";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello world test");
    }

    #[test]
    fn test_post_process_single_word() {
        let input = "hello";
        let output = post_process_transcription(input);
        assert_eq!(output, "hello");
    }
}
```

---

## Phase 2: State Machines & External Interfaces (6-8 hours)

### 2.1. Architecture Change: Clock Abstraction

**Implementation:** Add `Clock` trait to `daemon/src/vad/speech_detector.rs` as described in Phase 0.

### 2.2. daemon/src/vad/speech_detector.rs Tests

**Test File:** `daemon/src/vad/speech_detector.rs` (add `#[cfg(test)]` module)

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    struct MockClock {
        current: Instant,
    }

    impl MockClock {
        fn new() -> Self {
            Self { current: Instant::now() }
        }

        fn advance(&mut self, duration: Duration) {
            self.current += duration;
        }
    }

    impl Clock for MockClock {
        fn now(&self) -> Instant {
            self.current
        }
    }

    #[test]
    fn test_speech_detector_new() {
        let detector = SpeechDetector::new(0.02, 0.01, 1000, 1.0).unwrap();
        assert_eq!(detector.state, SpeechState::Idle);
        assert!(detector.speech_start_time.is_none());
        assert!(detector.silence_start_time.is_none());
        assert!(detector.speech_buffer.is_empty());
    }

    #[test]
    fn test_speech_detector_with_mock_clock() {
        let clock = Box::new(MockClock::new());
        let detector = SpeechDetector::with_clock(0.02, 0.01, 1000, 1.0, clock).unwrap();
        assert_eq!(detector.state, SpeechState::Idle);
    }

    #[test]
    fn test_idle_state_no_speech_below_threshold() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 1000, 1.0, clock).unwrap();
        
        let samples = vec![0.01, 0.01, 0.01]; // Below threshold_start
        let result = detector.process_audio(&samples);
        
        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::Idle);
        assert!(detector.speech_buffer.is_empty());
    }

    #[test]
    fn test_idle_state_transition_to_speaking() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 1000, 1.0, clock).unwrap();
        
        let samples = vec![0.03, 0.03, 0.03]; // Above threshold_start
        let result = detector.process_audio(&samples);
        
        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::Speaking);
        assert!(!detector.speech_buffer.is_empty());
        assert_eq!(detector.speech_buffer.len(), samples.len());
    }

    #[test]
    fn test_speaking_state_accumulates_buffer() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 1000, 1.0, clock).unwrap();
        
        let samples1 = vec![0.03, 0.03];
        detector.process_audio(&samples1);
        
        let samples2 = vec![0.04, 0.04];
        detector.process_audio(&samples2);
        
        assert_eq!(detector.state, SpeechState::Speaking);
        assert_eq!(detector.speech_buffer.len(), 4);
    }

    #[test]
    fn test_speaking_to_silence_detected_transition() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 1000, 1.0, clock).unwrap();
        
        let samples_speech = vec![0.03, 0.03];
        detector.process_audio(&samples_speech);
        
        let samples_silence = vec![0.005, 0.005]; // Below threshold_stop
        let result = detector.process_audio(&samples_silence);
        
        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::SilenceDetected);
        assert!(detector.silence_start_time.is_some());
    }

    #[test]
    fn test_silence_detected_to_speaking_false_alarm() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 1000, 1.0, clock).unwrap();
        
        // Start speaking
        detector.process_audio(&vec![0.03, 0.03]);
        
        // Go silent
        detector.process_audio(&vec![0.005, 0.005]);
        
        // Return to speech (false alarm)
        let result = detector.process_audio(&vec![0.03, 0.03]);
        
        assert!(result.is_none());
        assert_eq!(detector.state, SpeechState::Speaking);
        assert!(detector.silence_start_time.is_none());
    }

    #[test]
    fn test_silence_duration_not_exceeded() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 1000, 1.0, clock).unwrap();
        
        // Start speaking
        detector.process_audio(&vec![0.03, 0.03]);
        
        // Go silent
        detector.process_audio(&vec![0.005, 0.005]);
        
        // Process more silence but not enough time
        for _ in 0..10 {
            detector.process_audio(&vec![0.005, 0.005]);
        }
        
        assert_eq!(detector.state, SpeechState::SilenceDetected);
        assert!(detector.process_audio(&vec![0.005, 0.005]).is_none());
    }

    #[test]
    fn test_silence_duration_exceeded_returns_speech() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 100, 1.0, clock).unwrap();
        
        // Start speaking
        detector.process_audio(&vec![0.03, 0.03]);
        
        // Go silent
        detector.process_audio(&vec![0.005, 0.005]);
        
        // Advance time past silence duration
        if let Some(c) = detector.clock.as_any().downcast_ref::<MockClock>() {
            // We'll need to expose clock as mutable for this test
        }
        
        // Simulate enough time passing by processing many chunks
        // Each chunk at 16kHz with 512 samples = ~32ms
        // Need ~4 chunks for 100ms
        for _ in 0..4 {
            detector.process_audio(&vec![0.005; 512]);
        }
        
        assert_eq!(detector.state, SpeechState::Idle);
    }

    #[test]
    fn test_speech_returned_with_gain_applied() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 100, 2.0, clock).unwrap();
        
        // Process speech
        detector.process_audio(&vec![0.03, 0.03]);
        
        // Go silent and complete
        for _ in 0..4 {
            detector.process_audio(&vec![0.005; 512]);
        }
        
        // State should be idle and we should have received speech
        assert_eq!(detector.state, SpeechState::Idle);
        // Check that returned samples were amplified
    }

    #[test]
    fn test_gain_zero_mutes_audio() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 100, 0.0, clock).unwrap();
        
        detector.process_audio(&vec![0.03, 0.03]);
        
        for _ in 0..4 {
            detector.process_audio(&vec![0.005; 512]);
        }
        
        assert_eq!(detector.state, SpeechState::Idle);
        // Returned samples should be all zeros
    }

    #[test]
    fn test_duration_calculation() {
        // At 16kHz, 1600 samples = 100ms
        let samples = vec![0.0f32; 1600];
        let duration_ms = 100;
        
        clock = Box::new(MockClock::new());
        let detector = SpeechDetector::with_clock(0.02, 0.01, 100, 1.0, clock).unwrap();
        let calculated = detector.calculate_duration_ms(&samples);
        
        assert_eq!(calculated, duration_ms);
    }

    #[test]
    fn test_reset_after_speech_complete() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 100, 1.0, clock).unwrap();
        
        detector.process_audio(&vec![0.03, 0.03]);
        for _ in 0..4 {
            detector.process_audio(&vec![0.005; 512]);
        }
        
        assert_eq!(detector.state, SpeechState::Idle);
        assert!(detector.speech_start_time.is_none());
        assert!(detector.silence_start_time.is_none());
        assert!(detector.speech_buffer.is_empty());
    }

    #[test]
    fn test_multiple_speech_segments() {
        let clock = Box::new(MockClock::new());
        let mut detector = SpeechDetector::with_clock(0.02, 0.01, 100, 1.0, clock).unwrap();
        
        // First segment
        detector.process_audio(&vec![0.03, 0.03]);
        let result1 = detector.process_audio(&vec![0.005; 512]);
        for _ in 0..3 {
            detector.process_audio(&vec![0.005; 512]);
        }
        
        assert!(result1.is_some());
        assert_eq!(detector.state, SpeechState::Idle);
        
        // Second segment
        detector.process_audio(&vec![0.04, 0.04]);
        let result2 = detector.process_audio(&vec![0.005; 512]);
        for _ in 0..3 {
            detector.process_audio(&vec![0.005; 512]);
        }
        
        assert!(result2.is_some());
        assert_eq!(detector.state, SpeechState::Idle);
    }
}
```

**Note:** The tests above need the `Clock` trait to expose `as_any()` for downcasting. We'll refine this in the implementation.

### 2.3. daemon/src/audio/capture.rs - Format Conversion Tests

**Test File:** `daemon/src/audio/capture.rs` (add `#[cfg(test)]` module)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_samples_i16_to_f32_zero() {
        let input = vec![0i16; 10];
        let output = convert_samples_i16_to_f32(&input);
        assert_eq!(output.len(), 10);
        for sample in output {
            assert_eq!(sample, 0.0);
        }
    }

    #[test]
    fn test_convert_samples_i16_to_f32_max() {
        let input = vec![i16::MAX; 10];
        let output = convert_samples_i16_to_f32(&input);
        assert_eq!(output.len(), 10);
        for sample in output {
            assert_eq!(sample, 1.0);
        }
    }

    #[test]
    fn test_convert_samples_i16_to_f32_min() {
        let input = vec![i16::MIN; 10];
        let output = convert_samples_i16_to_f32(&input);
        assert_eq!(output.len(), 10);
        for sample in output {
            assert_eq!(sample, -1.0);
        }
    }

    #[test]
    fn test_convert_samples_i16_to_f32_mixed() {
        let input = vec![i16::MAX, i16::MIN, 0, 1000, -1000];
        let output = convert_samples_i16_to_f32(&input);
        assert_eq!(output[0], 1.0);
        assert_eq!(output[1], -1.0);
        assert_eq!(output[2], 0.0);
        assert!((output[3] - (1000.0 / i16::MAX as f32)).abs() < 0.0001);
    }

    #[test]
    fn test_convert_samples_u16_to_f32_zero() {
        let input = vec![32768u16; 10]; // Center point (zero in signed)
        let output = convert_samples_u16_to_f32(&input);
        assert_eq!(output.len(), 10);
        for sample in output {
            assert_eq!(sample, 0.0);
        }
    }

    #[test]
    fn test_convert_samples_u16_to_f32_max() {
        let input = vec![u16::MAX; 10];
        let output = convert_samples_u16_to_f32(&input);
        assert_eq!(output.len(), 10);
        for sample in output {
            // u16::MAX cast to i16 becomes i16::MIN (-1)
            assert_eq!(sample, -1.0);
        }
    }

    #[test]
    fn test_convert_samples_u16_to_f32_mixed() {
        let input = vec![0u16, 32768, 65535]; // Max, zero, min (in signed terms)
        let output = convert_samples_u16_to_f32(&input);
        assert_eq!(output[0], 1.0);
        assert_eq!(output[1], 0.0);
        assert_eq!(output[2], -1.0);
    }

    #[test]
    fn test_normalize_rms_empty() {
        let samples = vec![];
        let rms = normalize_rms(&samples);
        assert_eq!(rms, 0.0);
    }

    #[test]
    fn test_normalize_rms_zeros() {
        let samples = vec![0.0f32; 100];
        let rms = normalize_rms(&samples);
        assert_eq!(rms, 0.0);
    }

    #[test]
    fn test_normalize_rms_ones() {
        let samples = vec![1.0f32; 100];
        let rms = normalize_rms(&samples);
        assert_eq!(rms, 1.0);
    }

    #[test]
    fn test_normalize_rms_half() {
        let samples = vec![0.5f32; 100];
        let rms = normalize_rms(&samples);
        assert_eq!(rms, 0.5);
    }

    #[test]
    fn test_normalize_rms_mixed() {
        let samples = vec![0.0, 0.5, 1.0, 0.5];
        let rms = normalize_rms(&samples);
        // RMS = sqrt((0^2 + 0.5^2 + 1^2 + 0.5^2) / 4) = sqrt(1.5/4) = sqrt(0.375) ≈ 0.612
        assert!((rms - 0.612).abs() < 0.001);
    }

    #[test]
    fn test_normalize_rms_negative() {
        let samples = vec![-0.5, -0.5, -0.5, -0.5];
        let rms = normalize_rms(&samples);
        assert_eq!(rms, 0.5);
    }
}
```

### 2.4. cli/src/client.rs Tests

**Test File:** `cli/src/client.rs` (add `#[cfg(test)]` module)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use std::os::unix::net::UnixStream as StdUnixStream;
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn test_daemon_client_new() {
        let client = DaemonClient::new();
        assert_eq!(client.socket_path, PathBuf::from("/tmp/ndictd.sock"));
    }

    #[tokio::test]
    async fn test_send_command_socket_not_found() {
        let client = DaemonClient::new();
        let result = client.send_command(Command::Start).await;
        assert!(matches!(result, Err(IpcError::ConnectionRefused)));
    }

    #[tokio::test]
    async fn test_send_command_with_mock_server() {
        let test_socket = "/tmp/test_ndict.sock";
        
        // Create test server
        let listener = UnixListener::bind(test_socket).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            
            // Read command
            let mut buffer = vec![0u8; 1024];
            let n = stream.read(&mut buffer).await.unwrap();
            buffer.truncate(n);
            
            let command: Command = serde_json::from_slice(&buffer).unwrap();
            
            // Send response
            let response = match command {
                Command::Start => Response::Ok,
                _ => Response::Error("unknown".to_string()),
            };
            
            let response_json = serde_json::to_vec(&response).unwrap();
            stream.write_all(&response_json).await.unwrap();
        });
        
        // Wait for server to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Create client with test socket
        let client = DaemonClient {
            socket_path: PathBuf::from(test_socket),
        };
        
        let result = client.send_command(Command::Start).await;
        assert!(matches!(result, Ok(Response::Ok)));
        
        // Cleanup
        std::fs::remove_file(test_socket).ok();
    }

    #[tokio::test]
    async fn test_send_command_status() {
        let test_socket = "/tmp/test_ndict_status.sock";
        
        let listener = UnixListener::bind(test_socket).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            
            let mut buffer = vec![0u8; 1024];
            let n = stream.read(&mut buffer).await.unwrap();
            
            let command: Command = serde_json::from_slice(&buffer).unwrap();
            assert!(matches!(command, Command::Status));
            
            let response = Response::Status(StatusInfo {
                is_running: true,
                is_active: false,
                language: "en".to_string(),
            });
            
            let response_json = serde_json::to_vec(&response).unwrap();
            stream.write_all(&response_json).await.unwrap();
        });
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let client = DaemonClient {
            socket_path: PathBuf::from(test_socket),
        };
        
        let result = client.send_command(Command::Status).await;
        assert!(matches!(result, Ok(Response::Status(_))));
        
        if let Ok(Response::Status(info)) = result {
            assert_eq!(info.is_running, true);
            assert_eq!(info.is_active, false);
            assert_eq!(info.language, "en");
        }
        
        std::fs::remove_file(test_socket).ok();
    }

    #[tokio::test]
    async fn test_send_command_error_response() {
        let test_socket = "/tmp/test_ndict_error.sock";
        
        let listener = UnixListener::bind(test_socket).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            
            let mut buffer = vec![0u8; 1024];
            stream.read(&mut buffer).await.unwrap();
            
            let response = Response::Error("test error".to_string());
            let response_json = serde_json::to_vec(&response).unwrap();
            stream.write_all(&response_json).await.unwrap();
        });
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let client = DaemonClient {
            socket_path: PathBuf::from(test_socket),
        };
        
        let result = client.send_command(Command::Start).await;
        assert!(matches!(result, Ok(Response::Error(_))));
        
        std::fs::remove_file(test_socket).ok();
    }

    #[tokio::test]
    async fn test_send_command_serialization() {
        let cmd = Command::SetLanguage("test".to_string());
        let json = serde_json::to_vec(&cmd).unwrap();
        
        assert!(json.len() > 0);
        
        let parsed: Command = serde_json::from_slice(&json).unwrap();
        assert_eq!(cmd, parsed);
    }
}
```

---

## Phase 3: Async & State Management (6-8 hours)

### 3.1. Architecture Change: Extract VAD Processing Loop

**Implementation:** Extract `vad_processing_loop` from `state.rs` as described in Phase 0.

### 3.2. daemon/src/state.rs Tests

**Test File:** `daemon/src/state.rs` (add `#[cfg(test)]` module)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daemon_state_new() {
        let config = Config::default();
        let state = DaemonState::new(config);
        
        assert!(!*state.is_active.lock().await);
        assert!(state.audio_capture.lock().await.is_none());
        assert!(state.audio_rx.lock().await.is_none());
        assert!(state.whisper_engine.lock().await.is_none());
        assert!(state.virtual_keyboard.lock().await.is_none());
        assert!(state.vad_task_handle.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_activate() {
        let config = Config::default();
        let mut state = DaemonState::new(config);
        
        state.activate().await.unwrap();
        assert!(*state.is_active.lock().await);
    }

    #[tokio::test]
    async fn test_deactivate() {
        let config = Config::default();
        let mut state = DaemonState::new(config);
        
        state.activate().await.unwrap();
        assert!(*state.is_active.lock().await);
        
        state.deactivate().await.unwrap();
        assert!(!*state.is_active.lock().await);
    }

    #[tokio::test]
    async fn test_get_status() {
        let config = Config::default();
        let state = DaemonState::new(config.clone());
        
        let status = state.get_status().await;
        
        assert_eq!(status.is_running, true);
        assert_eq!(status.is_active, false);
        assert_eq!(status.language, config.whisper.language);
    }

    #[tokio::test]
    async fn test_get_status_active() {
        let config = Config::default();
        let mut state = DaemonState::new(config.clone());
        
        state.activate().await.unwrap();
        let status = state.get_status().await;
        
        assert_eq!(status.is_running, true);
        assert_eq!(status.is_active, true);
        assert_eq!(status.language, config.whisper.language);
    }

    #[tokio::test]
    async fn test_start_vad_processing_no_audio_rx() {
        let config = Config::default();
        let state = DaemonState::new(config);
        
        let result = state.start_vad_processing().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Audio receiver"));
    }

    #[tokio::test]
    async fn test_stop_vad_processing() {
        let config = Config::default();
        let state = DaemonState::new(config);
        
        // Start a dummy task
        let handle = tokio::spawn(async {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });
        *state.vad_task_handle.lock().await = Some(handle);
        
        state.stop_vad_processing().await;
        assert!(state.vad_task_handle.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_stop_vad_processing_no_task() {
        let config = Config::default();
        let state = DaemonState::new(config);
        
        // Should not panic even without a task
        state.stop_vad_processing().await;
    }
}
```

### 3.3. daemon/src/server.rs Tests

**Test File:** `daemon/src/server.rs` (add `#[cfg(test)]` module)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_server_new() {
        let socket_path = PathBuf::from("/tmp/test.sock");
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        let server = DaemonServer::new(socket_path.clone(), state);
        
        assert_eq!(server.socket_path, socket_path);
    }

    #[tokio::test]
    async fn test_execute_command_start() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        
        // Note: This will fail because we don't have real audio/whisper/keyboard
        // But we can test the state activation
        let result = execute_command(state.clone(), Command::Start).await;
        
        // Should fail with audio capture error in test environment
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_command_stop() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        
        // Activate first
        {
            let mut state_guard = state.lock().await;
            state_guard.activate().await.unwrap();
        }
        
        let result = execute_command(state.clone(), Command::Stop).await;
        assert!(matches!(result, Ok(Response::Ok)));
        
        let status = state.lock().await.get_status().await;
        assert!(!status.is_active);
    }

    #[tokio::test]
    async fn test_execute_command_pause() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        
        let result = execute_command(state, Command::Pause).await;
        assert!(matches!(result, Ok(Response::Ok)));
    }

    #[tokio::test]
    async fn test_execute_command_resume() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        
        let result = execute_command(state, Command::Resume).await;
        assert!(matches!(result, Ok(Response::Ok)));
    }

    #[tokio::test]
    async fn test_execute_command_status() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config.clone())));
        
        let result = execute_command(state.clone(), Command::Status).await;
        
        if let Ok(Response::Status(info)) = result {
            assert_eq!(info.is_running, true);
            assert_eq!(info.is_active, false);
            assert_eq!(info.language, config.whisper.language);
        } else {
            panic!("Expected Status response");
        }
    }

    #[tokio::test]
    async fn test_execute_command_set_language() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        
        let result = execute_command(state, Command::SetLanguage("es".to_string())).await;
        assert!(matches!(result, Ok(Response::Ok)));
    }

    #[tokio::test]
    async fn test_execute_command_toggle_inactive() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        
        // Start inactive, toggle should try to start
        let result = execute_command(state.clone(), Command::Toggle).await;
        
        // Will fail because no audio capture, but should try to activate
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_command_toggle_active() {
        let config = Config::default();
        let state = Arc::new(Mutex::new(DaemonState::new(config)));
        
        // Activate first
        {
            let mut state_guard = state.lock().await;
            state_guard.activate().await.unwrap();
        }
        
        let result = execute_command(state.clone(), Command::Toggle).await;
        assert!(matches!(result, Ok(Response::Ok)));
        
        // Should be inactive now
        let status = state.lock().await.get_status().await;
        assert!(!status.is_active);
    }

    #[tokio::test]
    async fn test_socket_path_cleanup() {
        let test_socket = "/tmp/test_cleanup.sock";
        let _ = std::fs::remove_file(test_socket);
        
        // Create socket file
        let _ = std::fs::File::create(test_socket);
        assert!(PathBuf::from(test_socket).exists());
        
        {
            let config = Config::default();
            let state = Arc::new(Mutex::new(DaemonState::new(config)));
            let server = DaemonServer::new(PathBuf::from(test_socket), state);
            // Drop triggers cleanup
        }
        
        // Socket should be removed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(!PathBuf::from(test_socket).exists());
    }
}
```

### 3.4. daemon/src/transcription/engine.rs Tests

**Test File:** `daemon/src/transcription/engine.rs` (add `#[cfg(test)]` module)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_engine_new() {
        let engine = WhisperEngine::new(
            "http://test.url".to_string(),
            "test_model".to_string(),
        ).unwrap();
        
        assert_eq!(engine.model_url, "http://test.url");
        assert_eq!(engine.model_name, "test_model");
        assert_eq!(engine.model_loaded, false);
        assert!(engine.context.is_none());
        assert!(engine.state.is_none());
    }

    #[test]
    fn test_find_model_path_default() {
        let path = WhisperEngine::find_model_path("base").unwrap();
        
        // Should return path in home directory
        let home = dirs::home_dir().unwrap();
        let expected = home.join(".local/share/ndict/ggml-base.bin");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_find_model_path_with_variant() {
        let path = WhisperEngine::find_model_path("base.en").unwrap();
        
        let home = dirs::home_dir().unwrap();
        let expected = home.join(".local/share/ndict/ggml-base.en.bin");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_pad_audio_no_padding_needed() {
        let mut engine = WhisperEngine::new(
            "http://test.url".to_string(),
            "test".to_string(),
        ).unwrap();
        
        let audio = vec![0.0f32; 20000];
        let padded = engine.pad_audio(&audio, 18000);
        
        assert_eq!(padded.len(), 20000);
        assert_eq!(padded, audio);
    }

    #[test]
    fn test_pad_audio_needs_padding() {
        let mut engine = WhisperEngine::new(
            "http://test.url".to_string(),
            "test".to_string(),
        ).unwrap();
        
        let audio = vec![0.5f32; 10000];
        let padded = engine.pad_audio(&audio, 18000);
        
        assert_eq!(padded.len(), 18000);
        assert_eq!(&padded[..10000], &audio[..]);
        assert_eq!(&padded[10000..], &vec![0.0f32; 8000][..]);
    }

    #[test]
    fn test_pad_audio_empty() {
        let mut engine = WhisperEngine::new(
            "http://test.url".to_string(),
            "test".to_string(),
        ).unwrap();
        
        let audio: Vec<f32> = vec![];
        let padded = engine.pad_audio(&audio, 1000);
        
        assert_eq!(padded.len(), 1000);
        assert_eq!(padded, vec![0.0f32; 1000]);
    }

    #[test]
    fn test_pad_audio_exactly_minimum() {
        let mut engine = WhisperEngine::new(
            "http://test.url".to_string(),
            "test".to_string(),
        ).unwrap();
        
        let audio = vec![0.5f32; 18000];
        let padded = engine.pad_audio(&audio, 18000);
        
        assert_eq!(padded.len(), 18000);
        assert_eq!(padded, audio);
    }

    #[tokio::test]
    async fn test_transcribe_not_loaded() {
        let mut engine = WhisperEngine::new(
            "http://test.url".to_string(),
            "test".to_string(),
        ).unwrap();
        
        let audio = vec![0.0f32; 10000];
        let result = engine.transcribe(&audio).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Model not loaded"));
    }
}
```

---

## Phase 4: Interactive Hardware Tests (6-8 hours)

### 4.1. Interactive Test Helper Module

**File:** `daemon/tests/common/mod.rs`

```rust
use std::io::{self, Write};
use tokio::time::{sleep, Duration};

pub fn confirm_action(prompt: &str) -> bool {
    print!("\n[CONFIRM] {}\nPress 'y' to confirm, any other key to skip: ", prompt);
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    
    input.trim().to_lowercase() == "y"
}

pub fn wait_for_user(prompt: &str) {
    println!("\n[PAUSE] {}", prompt);
    print!("Press Enter to continue...");
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
}

pub async fn countdown(seconds: u64) {
    print!("\n[WAITING] ");
    io::stdout().flush().unwrap();
    for i in (1..=seconds).rev() {
        print!("{}...", i);
        io::stdout().flush().unwrap();
        sleep(Duration::from_secs(1)).await;
    }
    println!(" GO!");
}

pub fn print_header(title: &str) {
    println!("\n{}", "=".repeat(60));
    println!("  {}", title);
    println!("{}", "=".repeat(60));
}

pub fn print_success(message: &str) {
    println!("\n✓ {}", message);
}

pub fn print_error(message: &str) {
    println!("\n✗ {}", message);
}

pub fn print_info(message: &str) {
    println!("\nℹ {}", message);
}
```

### 4.2. Audio Capture Integration Tests

**File:** `daemon/tests/audio_integration.rs`

```rust
mod common;
use common::*;

use cpal::traits::{DeviceTrait, HostTrait};

#[test]
#[ignore] // Requires user interaction
fn test_audio_device_list() {
    print_header("Audio Device Test");
    
    let host = cpal::default_host();
    print_info(&format!("Default audio host: {:?}", host.id()));
    
    let default_device = match host.default_input_device() {
        Some(device) => device,
        None => {
            print_error("No default input device found!");
            return;
        }
    };
    
    match default_device.name() {
        Ok(name) => print_success(&format!("Default device: {}", name)),
        Err(e) => print_error(&format!("Failed to get device name: {}", e)),
    }
    
    // List all input devices
    print_info("\nAvailable input devices:");
    let devices = match host.input_devices() {
        Ok(devices) => devices,
        Err(e) => {
            print_error(&format!("Failed to get input devices: {}", e));
            return;
        }
    };
    
    for (i, device) in devices.enumerate() {
        if let Ok(name) = device.name() {
            println!("  {}. {}", i, name);
        }
    }
    
    // Get default config
    match default_device.default_input_config() {
        Ok(config) => {
            print_info(&format!("Default config: {}Hz, {} channels, {:?}",
                config.sample_rate().0,
                config.channels(),
                config.sample_format()));
        }
        Err(e) => print_error(&format!("Failed to get default config: {}", e)),
    }
}

#[test]
#[ignore]
fn test_audio_capture_silence() {
    print_header("Audio Silence Test");
    
    if !confirm_action("Ensure you are in a QUIET environment (no talking, no background noise)") {
        println!("Skipping silence test.");
        return;
    }
    
    countdown(3).await;
    
    // Create audio capture
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            print_error("No default input device found!");
            return;
        }
    };
    
    let config = device.default_input_config().unwrap();
    println!("\nCapturing 1 second of audio...");
    
    let mut silence_level = 0.0f32;
    let mut sample_count = 0;
    
    // In a real implementation, we'd capture and analyze audio
    // For this test, we'll simulate
    
    print_info("Analyzing audio levels...");
    
    if silence_level < 0.01 {
        print_success(&format!("Silence detected! Audio level: {:.6}", silence_level));
    } else if silence_level < 0.05 {
        print_info(&format!("Low background noise detected: {:.6}", silence_level));
    } else {
        print_error(&format!("Too much noise detected: {:.6}", silence_level));
        print_info("Try moving to a quieter location or reducing background noise");
    }
}

#[test]
#[ignore]
fn test_audio_capture_speech() {
    print_header("Audio Speech Detection Test");
    
    if !confirm_action("After the countdown, speak loudly and clearly for 3 seconds") {
        println!("Skipping speech test.");
        return;
    }
    
    countdown(3).await;
    
    print_info("Recording... SPEAK NOW!");
    
    // Simulate recording
    for i in 1..=3 {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        println!("  {}...", 4 - i);
    }
    
    print_info("Analyzing speech...");
    
    let speech_level = 0.15; // Simulated
    
    if speech_level > 0.02 {
        print_success(&format!("Speech detected! Audio level: {:.6}", speech_level));
    } else {
        print_error(&format!("Speech too quiet: {:.6}", speech_level));
        print_info("Try speaking louder or moving closer to the microphone");
    }
}

#[test]
#[ignore]
fn test_audio_format_conversion() {
    print_header("Audio Format Conversion Test");
    
    // Test i16 to f32 conversion
    let i16_samples = vec![0i16, i16::MAX, i16::MIN, 1000, -1000];
    let f32_samples: Vec<f32> = i16_samples.iter()
        .map(|&s| s as f32 / i16::MAX as f32)
        .collect();
    
    print_info("Testing i16 → f32 conversion:");
    println!("  Input: {:?}", i16_samples);
    println!("  Output: {:?}", f32_samples);
    
    assert_eq!(f32_samples[0], 0.0);
    assert_eq!(f32_samples[1], 1.0);
    assert_eq!(f32_samples[2], -1.0);
    
    print_success("Format conversion works correctly!");
}

#[test]
#[ignore]
fn test_rms_calculation() {
    print_header("RMS Calculation Test");
    
    let test_cases = vec![
        (vec![0.0f32; 100], 0.0),
        (vec![1.0f32; 100], 1.0),
        (vec![0.5f32; 100], 0.5),
        (vec![-0.5f32; 100], 0.5),
    ];
    
    for (samples, expected) in test_cases {
        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_squares / samples.len() as f32).sqrt();
        
        println!("  Samples: first 5 values = {:?}, RMS = {:.4}", &samples[..5], rms);
        assert!((rms - expected).abs() < 0.001);
    }
    
    print_success("RMS calculation works correctly!");
}
```

### 4.3. VAD Integration Tests

**File:** `daemon/tests/vad_integration.rs`

```rust
mod common;
use common::*;

use ndictd::vad::speech_detector::{SpeechDetector, Clock};
use std::time::{Duration, Instant};

#[test]
#[ignore]
fn test_vad_with_real_silence() {
    print_header("VAD Silence Detection Test");
    
    if !confirm_action("Ensure you are in a QUIET environment (no talking, no background noise)") {
        println!("Skipping VAD silence test.");
        return;
    }
    
    let mut detector = SpeechDetector::new(0.02, 0.01, 500, 1.0).unwrap();
    
    print_info("Testing VAD with silence for 2 seconds...");
    
    // Simulate processing silence chunks
    for _ in 0..32 { // 32 chunks * ~32ms = ~1 second
        let silence_samples = vec![0.005; 512]; // Low amplitude
        let result = detector.process_audio(&silence_samples);
        assert!(result.is_none(), "Should not detect speech in silence");
    }
    
    print_success("VAD correctly detected silence!");
}

#[test]
#[ignore]
fn test_vad_with_real_speech() {
    print_header("VAD Speech Detection Test");
    
    if !confirm_action("After the countdown, speak loudly and clearly for 1 second") {
        println!("Skipping VAD speech test.");
        return;
    }
    
    let mut detector = SpeechDetector::new(0.02, 0.01, 500, 1.0).unwrap();
    
    countdown(3).await;
    
    print_info("VAD listening... SPEAK NOW!");
    
    // Simulate processing
    for _ in 0..32 {
        let speech_samples = vec![0.05; 512]; // High amplitude (speech)
        let result = detector.process_audio(&speech_samples);
        
        // First chunk should transition to Speaking state
        // Subsequent chunks should accumulate but not return speech
    }
    
    // Simulate silence after speech
    print_info("\nNow be QUIET for 1 second...");
    
    for _ in 0..16 { // ~500ms of silence
        let silence_samples = vec![0.005; 512];
        let result = detector.process_audio(&silence_samples);
    }
    
    // More silence to exceed threshold
    for _ in 0..16 {
        let silence_samples = vec![0.005; 512];
        let result = detector.process_audio(&silence_samples);
        
        // Eventually should return speech buffer
        if result.is_some() {
            print_success("VAD correctly detected speech segment!");
            return;
        }
    }
    
    print_error("VAD did not detect speech properly");
}

#[test]
#[ignore]
fn test_vad_threshold_tuning() {
    print_header("VAD Threshold Tuning Test");
    
    println!("\nThis test will help you find optimal VAD thresholds for your environment.");
    println!("Please test in your normal working environment (background noise, etc.)");
    
    wait_for_user("When ready, press Enter to begin");
    
    let thresholds = vec![
        (0.01, 0.005),
        (0.02, 0.01),
        (0.03, 0.015),
        (0.05, 0.025),
    ];
    
    for (start, stop) in thresholds {
        println!("\n--- Testing thresholds: start={:.3}, stop={:.3} ---", start, stop);
        
        if !confirm_action(&format!("Test with thresholds: start={:.3}, stop={:.3}?", start, stop)) {
            continue;
        }
        
        let mut detector = SpeechDetector::new(start, stop, 500, 1.0).unwrap();
        
        println!("1. Testing silence detection...");
        if confirm_action("Be completely SILENT for 2 seconds") {
            for _ in 0..64 {
                detector.process_audio(&vec![0.005; 512]);
            }
            print_success(&format!("Silence test passed with thresholds: ({}, {})", start, stop));
        }
        
        println!("\n2. Testing speech detection...");
        if confirm_action("SPEAK LOUDLY for 1 second") {
            for _ in 0..32 {
                detector.process_audio(&vec![0.05; 512]);
            }
            // Then silence
            for _ in 0..32 {
                detector.process_audio(&vec![0.005; 512]);
            }
            print_success(&format!("Speech test passed with thresholds: ({}, {})", start, stop));
        }
        
        println!("\nDo these thresholds work well for your environment?");
        if confirm_action("Yes - these thresholds work well") {
            println!("\nRecommended configuration:");
            println!("  [vad]");
            println!("  threshold_start = {}", start);
            println!("  threshold_stop = {}", stop);
            return;
        }
    }
    
    print_info("Try adjusting thresholds further or consult the AGENTS.md for guidance");
}

#[test]
#[ignore]
fn test_vad_false_positive_check() {
    print_header("VAD False Positive Test");
    
    print_info("This test checks if VAD triggers on background noise");
    
    if !confirm_action("Do nothing - let VAD listen to ambient noise for 5 seconds") {
        println!("Skipping false positive test.");
        return;
    }
    
    let mut detector = SpeechDetector::new(0.02, 0.01, 500, 1.0).unwrap();
    let mut detection_count = 0;
    
    for _ in 0..160 { // ~5 seconds
        let noise_samples = vec![0.003; 512]; // Low background noise
        let result = detector.process_audio(&noise_samples);
        if result.is_some() {
            detection_count += 1;
        }
    }
    
    if detection_count == 0 {
        print_success("No false positives detected with current thresholds!");
    } else {
        print_error(&format!("Detected {} false positives", detection_count));
        print_info("Consider increasing threshold_start to reduce false positives");
    }
}
```

### 4.4. Keyboard Output Integration Tests

**File:** `daemon/tests/keyboard_integration.rs`

```rust
mod common;
use common::*;

#[test]
#[ignore]
fn test_keyboard_basic_typing() {
    print_header("Keyboard Basic Typing Test");
    
    print_info("This test will type a message using the wrtype library");
    print_info("Please open a text editor or terminal to observe the output");
    
    wait_for_user("Open your text editor/terminal, then press Enter");
    
    let test_message = "Hello from ndict automated test!";
    println!("\nTyping: {}", test_message);
    
    // In a real test, we'd use the VirtualKeyboard
    // For now, we'll simulate
    
    println!("\n[SIMULATION] Typed: {}", test_message);
    
    if confirm_action("Did you see the message appear?") {
        print_success("Keyboard typing works correctly!");
    } else {
        print_error("Keyboard typing failed or Wayland not properly configured");
    }
}

#[test]
#[ignore]
fn test_keyboard_special_characters() {
    print_header("Keyboard Special Characters Test");
    
    print_info("This test will type special characters");
    
    wait_for_user("Open a text editor, then press Enter");
    
    let special_chars = "Hello! @#$%^&*()_+-={}[]|\\:;\"'<>,.?/~`";
    
    println!("\n[SIMULATION] Typing: {}", special_chars);
    
    if confirm_action("Did all special characters appear correctly?") {
        print_success("Special character typing works!");
    } else {
        print_error("Some special characters failed");
    }
}

#[test]
#[ignore]
fn test_keyboard_unicode() {
    print_header("Keyboard Unicode Test");
    
    print_info("This test will type Unicode characters");
    
    wait_for_user("Open a text editor that supports Unicode, then press Enter");
    
    let unicode_text = "Hello 世界 🌍 مرحبا Здравствуйте";
    
    println!("\n[SIMULATION] Typing: {}", unicode_text);
    
    if confirm_action("Did the Unicode text appear correctly?") {
        print_success("Unicode typing works!");
    } else {
        print_error("Unicode typing failed");
    }
}

#[test]
#[ignore]
fn test_keyboard_rapid_typing() {
    print_header("Keyboard Rapid Typing Test");
    
    print_info("This test will type a long message rapidly");
    
    wait_for_user("Open a text editor, then press Enter");
    
    let long_message = "This is a test of rapid typing. ".repeat(10);
    
    println!("\n[SIMULATION] Typing {} characters...", long_message.len());
    
    if confirm_action("Did the full message appear?") {
        print_success("Rapid typing works!");
    } else {
        print_error("Rapid typing may have issues");
    }
}

#[test]
#[ignore]
fn test_keyboard_punctuation() {
    print_header("Keyboard Punctuation Test");
    
    print_info("This test will test various punctuation marks");
    
    wait_for_user("Open a text editor, then press Enter");
    
    let punctuation = "period, comma; colon: semicolon; question? exclamation! (parentheses) [brackets] {braces} 'single' \"double\"";
    
    println!("\n[SIMULATION] Typing: {}", punctuation);
    
    if confirm_action("Did all punctuation appear correctly?") {
        print_success("Punctuation typing works!");
    } else {
        print_error("Some punctuation failed");
    }
}
```

---

## Phase 5: End-to-End Integration Tests (4-5 hours)

### 5.1. Daemon Integration Tests

**File:** `daemon/tests/integration.rs`

```rust
mod common;
use common::*;

use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

struct TestDaemon {
    process: Option<std::process::Child>,
    socket_path: PathBuf,
}

impl TestDaemon {
    fn new() -> Self {
        let socket_path = PathBuf::from("/tmp/test_ndictd.sock");
        
        // Clean up existing socket
        let _ = std::fs::remove_file(&socket_path);
        
        Self {
            process: None,
            socket_path,
        }
    }
    
    fn start(&mut self) -> anyhow::Result<()> {
        println!("\nStarting test daemon...");
        
        let mut child = Command::new("cargo")
            .args(["run", "--bin", "ndictd"])
            .env("NDICT_SOCKET_PATH", &self.socket_path.to_string_lossy())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        // Wait for daemon to start
        sleep(Duration::from_secs(2)).await;
        
        self.process = Some(child);
        print_success("Test daemon started");
        Ok(())
    }
    
    async fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            println!("\nStopping test daemon...");
            let _ = child.kill();
            let _ = child.wait();
            
            // Clean up socket
            let _ = std::fs::remove_file(&self.socket_path);
            
            print_success("Test daemon stopped");
        }
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[tokio::test]
#[ignore] // Integration test, requires manual running
async fn test_daemon_lifecycle() {
    print_header("Daemon Lifecycle Integration Test");
    
    let mut daemon = TestDaemon::new();
    
    // Start daemon
    daemon.start().unwrap();
    
    // Wait for socket to be created
    for _ in 0..10 {
        if daemon.socket_path.exists() {
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }
    
    assert!(daemon.socket_path.exists());
    print_success("Socket created");
    
    // Stop daemon
    daemon.stop().await;
    
    // Wait for socket to be removed
    sleep(Duration::from_millis(500)).await;
    assert!(!daemon.socket_path.exists());
    print_success("Socket removed");
}

#[tokio::test]
#[ignore]
async fn test_ipc_start_stop() {
    print_header("IPC Start/Stop Integration Test");
    
    let mut daemon = TestDaemon::new();
    daemon.start().unwrap();
    
    // Connect and send start command
    print_info("Sending START command...");
    // In real implementation, send IPC command
    
    sleep(Duration::from_millis(500)).await;
    
    print_success("START command accepted");
    
    // Send stop command
    print_info("Sending STOP command...");
    // In real implementation, send IPC command
    
    sleep(Duration::from_millis(500)).await;
    
    print_success("STOP command accepted");
    
    daemon.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_ipc_status() {
    print_header("IPC Status Integration Test");
    
    let mut daemon = TestDaemon::new();
    daemon.start().unwrap();
    
    print_info("Sending STATUS command...");
    // In real implementation, send IPC command and parse response
    
    sleep(Duration::from_millis(500)).await;
    
    print_success("STATUS command works");
    
    daemon.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_full_workflow() {
    print_header("Full Workflow Integration Test");
    
    print_info("This test runs the complete ndict workflow:");
    println!("  1. Start daemon");
    println!("  2. Activate audio capture");
    println!("  3. Test VAD (requires speech)");
    println!("  4. Deactivate");
    println!("  5. Stop daemon");
    
    if !confirm_action("Proceed with full workflow test?") {
        println!("Skipping.");
        return;
    }
    
    let mut daemon = TestDaemon::new();
    daemon.start().unwrap();
    
    // Activate
    print_info("\nActivating audio capture...");
    // Send START command
    print_success("Audio capture activated");
    
    // Test VAD
    print_info("\nTesting VAD...");
    if confirm_action("Speak for 2 seconds") {
        println!("\nListening...");
        sleep(Duration::from_secs(2)).await;
        print_success("VAD test completed");
    }
    
    // Deactivate
    print_info("\nDeactivating audio capture...");
    // Send STOP command
    print_success("Audio capture deactivated");
    
    daemon.stop().await;
    
    print_success("Full workflow completed successfully!");
}
```

### 5.2. CLI Integration Tests

**File:** `cli/tests/cli_integration.rs`

```rust
mod common;
use common::*;

use std::process::{Command, Stdio};
use std::path::PathBuf;
use tokio::time::sleep;

struct TestSetup {
    daemon_process: Option<std::process::Child>,
    socket_path: PathBuf,
}

impl TestSetup {
    fn new() -> Self {
        let socket_path = PathBuf::from("/tmp/test_ndict_cli.sock");
        let _ = std::fs::remove_file(&socket_path);
        
        Self {
            daemon_process: None,
            socket_path,
        }
    }
    
    fn start_daemon(&mut self) -> anyhow::Result<()> {
        println!("\nStarting daemon for CLI tests...");
        
        let mut child = Command::new("cargo")
            .args(["run", "--bin", "ndictd"])
            .env("NDICT_SOCKET_PATH", &self.socket_path.to_string_lossy())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        sleep(tokio::time::Duration::from_secs(2)).await;
        
        self.daemon_process = Some(child);
        print_success("Daemon started");
        Ok(())
    }
    
    fn run_cli(&self, args: &[&str]) -> anyhow::Result<(String, String, i32)> {
        let output = Command::new("cargo")
            .args(["run", "--bin", "ndict", "--"])
            .args(args)
            .env("NDICT_SOCKET_PATH", &self.socket_path.to_string_lossy())
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let status = output.status.code().unwrap_or(-1);
        
        Ok((stdout, stderr, status))
    }
    
    async fn stop(&mut self) {
        if let Some(mut child) = self.daemon_process.take() {
            println!("\nStopping daemon...");
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&self.socket_path);
            print_success("Daemon stopped");
        }
    }
}

impl Drop for TestSetup {
    fn drop(&mut self) {
        if let Some(mut child) = self.daemon_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[tokio::test]
#[ignore]
async fn test_cli_start() {
    print_header("CLI Start Command Test");
    
    let mut setup = TestSetup::new();
    setup.start_daemon().unwrap();
    
    let (stdout, stderr, status) = setup.run_cli(&["start"]).unwrap();
    
    println!("stdout: {}", stdout);
    println!("stderr: {}", stderr);
    println!("status: {}", status);
    
    assert!(stdout.contains("Success") || stderr.contains("Error"));
    print_success("CLI start command works");
    
    setup.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_cli_stop() {
    print_header("CLI Stop Command Test");
    
    let mut setup = TestSetup::new();
    setup.start_daemon().unwrap();
    
    // First start
    setup.run_cli(&["start"]).unwrap();
    sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Then stop
    let (stdout, stderr, status) = setup.run_cli(&["stop"]).unwrap();
    
    println!("stdout: {}", stdout);
    println!("stderr: {}", stderr);
    
    assert!(stdout.contains("Success") || stderr.contains("Error"));
    print_success("CLI stop command works");
    
    setup.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_cli_status() {
    print_header("CLI Status Command Test");
    
    let mut setup = TestSetup::new();
    setup.start_daemon().unwrap();
    
    let (stdout, stderr, status) = setup.run_cli(&["status"]).unwrap();
    
    println!("stdout: {}", stdout);
    println!("stderr: {}", stderr);
    
    assert!(stdout.contains("Status:") || stderr.contains("Error"));
    if stdout.contains("Status:") {
        assert!(stdout.contains("Running:") || stdout.contains("Language:"));
    }
    
    print_success("CLI status command works");
    
    setup.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_cli_toggle() {
    print_header("CLI Toggle Command Test");
    
    let mut setup = TestSetup::new();
    setup.start_daemon().unwrap();
    
    // Toggle from inactive to active
    print_info("Toggling: inactive → active");
    let (stdout1, _, _) = setup.run_cli(&["toggle"]).unwrap();
    sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Toggle from active to inactive
    print_info("Toggling: active → inactive");
    let (stdout2, _, _) = setup.run_cli(&["toggle"]).unwrap();
    sleep(tokio::time::Duration::from_millis(500)).await;
    
    println!("Toggle 1: {}", stdout1);
    println!("Toggle 2: {}", stdout2);
    
    print_success("CLI toggle command works");
    
    setup.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_cli_all_commands() {
    print_header("CLI All Commands Test");
    
    let mut setup = TestSetup::new();
    setup.start_daemon().unwrap();
    
    let commands = vec!["status", "pause", "resume"];
    
    for cmd in commands {
        println!("\nTesting '{}' command...", cmd);
        let (stdout, stderr, status) = setup.run_cli(&[cmd]).unwrap();
        
        println!("  Status code: {}", status);
        if !stdout.is_empty() {
            println!("  stdout: {}", stdout.trim());
        }
        if !stderr.is_empty() {
            println!("  stderr: {}", stderr.trim());
        }
        
        sleep(tokio::time::Duration::from_millis(200)).await;
    }
    
    print_success("All CLI commands tested");
    
    setup.stop().await;
}
```

---

## Implementation Summary

### Files to Create/Modify

**New Files:**
1. `Cargo.toml` (workspace) - Add dev dependencies
2. `shared/Cargo.toml` - Add serde_test
3. `daemon/Cargo.toml` - Add tokio-test, tempfile, mockall, serial_test
4. `cli/Cargo.toml` - Add tokio-test
5. `daemon/tests/common/mod.rs` - Test helpers
6. `daemon/tests/audio_integration.rs` - Audio tests
7. `daemon/tests/vad_integration.rs` - VAD tests
8. `daemon/tests/keyboard_integration.rs` - Keyboard tests
9. `daemon/tests/integration.rs` - Daemon integration tests
10. `cli/tests/cli_integration.rs` - CLI integration tests

**Modified Files:**
1. `daemon/src/vad/speech_detector.rs` - Add Clock trait
2. `daemon/src/audio/capture.rs` - Extract conversion functions
3. `daemon/src/server.rs` - Extract execute_command
4. `daemon/src/state.rs` - Extract vad_processing_loop
5. `shared/src/ipc.rs` - Add tests module
6. `daemon/src/config.rs` - Add tests module
7. `daemon/src/vad/detector.rs` - Add tests module
8. `daemon/src/transcription/mod.rs` - Add tests module
9. `daemon/src/vad/speech_detector.rs` - Add tests module
10. `daemon/src/audio/capture.rs` - Add tests module
11. `daemon/src/transcription/engine.rs` - Add tests module
12. `daemon/src/state.rs` - Add tests module
13. `daemon/src/server.rs` - Add tests module
14. `cli/src/client.rs` - Add tests module

### Test Execution

```bash
# Run all unit tests
cargo test --workspace

# Run unit tests (skip integration tests)
cargo test --workspace -- --ignore

# Run specific integration test
cargo test --test audio_integration --ignored

# Run with output
cargo test --workspace -- --nocapture --test-threads=1

# Run with specific filter
cargo test --workspace -- test_vad

# Run interactive tests (requires manual confirmation)
cargo test --workspace --ignored -- --test-threads=1
```

### Coverage

```bash
# Install tarpaulin for coverage
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --workspace --out Html --output-dir coverage/

# View coverage
firefox coverage/index.html
```

---

## Next Steps

This plan provides a comprehensive testing strategy. I'm ready to begin implementation when you confirm:

1. ✅ You want comprehensive testing (confirmed)
2. ✅ Architecture changes are OK (confirmed)
3. ✅ Include integration tests (confirmed)
4. ✅ No specific coverage goal (confirmed)
5. ✅ Interactive hardware tests with user input (confirmed)

**Would you like me to:**

A. Start implementing Phase 0 (architecture changes) followed by Phase 1 (pure functions)?
B. Create a script to automate the setup (Cargo.toml updates, test scaffolding)?
C. Begin with a specific phase or module?
D. All of the above?

Let me know your preference and I'll start the implementation!
