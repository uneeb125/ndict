#!/bin/bash

# ==========================================
# CONFIGURATION
# ==========================================
DAEMON_NAME="ndictd"
CLI_NAME="ndict"
WAYBAR_SCRIPT_NAME="ndict-waybar"
IRONBAR_SCRIPT_NAME="ndict-ironbar"
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
        $HOME/.local/bin/ndict stop
        rm -f "\$STATE_FILE"
    else
        # Currently started, so START it
        $HOME/.local/bin/ndict start
        touch "\$STATE_FILE"
    fi
    # Send signal to waybar to update immediately (Signal 8)
    pkill -RTMIN+8 waybar
    exit 0
fi

# FUNCTION: Manual mode complete (post-processed)
if [ "\$1" == "complete" ]; then
    $HOME/.local/bin/ndict m-complete
    exit 0
fi

# FUNCTION: Manual mode complete raw (no post-processing)
if [ "\$1" == "raw" ]; then
    $HOME/.local/bin/ndict m-complete-raw
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


# This script manages the state file to track if we are recording or not
cat > "$DEST_BIN_DIR/$IRONBAR_SCRIPT_NAME" <<EOF
#!/bin/bash

# --- 1. Configuration (Defined at the top!) ---
STATE_FILE="/tmp/ndict.state"
PIPE_FILE="/tmp/ndict.pipe"

# Tokyo Night Colors
COLOR_REC="#f7768e"  # Red
COLOR_IDLE="#7aa2f7" # Blue

# --- 2. Helper Function: Get Status String ---
get_status_string() {
    if [ -f "$STATE_FILE" ]; then
        echo "<span foreground='$COLOR_REC' weight='bold'> Listening...</span>"
    else
        echo "<span foreground='$COLOR_IDLE' weight='bold'> Idle</span>"
    fi
}

# --- 3. Logic Handler ---
case "$1" in
    toggle)
        # --- TOGGLE MODE (Runs on Click) ---

        # 1. Toggle the state file and run the actual logic
        if [ -f "$STATE_FILE" ]; then
            $HOME/.local/bin/ndict stop
            rm -f "$STATE_FILE"
        else
            $HOME/.local/bin/ndict start
            touch "$STATE_FILE"
        fi

        # 2. Push update to Ironbar via the Pipe
        # We verify the pipe exists to avoid hanging if Ironbar isn't running
        if [ -p "$PIPE_FILE" ]; then
            get_status_string > "$PIPE_FILE" &
        fi
        exit 0
        ;;

    complete)
        $HOME/.local/bin/ndict m-complete
        exit 0
        ;;

    raw)
        $HOME/.local/bin/ndict m-complete-raw
        exit 0
        ;;

    *)
        # --- WATCH MODE (Runs on Ironbar Startup) ---

        # 1. Ensure the Named Pipe exists
        # Clean up old pipe if it's not a pipe file (e.g. stale normal file)
        if [ -e "$PIPE_FILE" ] && [ ! -p "$PIPE_FILE" ]; then
            rm -f "$PIPE_FILE"
        fi

        # Create the pipe if missing
        if [ ! -p "$PIPE_FILE" ]; then
            mkfifo "$PIPE_FILE"
        fi

        # 2. Output the initial state immediately (so bar isn't empty)
        get_status_string

        # 3. Block forever, listening to the pipe
        # "tail -f" keeps the script running. Ironbar reads standard output.
        tail -f "$PIPE_FILE"
        ;;
esac
EOF

chmod +x "$DEST_BIN_DIR/$IRONBAR_SCRIPT_NAME"


# ==========================================
# CONFIG INSTALLATION
# ==========================================
echo ""
echo -e "${YELLOW}======== CONFIGURATION ========${NC}"

CONFIG_DIR="$HOME/.config/ndict"
CONFIG_FILE="$CONFIG_DIR/config.toml"
EXAMPLE_CONFIG="./config.example.toml"

if [ ! -f "$EXAMPLE_CONFIG" ]; then
    echo -e "${RED}Warning: Example config file not found at $EXAMPLE_CONFIG${NC}"
    echo "Skipping config installation."
else
    # Check if config already exists
    if [ -f "$CONFIG_FILE" ]; then
        echo -e "${YELLOW}Config file already exists at: $CONFIG_FILE${NC}"
        echo "Skipping config installation to avoid overwriting your settings."
    else
        # Ask user if they want to install default config
        echo ""
        read -p "Would you like to install the default configuration file? [y/N] " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            mkdir -p "$CONFIG_DIR"
            cp "$EXAMPLE_CONFIG" "$CONFIG_FILE"
            echo -e "${GREEN}Config installed to: $CONFIG_FILE${NC}"
            echo -e "${YELLOW}You can edit this file to customize your settings.${NC}"
        else
            echo "Skipping config installation."
            echo "You can install it later manually:"
            echo "  mkdir -p ~/.config/ndict"
            echo "  cp config.example.toml ~/.config/ndict/config.toml"
        fi
    fi
fi

# ==========================================
# UPDATE SERVICE FILE
# ==========================================
echo "Installing systemd service..."
if [ ! -f "./systemd/ndictd.service" ]; then
    echo -e "${RED}Error: systemd service file not found at ./systemd/ndictd.service${NC}"
    exit 1
fi

cp "./systemd/ndictd.service" "$DEST_SERVICE_DIR/$SERVICE_NAME"

# ==========================================
# RELOAD AND START
# ==========================================
sleep 1
systemctl --user daemon-reload
systemctl --user enable "$SERVICE_NAME"
systemctl --user start "$SERVICE_NAME"

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
    "on-click-right": "$DEST_BIN_DIR/ndict-waybar raw",
    "on-click-middle": "$DEST_BIN_DIR/ndict-waybar complete",
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
