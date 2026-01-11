#!/bin/bash

# ==========================================
# CONFIGURATION
# ==========================================
DAEMON_NAME="ndictd"
CLI_NAME="ndict"
WAYBAR_SCRIPT_NAME="ndict-waybar"
SERVICE_NAME="ndict.service"

# Source Directory (Rust release folder)
SOURCE_DIR="./target/release"

# Destination paths
DEST_BIN_DIR="$HOME/.local/bin"
DEST_SERVICE_DIR="$HOME/.config/systemd/user"
DATA_DIR="$HOME/.local/share/ndict"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# ==========================================
# PRE-CHECKS
# ==========================================
if [ ! -f "$SOURCE_DIR/$DAEMON_NAME" ] || [ ! -f "$SOURCE_DIR/$CLI_NAME" ]; then
    echo -e "${RED}Error: Binaries not found in $SOURCE_DIR${NC}"
    echo "Make sure you have compiled the project (cargo build --release)."
    exit 1
fi

echo -e "${GREEN}Installing $DAEMON_NAME, $CLI_NAME, and Waybar integration...${NC}"

# ==========================================
# STOP EXISTING SERVICE
# ==========================================
if systemctl --user is-active --quiet "$SERVICE_NAME"; then
    echo -e "${YELLOW}Stopping running daemon...${NC}"
    systemctl --user stop "$SERVICE_NAME"
fi

# ==========================================
# INSTALL BINARIES
# ==========================================
mkdir -p "$DEST_BIN_DIR"
mkdir -p "$DEST_SERVICE_DIR"
mkdir -p "$DATA_DIR"

echo "Installing binaries..."
cp "$SOURCE_DIR/$DAEMON_NAME" "$DEST_BIN_DIR/$DAEMON_NAME"
cp "$SOURCE_DIR/$CLI_NAME" "$DEST_BIN_DIR/$CLI_NAME"
chmod +x "$DEST_BIN_DIR/$DAEMON_NAME"
chmod +x "$DEST_BIN_DIR/$CLI_NAME"

# ==========================================
# CREATE WAYBAR WRAPPER SCRIPT
# ==========================================
echo "Creating Waybar wrapper script ($WAYBAR_SCRIPT_NAME)..."

# This script manages the state file to track if we are recording or not
cat > "$DEST_BIN_DIR/$WAYBAR_SCRIPT_NAME" <<EOF
#!/bin/bash

STATE_FILE="/tmp/ndict.state"

# FUNCTION: Toggle State
if [ "\$1" == "toggle" ]; then
    if [ -f "\$STATE_FILE" ]; then
        # Currently running, so STOP it
        ndict stop
        rm -f "\$STATE_FILE"
    else
        # Currently stopped, so START it
        ndict start
        touch "\$STATE_FILE"
    fi
    # Send signal to waybar to update immediately (Signal 8)
    pkill -RTMIN+8 waybar
    exit 0
fi

# FUNCTION: Check Status (Default)
if [ -f "\$STATE_FILE" ]; then
    # Running (Red)
    echo '{"text": "", "tooltip": "NDict: Listening...", "class": "recording", "alt": "recording"}'
else
    # Stopped (Blue)
    echo '{"text": "", "tooltip": "NDict: Idle", "class": "idle", "alt": "idle"}'
fi
EOF

chmod +x "$DEST_BIN_DIR/$WAYBAR_SCRIPT_NAME"

# ==========================================
# UPDATE SERVICE FILE
# ==========================================
echo "Updating systemd service..."
cat > "$DEST_SERVICE_DIR/$SERVICE_NAME" <<EOF
[Unit]
Description=ndict - Speech to Text Daemon
After=graphical-session.target network.target
PartOf=graphical-session.target

[Service]
Type=simple
Environment="PATH=%h/.local/bin:/usr/local/bin:/usr/bin:/bin"
ExecStartPre=/usr/bin/mkdir -p %h/.local/share/ndict
# Ensure lock file is gone on fresh start
ExecStartPre=/bin/rm -f /tmp/ndict.state
ExecStart=%h/.local/bin/$DAEMON_NAME
ExecStopPost=/bin/rm -f /tmp/ndict.state
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

# ==========================================
# RELOAD AND START
# ==========================================
systemctl --user daemon-reload
systemctl --user enable "$SERVICE_NAME"
systemctl --user restart "$SERVICE_NAME"

sleep 1
if systemctl --user is-active --quiet "$SERVICE_NAME"; then
    echo -e "${GREEN}Success! Installed binaries and Waybar script.${NC}"
    echo "Script location: $DEST_BIN_DIR/$WAYBAR_SCRIPT_NAME"
else
    echo -e "${RED}Daemon failed to start.${NC}"
fi

# ==========================================
# PRETTY PRINT INSTRUCTIONS
# ==========================================

# Function to handle printing (uses bat if available, cat if not)
print_code() {
    local lang=$1
    if command -v bat &> /dev/null; then
        # --paging=never prevents it from opening 'less' and blocking the script
        bat -l "$lang" --style=plain --paging=never
    else
        cat
    fi
}


echo -e "\n${YELLOW}======== WAYBAR CONFIGURATION ========${NC}"

echo ""


echo "Add this to your Waybar 'config' (modules-left/right):"

print_code json <<EOF
"custom/ndict": {
    "format": "{}",
    "return-type": "json",
    "interval": 1,
    "exec": "$DEST_BIN_DIR/ndict-waybar",
    "on-click": "$DEST_BIN_DIR/ndict-waybar toggle",
    "signal": 8
}
EOF


echo ""

echo -e "${RED}Don't forget to actually enable the module${NC}"


echo -e "\n${YELLOW}=== WAYBAR CSS ===${NC}"
echo "Add this to your Waybar 'style.css' and adjust to taste:"

print_code css <<EOF
#custom-ndict {
    padding: 0 10px;
    font-weight: bold;
}

/* Stopped / Idle (Blue) */
#custom-ndict.idle {
    color: #89b4fa; 
}

/* Recording (Red + Blink) */
#custom-ndict.recording {
    color: #f38ba8; 
    animation-name: blink;
    animation-duration: 2s;
    animation-timing-function: linear;
    animation-iteration-count: infinite;
}

@keyframes blink {
    0% { opacity: 1.0; }
    50% { opacity: 0.5; }
    100% { opacity: 1.0; }
}
EOF

# echo -e "\n${GREEN}Installation and Instruction generation complete!${NC}"
