# ndict Speech-to-Text Daemon

**Generated:** 2026-01-10
**Commit:** N/A
**Branch:** main

## OVERVIEW
Rust workspace implementing speech-to-text daemon (ndictd) with CLI control (ndict). Captures mic audio, detects speech via VAD, transcribes with Whisper, types to active Wayland window.

## STRUCTURE
```
ndict/
├── shared/         # IPC protocol library (Command/Response/StatusInfo)
├── daemon/         # ndictd binary - audio, VAD, Whisper, keyboard
├── cli/            # ndict binary - Unix socket client
├── systemd/        # User-level service file with CAP_SYS_INPUT
└── install.sh      # Installation script with Waybar integration
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Daemon entry | `daemon/src/main.rs` | Initializes tracing, config, socket server |
| IPC protocol | `shared/src/ipc.rs` | Command/Response enums, StatusInfo struct |
| Daemon state | `daemon/src/state.rs` | Manages all components, VAD processing loop |
| Socket server | `daemon/src/server.rs` | Unix socket at `/tmp/ndictd.sock` |
| Audio capture | `daemon/src/audio/capture.rs` | cpal 16kHz mono via broadcast channel |
| VAD logic | `daemon/src/vad/speech_detector.rs` | RMS threshold + state machine |
| Whisper engine | `daemon/src/transcription/engine.rs` | whisper-rs with model download |
| Keyboard output | `daemon/src/output/keyboard.rs` | wrtype for Wayland emulation |
| CLI commands | `cli/src/main.rs` | start/stop/status/toggle with clap |
| Installation | `install.sh` | Copies bins, systemd service, waybar script |
| Config example | `config.example.toml` | Sample configuration with all options documented |

## CONVENTIONS
- Unix socket path: `/tmp/ndictd.sock` (should be `/run/user/$UID/`)
- Config file: `~/.config/ndict/config.toml` (defaults in code)
- State file for Waybar: `/tmp/ndict.state`
- Async: tokio with `Arc<Mutex<T>>` for shared state
- Audio: broadcast channel (capacity 100) for VAD → Whisper pipeline
- Timeout: 30s for Whisper transcription, 5s for keyboard typing
- tracing: structured logging with LevelFilter::INFO

## ANTI-PATTERNS (THIS PROJECT)
- **Orphaned `src/` at root** - Contains stub main.rs not in workspace, should be deleted
- **Duplicate trans/ module** - `daemon/src/trans/` shadows `transcription/`, both exist
- **enigo-examples/ at root** - Should be moved to `examples/` or removed
- **No CI/CD** - Missing `.github/workflows/` or similar
- **Hardcoded `/tmp/` paths** - Should use XDG runtime directory

## UNIQUE STYLES
- **Waybar integration**: install.sh generates `ndict-waybar` script with JSON output for status indicator
- **Gain control**: Audio amplification applied after VAD but before Whisper transcription
- **VAD state machine**: Idle → Speaking → SilenceDetected → Idle (in `speech_detector.rs`)
- **VAD hysteresis**: Separate threshold_start and threshold_stop for recording to prevent rapid toggling
- **Post-processing**: Dedupes consecutive words, removes bracketed content

## COMMANDS
```bash
# Build
cargo build --release

# Run daemon (in foreground for testing)
./target/release/ndictd

# Run CLI (controls daemon)
./target/release/ndict start     # Activate mic capture
./target/release/ndict stop      # Deactivate
./target/release/ndict status    # Check state
./target/release/ndict toggle    # Flip active state

# Configure
# Copy example config to your config directory
mkdir -p ~/.config/ndict
cp config.example.toml ~/.config/ndict/config.toml
# Edit with your preferred values
vim ~/.config/ndict/config.toml

# Install (bins + systemd + waybar)
./install.sh

# View daemon logs
journalctl --user -u ndictd -f
```

## NOTES
- Daemon uses `CAP_SYS_INPUT` capability for keyboard emulation (set in systemd service)
- VAD is simple RMS-based threshold (not Silero), accuracy may vary
- Whisper model downloaded from HuggingFace on first run to `~/.local/share/ndict/models/`
- Pause/Resume and SetLanguage commands are stubbed ("not yet implemented")
- Audio format: 16kHz, 1 channel, chunk_size=512
- **VAD Hysteresis**: `threshold_start` (higher) to begin recording, `threshold_stop` (lower) to end recording
  - This creates a "dead zone" where audio levels between the two thresholds maintain current state
  - Prevents rapid toggling when audio level is near the threshold
  - Typical ratio: threshold_stop should be 50-80% of threshold_start
