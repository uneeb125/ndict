# Shared IPC Library

## OVERVIEW
Core contract defining the binary protocol between ndict CLI and ndictd daemon via Unix domain sockets.

## WHERE TO LOOK

**Protocol Definition:**
- `ipc.rs` - All IPC types: Command, Response, StatusInfo, IpcError

**Key Types:**

### Command enum
Client → Daemon commands:
- `Start` - Activate audio capture and transcription
- `Stop` - Deactivate and cleanup
- `Pause` - Temporarily suspend transcription
- `Resume` - Resume from paused state
- `Status` - Query current daemon state
- `SetLanguage(String)` - Set transcription language code
- `Toggle` - Quick start/stop toggle

### Response enum
Daemon → Client responses:
- `Ok` - Command succeeded
- `Error(String)` - Command failed with message
- `Status(StatusInfo)` - Current daemon state

### StatusInfo struct
```rust
pub struct StatusInfo {
    pub is_running: bool,   // Daemon process alive
    pub is_active: bool,    // Audio capture active
    pub language: String,   // Current language code
}
```

### IpcError
IPC communication errors:
- `Io` - Socket/stream I/O failures
- `Serialization` - JSON encode/decode failures
- `ConnectionRefused` - Daemon not running
- `Timeout` - Operation timed out

## ANTI-PATTERNS

❌ **Breaking protocol changes** - Command/Response enums MUST remain backwards compatible
❌ **Adding new commands without versioning** - New commands should gracefully handle older daemons
❌ **Blocking serialization** - Never use blocking I/O in hot path
❌ **Ignoring ConnectionRefused** - Always check if daemon is running before commands
❌ **Silent errors** - IpcError should propagate to CLI with user-friendly messages

## CRITICAL NOTES

- This is the ONLY shared library between ndict (CLI) and ndictd (daemon)
- Uses serde for JSON serialization over Unix domain sockets
- Protocol changes require updating BOTH binaries simultaneously
