# ndictd (daemon/src)

**Generated:** 2026-01-10
**Commit:** N/A
**Branch:** main

## OVERVIEW
Core speech-to-text daemon implementing audio capture, VAD, Whisper transcription, and keyboard output.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Daemon entry | `main.rs` | Initializes tracing, loads config, runs socket server |
| Socket server | `server.rs` | Unix socket at /tmp/ndictd.sock (208 lines) |
| Daemon state | `state.rs` | DaemonState managing all components (195 lines) - audio_capture, speech_detector, whisper_engine, virtual_keyboard, vad_task_handle |
| Config loading | `config.rs` | Config from ~/.config/ndict/config.toml with defaults for audio/VAD/Whisper |
| Audio capture | `audio/capture.rs` | cpal 16kHz mono via broadcast channel |
| VAD detection | `vad/detector.rs` | RMS-based VoiceActivityDetector |
| VAD state machine | `vad/speech_detector.rs` | SpeechDetector with Idle → Speaking → SilenceDetected → Idle |
| Whisper engine | `transcription/engine.rs` | whisper-rs with model download |
| Keyboard output | `output/keyboard.rs` | wrtype for Wayland emulation |
| Post-processing | `transcription/mod.rs` | Dedupes consecutive words, removes bracketed content |

## CONVENTIONS
- Async: tokio with broadcast channels for audio data, Mutex/Arc for state sharing
- Audio: broadcast channel (capacity 100) for VAD → Whisper pipeline
- Timeout: 30s for Whisper transcription, 5s for keyboard typing
- VAD processing runs as background task, spawns Whisper transcription tasks
- tracing: structured logging with LevelFilter::INFO

## ANTI-PATTERNS
- **Stubbed commands**: Pause and Resume commands return Response::Ok but do nothing ("not yet implemented")
- **SetLanguage command**: Stubbed with "not yet implemented" log, does nothing
- **Missing duplicate trans/ module**: `daemon/src/trans/` shadows `transcription/`, both exist (should remove trans/)
