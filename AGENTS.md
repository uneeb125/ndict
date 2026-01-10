# ndict Speech-to-Text Implementation Plan

## 🏗️ Architecture Overview

**Components:**
- **ndictd**: Daemon server that runs as a background service
- **ndict**: CLI client that controls the daemon
- **shared**: Shared library with IPC protocol

```
┌─────────────┐         Unix Socket          ┌──────────────────────────┐
│   ndict     │◄──────────────────────────────►│   ndictd (daemon)       │
│  (CLI)      │     Commands/Status          │  ┌────────────────────┐  │
└─────────────┘                               │  │  Socket Server    │  │
                                              │  └────────────────────┘  │
                                              │           │              │
                                              │           ▼              │
                                              │  ┌────────────────────┐  │
                                              │  │  State Manager    │  │
                                              │  └────────────────────┘  │
                                              │           │              │
                                              │           ▼              │
                                              │  ┌────────────────────┐  │
                                              │  │  Audio Capture    │  │
                                              │  │  (cpal)          │  │
                                              │  └────────────────────┘  │
                                              │           │              │
                                              │           ▼              │
                                              │  ┌────────────────────┐  │
                                              │  │  VAD Engine       │  │
                                              │  │  (Silero VAD)     │  │
                                              │  └────────────────────┘  │
                                              │           │              │
                                              │           ▼              │
                                              │  ┌────────────────────┐  │
                                              │  │  Whisper Model    │  │
                                              │  │  (whisper-rs)     │  │
                                              │  └────────────────────┘  │
                                              │           │              │
                                              │           ▼              │
                                              │  ┌────────────────────┐  │
                                              │  │  Keyboard Output   │  │
                                              │  │  (uinput)         │  │
                                              │  └────────────────────┘  │
                                              └──────────────────────────┘
```

## 🔧 Technology Stack

| Component | Library | Reason |
|-----------|---------|---------|
| Audio Capture | `cpal` | Pure Rust, cross-platform, low-latency audio I/O |
| Whisper | `whisper-rs` + `whisper.cpp` | Optimized C++ backend, CPU-optimized |
| VAD | `silero-vad-rs` | Fast, accurate voice activity detection |
| Async Runtime | `tokio` | Industry standard, excellent ecosystem |
| IPC | Unix Domain Sockets | Built-in, low overhead, file-based permissions |
| Keyboard | `mouse-keyboard-input` | Cross-platform, works with Wayland via uinput |
| CLI | `clap` | Modern, derive-based API |
| Config | `serde` + `toml` | Type-safe, human-readable |
| Logging | `tracing` | Structured logging, async-friendly |

## 📋 Implementation Stages

### ✅ Stage 0: Foundation (Day 1)
**Status:** Complete

**Completed:**
- [x] Cargo workspace setup
- [x] Basic `ndictd` and `ndict` binaries
- [x] Both binaries compile and run

**Success Criteria:** ✅ Both binaries compile without errors

---

### ✅ Stage 1: IPC Communication (Days 2-3)
**Status:** Complete

**Completed:**
- [x] Shared IPC protocol (`Command`, `Response`, `StatusInfo` enums)
- [x] Unix domain socket server in daemon
- [x] Unix domain socket client in CLI
- [x] All commands (start, stop, pause, resume, status) working

**Success Criteria:** ✅ CLI can send commands and receive responses from daemon

---

### ✅ Stage 2: Audio Capture (Days 4-5)
**Status:** Complete

**Completed:**
- [x] cpal dependency added
- [x] AudioCapture module created (mock implementation for now)
- [x] DaemonState manages activation state
- [x] Audio capture can be started/stopped via CLI
- [x] Status correctly reports active state

**Note:** Full audio capture with cpal streams will be implemented in later stage when VAD is ready.

**Success Criteria:** ✅ Daemon tracks active state correctly

---

### 🎯 Stage 3: Voice Activity Detection (Days 6-7)
**Status:** In Progress

**To Do:**
- [ ] Add `silero-vad-rs` dependency
- [ ] Download/set up Silero VAD model
- [ ] Implement VoiceActivityDetector struct
- [ ] Create SpeechDetector with state machine
- [ ] States: Idle → Speaking → SilenceDetected → Idle
- [ ] Configurable thresholds (0.5, 500ms silence duration)
- [ ] Test VAD with audio input
- [ ] Verify speech detection accuracy

**Transcription Flow:**
```
User: ndict start
  ↓
Daemon: [ACTIVATED]
  ↓
Mic: Opens and starts capturing
  ↓
VAD: Monitoring audio levels...
  ↓
User: "Hello world" (speaking)
  ↓
VAD: ⬆️ Speech detected (audio level rises)
  ↓
Whisper: Starts processing audio chunks
  ↓
User: [stops speaking, silence]
  ↓
VAD: ⬇️ Silence detected (below threshold for 500ms)
  ↓
Whisper: Finalizes transcription
  ↓
Keyboard: Types "Hello world" instantly
  ↓
Loop: VAD continues monitoring for next speech
  ↓
User: ndict stop
  ↓
Daemon: [DEACTIVATED], mic closes
```

**Success Criteria:**
- ✅ VAD accurately detects speech/silence boundaries
- ✅ Triggers speech completion events at appropriate times
- ✅ Configurable thresholds work correctly

---

### 🧠 Stage 4: Whisper Transcription (Days 8-9)
**Status:** Pending

**To Do:**
- [ ] Add `whisper-rs` dependency
- [ ] Download Whisper base model
- [ ] Create WhisperEngine wrapper
- [ ] Integrate with SpeechDetector
- [ ] Handle model loading/lazy-loading
- [ ] Transcribe speech segments
- [ ] Handle language detection/selection
- [ ] Post-processing (filter duplicates, handle punctuation)
- [ ] Test transcription accuracy

**Success Criteria:** ✅ Whisper accurately transcribes speech segments to text

---

### ⌨️ Stage 5: Keyboard Emulation (Days 10-11)
**Status:** Pending

**To Do:**
- [ ] Add `mouse-keyboard-input` dependency
- [ ] Create VirtualKeyboard struct
- [ ] Implement character-to-keycode mapping
- [ ] Handle special characters, numbers, symbols
- [ ] Handle modifier keys (Shift, Ctrl, Alt)
- [ ] Type text all at once (instant, not character-by-character)
- [ ] Integrate with transcription pipeline
- [ ] Test with Wayland applications

**Success Criteria:** ✅ Text appears in active Wayland window

---

### 🚀 Stage 6: Systemd Integration (Days 12-13)
**Status:** Pending

**To Do:**
- [ ] Create user-level systemd service file
- [ ] Set `AmbientCapabilities=CAP_SYS_INPUT` for keyboard emulation
- [ ] Create installation script
- [ ] Handle service lifecycle (enable, start, stop, restart)
- [ ] Configure logging (journalctl)
- [ ] Add resource limits
- [ ] Test daemon lifecycle with systemd
- [ ] Test auto-restart on crashes

**Service File Location:** `~/.config/systemd/user/ndictd.service`

**Success Criteria:** ✅ Daemon runs as user-level systemd service

---

### 🎨 Stage 7: Optimization & Documentation (Day 14)
**Status:** Pending

**To Do:**
- [ ] Lazy-load Whisper model (only when first needed)
- [ ] Optimize audio buffer sizes
- [ ] Profile and reduce CPU usage
- [ ] Test idle CPU usage (<1%)
- [ ] Test active CPU usage (5-15%)
- [ ] Measure latency (speech end to text appearing)
- [ ] Monitor memory usage
- [ ] Write comprehensive README
- [ ] Document installation process
- [ ] Create user guide with examples
- [ ] Troubleshooting guide
- [ ] Configuration documentation

**Performance Targets:**
| Metric | Target |
|--------|---------|
| Idle CPU | < 1% |
| Active CPU | 5-15% |
| Latency | < 3s |
| Memory | < 1GB |

**Success Criteria:** ✅ All documentation complete, performance targets met

---

## 📊 Testing Checklist

### Stage 0
- [ ] `cargo build --release` succeeds
- [ ] `./target/release/ndictd` runs
- [ ] `./target/release/ndict --help` works

### Stage 1
- [ ] `ndict start` succeeds
- [ ] `ndict stop` succeeds
- [ ] `ndict status` shows correct state
- [ ] `ndict pause` succeeds
- [ ] `ndict resume` succeeds

### Stage 2
- [ ] `ndict start` shows "Activated audio capture"
- [ ] Status shows `Active: true`
- [ ] `ndict stop` shows "Audio capture stopped"
- [ ] Status shows `Active: false`

### Stage 3
- [ ] VAD detects speech when speaking
- [ ] VAD detects silence when not speaking
- [ ] Speech completion events trigger after 500ms silence
- [ ] VAD logs: "Speech detected", "Silence detected", "Speech complete"

### Stage 4
- [ ] Speech is transcribed to text
- [ ] Accuracy > 95% (manual testing)
- [ ] Transcription appears in logs
- [ ] No partial/cut-off words

### Stage 5
- [ ] Text appears in active Wayland window
- [ ] All characters type correctly
- [ ] Special characters work (space, punctuation)
- [ ] Capitalization works (Shift key)
- [ ] No errors in logs

### Stage 6
- [ ] Service starts with `systemctl --user start ndictd`
- [ ] Service runs as user (not root)
- [ ] `ndict` commands work
- [ ] Service survives reboots (if enabled)
- [ ] `journalctl --user -u ndictd -f` shows logs

### Stage 7
- [ ] Idle CPU < 1% (measured with htop)
- [ ] Active CPU 5-15% (measured with htop)
- [ ] Latency < 3s (from silence to text)
- [ ] Memory < 1GB (measured with `ps aux`)
- [ ] README is comprehensive
- [ ] User guide has examples
- [ ] Troubleshooting guide covers common issues

---

## 🎯 Final Deliverable

A fully functional speech-to-text daemon that:
1. ✅ Runs as a background systemd service
2. ✅ Listens to microphone only when activated
3. ✅ Detects speech/silence boundaries
4. ✅ Transcribes using local Whisper (base model)
5. ✅ Types text into active Wayland window
6. ✅ Controlled via simple CLI (`ndict start/stop/status`)
7. ✅ Consumes minimal CPU when idle (<1%)
8. ✅ Has comprehensive documentation

---

## 📝 Configuration File

`~/.config/ndict/config.toml`:

```toml
[audio]
device = "default"
sample_rate = 16000
chunk_size = 512

[vad]
threshold = 0.5
min_speech_duration_ms = 250
min_silence_duration_ms = 500

[whisper]
model = "base"
model_path = "~/.local/share/ndict/models/base.bin"
language = "auto"

[output]
typing_mode = "instant"
```

---

## 🔒 Security & Permissions

**Required Permissions:**
- `CAP_SYS_INPUT` - For keyboard emulation (uinput)
- Access to audio device - For microphone capture
- Unix socket permissions - For IPC

**Security Considerations:**
- Unix domain socket uses filesystem permissions
- No network exposure (local only)
- Model files stored in user's home directory
- Service runs as user-level (not root)

---

## 🚨 Known Challenges & Solutions

### 1. Wayland Keyboard Emulation
**Challenge:** Wayland doesn't allow arbitrary X11-style keyboard injection
**Solution:** Use Linux's `uinput` kernel module with `CAP_SYS_INPUT` capability

### 2. Model Size vs. Latency
**Challenge:** Larger models more accurate but slower
**Solution:** Default to `base` model, allow configuration

### 3. Continuous Audio Power
**Challenge:** Keeping mic active consumes power
**Solution:** Mic only activates on `ndict start`, VAD pauses when idle

### 4. Audio Device Conflicts
**Challenge:** Other apps may use the microphone
**Solution:** Handle unavailability gracefully, provide error messages

### 5. Text Accuracy & Context
**Challenge:** Whisper may misinterpret without context
**Solutions:**
- Increase chunk overlap
- Auto-detect language
- Allow custom vocabulary/prompts
- Confidence threshold filtering

---

## 📈 Future Enhancements (Post-MVP)

1. **Multi-language support** - Seamless language switching
2. **Punctuation/capitalization** - Better formatting
3. **Custom vocabulary** - Add domain-specific words
4. **Hotkey toggling** - Global hotkey to activate
5. **GUI status indicator** - System tray icon
6. **Translation** - Auto-translate to target language
7. **Command mode** - Voice commands to control system
8. **Audio file input** - Transcribe pre-recorded files
9. **Continuous streaming** - Type text as it's being transcribed
10. **Profanity filter** - Optional content filtering

---

## 🗓️ Timeline

| Stage | Days | Status |
|--------|-------|--------|
| 0: Foundation | 1 | ✅ Complete |
| 1: IPC | 2 | ✅ Complete |
| 2: Audio Capture | 2 | ✅ Complete |
| 3: VAD | 2 | 🔄 In Progress |
| 4: Whisper | 2 | ⏳ Pending |
| 5: Keyboard | 2 | ⏳ Pending |
| 6: Systemd | 2 | ⏳ Pending |
| 7: Optimization | 1 | ⏳ Pending |

**Total:** 14 days

---

## 🤝 Contributing

This plan is a living document. As we implement and test, we may discover new challenges or optimizations that require adjustments.

**Testing Philosophy:**
- Test each stage before moving on
- Manual testing for audio/VAD/transcription
- Automated testing for IPC/state management
- Performance testing at each stage

**Code Quality:**
- Follow Rust best practices
- Use `anyhow` for error handling
- Use `tracing` for structured logging
- Document public APIs
- Handle edge cases gracefully

---

## 📞 Support

For issues or questions:
1. Check `journalctl --user -u ndictd -f` for logs
2. Verify microphone: `pactl list sources short`
3. Check audio permissions
4. Verify CAP_SYS_INPUT is set: `systemctl --user show ndictd | grep AmbientCapabilities`
