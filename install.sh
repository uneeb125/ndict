#!/bin/bash

# ==========================================
# CONFIGURATION
# ==========================================
DAEMON_NAME="ndictd"
CLI_NAME="ndict"
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

# Check if binaries exist
if [ ! -f "$SOURCE_DIR/$DAEMON_NAME" ] || [ ! -f "$SOURCE_DIR/$CLI_NAME" ]; then
    echo -e "${RED}Error: Binaries not found in $SOURCE_DIR${NC}"
    echo "Make sure you have compiled the project (cargo build --release)."
    echo "Looking for: $DAEMON_NAME and $CLI_NAME"
    exit 1
fi

echo -e "${GREEN}Installing $DAEMON_NAME and $CLI_NAME...${NC}"

# ==========================================
# STOP EXISTING SERVICE
# ==========================================
# Stop daemon before overwriting binary
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

# 1. Install Daemon
echo "Installing Daemon ($DAEMON_NAME)..."
cp "$SOURCE_DIR/$DAEMON_NAME" "$DEST_BIN_DIR/$DAEMON_NAME"
chmod +x "$DEST_BIN_DIR/$DAEMON_NAME"

# 2. Install CLI
echo "Installing CLI ($CLI_NAME)..."
cp "$SOURCE_DIR/$CLI_NAME" "$DEST_BIN_DIR/$CLI_NAME"
chmod +x "$DEST_BIN_DIR/$CLI_NAME"

# ==========================================
# UPDATE SERVICE FILE (Daemon Only)
# ==========================================

echo "Updating systemd service..."
cat > "$DEST_SERVICE_DIR/$SERVICE_NAME" <<EOF
[Unit]
Description=ndict - Speech to Text Daemon
# Start after graphical session and network
After=graphical-session.target network.target
PartOf=graphical-session.target

[Service]
Type=simple
# Ensure PATH includes .local/bin
Environment="PATH=%h/.local/bin:/usr/local/bin:/usr/bin:/bin"

# Create data dir if missing
ExecStartPre=/usr/bin/mkdir -p %h/.local/share/ndict

# Run the DAEMON binary
ExecStart=%h/.local/bin/$DAEMON_NAME

# Restart logic
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

# ==========================================
# RELOAD AND START
# ==========================================

echo "Reloading systemd..."
systemctl --user daemon-reload

echo "Restarting daemon..."
systemctl --user enable "$SERVICE_NAME"
systemctl --user restart "$SERVICE_NAME"

# ==========================================
# STATUS CHECK
# ==========================================

sleep 1
if systemctl --user is-active --quiet "$SERVICE_NAME"; then
    echo -e "${GREEN}Success! Installation complete.${NC}"
    echo "Daemon is running in the background."
    echo "You can now use the CLI command: ${GREEN}$CLI_NAME <args>${NC}"
else
    echo -e "${RED}Daemon failed to start.${NC}"
    systemctl --user status "$SERVICE_NAME" --no-pager
fi
