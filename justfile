# ndict - Speech-to-Text Daemon
# Usage: just <recipe>

# === BUILD ===

build:
    cargo build --workspace

release:
    cargo build --workspace --release

clean:
    cargo clean

# === TEST ===

test:
    cargo test --workspace

test-ignored:
    cargo test --workspace --ignored -- --test-threads=1

# === INSTALL ===

install: release
    @echo "Installing ndict..."
    mkdir -p ~/.local/bin
    mkdir -p ~/.config/systemd/user
    mkdir -p ~/.local/share/ndict
    cp target/release/ndictd ~/.local/bin/ndictd
    cp target/release/ndict ~/.local/bin/ndict
    chmod +x ~/.local/bin/ndictd ~/.local/bin/ndict
    @echo "Binaries installed to ~/.local/bin/"

install-waybar: install
    @just _create-waybar-script
    @just _create-ironbar-script
    @echo "Waybar/Ironbar scripts installed to ~/.local/bin/"

install-systemd: install
    @if [ ! -f systemd/ndictd.service ]; then echo "Error: systemd service file not found"; exit 1; fi
    cp systemd/ndictd.service ~/.config/systemd/user/ndict.service
    systemctl --user daemon-reload
    systemctl --user enable ndict.service
    systemctl --user start ndict.service
    @if systemctl --user is-active --quiet ndict.service; then
        echo "Daemon started successfully"
    else
        echo "Daemon failed to start"
        exit 1
    fi

install-all: install-waybar install-systemd
    @just _print-waybar-config
    @just _print-waybar-css

install-config:
    @CONFIG_DIR="$HOME/.config/ndict"
    @CONFIG_FILE="$CONFIG_DIR/config.toml"
    @if [ -f "$CONFIG_FILE" ]; then
        echo "Config already exists at $CONFIG_FILE, skipping"
    else
        mkdir -p "$CONFIG_DIR"
        cp config.example.toml "$CONFIG_FILE"
        echo "Config installed to $CONFIG_FILE"
    fi

# === SERVICE CONTROL ===

start:
    systemctl --user start ndict.service

stop:
    systemctl --user stop ndict.service

restart:
    systemctl --user restart ndict.service

status:
    systemctl --user status ndict.service

logs:
    journalctl --user -u ndict.service -f

# === UNINSTALL ===

uninstall:
    systemctl --user stop ndict.service 2>/dev/null || true
    systemctl --user disable ndict.service 2>/dev/null || true
    rm -f ~/.config/systemd/user/ndict.service
    systemctl --user daemon-reload
    rm -f ~/.local/bin/ndictd ~/.local/bin/ndict
    rm -f ~/.local/bin/ndict-waybar ~/.local/bin/ndict-ironbar
    echo "Uninstalled"

# === INTERNAL RECIPES (prefixed with _) ===

_create-waybar-script:
    @cat > ~/.local/bin/ndict-waybar <<'WAYBAR_EOF'
#!/bin/bash
STATE_FILE="/tmp/ndict.state"
if [ "$1" == "toggle" ]; then
    if [ -f "$STATE_FILE" ]; then
        $HOME/.local/bin/ndict stop
        rm -f "$STATE_FILE"
    else
        $HOME/.local/bin/ndict start
        touch "$STATE_FILE"
    fi
    pkill -RTMIN+8 waybar
    exit 0
fi
if [ -f "$STATE_FILE" ]; then
    echo '{"text": "", "tooltip": "NDict: Listening...", "class": "recording", "alt": "recording"}'
else
    echo '{"text": "", "tooltip": "NDict: Idle", "class": "idle", "alt": "idle"}'
fi
WAYBAR_EOF
    @chmod +x ~/.local/bin/ndict-waybar

_create-ironbar-script:
    @cat > ~/.local/bin/ndict-ironbar <<'IRONBAR_EOF'
#!/bin/bash
STATE_FILE="/tmp/ndict.state"
PIPE_FILE="/tmp/ndict.pipe"
COLOR_REC="#f7768e"
COLOR_IDLE="#7aa2f7"
get_status_string() {
    if [ -f "$STATE_FILE" ]; then
        echo "<span foreground='$COLOR_REC' weight='bold'> Listening...</span>"
    else
        echo "<span foreground='$COLOR_IDLE' weight='bold'> Idle</span>"
    fi
}
case "$1" in
    toggle)
        if [ -f "$STATE_FILE" ]; then
            $HOME/.local/bin/ndict stop
            rm -f "$STATE_FILE"
        else
            $HOME/.local/bin/ndict start
            touch "$STATE_FILE"
        fi
        if [ -p "$PIPE_FILE" ]; then
            get_status_string > "$PIPE_FILE" &
        fi
        exit 0
        ;;
    *)
        if [ -e "$PIPE_FILE" ] && [ ! -p "$PIPE_FILE" ]; then
            rm -f "$PIPE_FILE"
        fi
        if [ ! -p "$PIPE_FILE" ]; then
            mkfifo "$PIPE_FILE"
        fi
        get_status_string
        tail -f "$PIPE_FILE"
        ;;
esac
IRONBAR_EOF
    @chmod +x ~/.local/bin/ndict-ironbar

_print-waybar-config:
    @echo ""
    @echo "=== WAYBAR CONFIG ==="
    @echo 'Add to your Waybar config (modules-left/right):'
    @echo ""
    @echo '"custom/ndict": {'
    @echo '    "format": "{}",'
    @echo '    "return-type": "json",'
    @echo '    "interval": 1,'
    @echo '    "exec": "$HOME/.local/bin/ndict-waybar",'
    @echo '    "on-click": "$HOME/.local/bin/ndict-waybar toggle",'
    @echo '    "signal": 8'
    @echo '}'

_print-waybar-css:
    @echo ""
    @echo "=== WAYBAR CSS ==="
    @echo "Add to your Waybar style.css:"
    @echo ""
    @echo "#custom-ndict {"
    @echo "    padding: 0 10px;"
    @echo "    font-weight: bold;"
    @echo "}"
    @echo "#custom-ndict.idle { color: #89b4fa; }"
    @echo "#custom-ndict.recording {"
    @echo "    color: #f38ba8;"
    @echo "    animation-name: blink;"
    @echo "    animation-duration: 2s;"
    @echo "    animation-timing-function: linear;"
    @echo "    animation-iteration-count: infinite;"
    @echo "}"
    @echo "@keyframes blink {"
    @echo "    0% { opacity: 1.0; }"
    @echo "    50% { opacity: 0.5; }"
    @echo "    100% { opacity: 1.0; }"
    @echo "}"
