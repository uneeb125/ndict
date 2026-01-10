#!/bin/bash

# ==========================================
# CONFIGURATION
# ==========================================
BINARY_NAME="ndictd"
SERVICE_NAME="ndict.service"

# Source Path (Rust release folder)
SOURCE_BIN="./target/release/$BINARY_NAME"

# Destination paths (Standard Linux User Paths)
DEST_BIN_DIR="$HOME/.local/bin"
DEST_SERVICE_DIR="$HOME/.config/systemd/user"
DATA_DIR="$HOME/.local/share/ndict"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# ==========================================
# PRE-CHECKS
# ==========================================

# 1. Check if source binary exists
if [ ! -f "$SOURCE_BIN" ]; then
    echo -e "${RED}Error: Binary not found at '$SOURCE_BIN'${NC}"
    echo "Did you run 'cargo build --release'?"
    exit 1
fi

echo -e "${GREEN}Installing/Updating $BINARY_NAME from release folder...${NC}"

# ==========================================
# STOP EXISTING SERVICE
# ==========================================
# Stop to avoid "Text file busy" errors during copy
if systemctl --user is-active --quiet "$SERVICE_NAME"; then
    echo -e "${YELLOW}Stopping running service...${NC}"
    systemctl --user stop "$SERVICE_NAME"
fi

# ==========================================
# INSTALL FILES
# ==========================================

# 1. Create necessary directories
mkdir -p "$DEST_BIN_DIR"
mkdir -p "$DEST_SERVICE_DIR"
mkdir -p "$DATA_DIR"

# 2. Copy the binary (overwrites existing)
echo "Copying binary to $DEST_BIN_DIR..."
cp "$SOURCE_BIN" "$DEST_BIN_DIR/$BINARY_NAME"
chmod +x "$DEST_BIN_DIR/$BINARY_NAME"

# 3. Write the Systemd Service File
# Using %h makes this portable (auto-expands to Home dir)
echo "Updating service file..."
cat > "$DEST_SERVICE_DIR/$SERVICE_NAME" <<EOF
[Unit]
Description=ndict - Speech to Text Daemon
# Start after graphical session and network are ready
After=graphical-session.target network.target
PartOf=graphical-session.target

[Service]
Type=simple
# Ensure PATH includes .local/bin so the app finds itself or other tools
Environment="PATH=%h/.local/bin:/usr/local/bin:/usr/bin:/bin"

# Create data dir if missing
ExecStartPre=/usr/bin/mkdir -p %h/.local/share/ndict

# Run the binary
ExecStart=%h/.local/bin/$BINARY_NAME

# Restart logic
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

# ==========================================
# RELOAD AND START
# ==========================================

echo "Reloading systemd daemon..."
systemctl --user daemon-reload

echo "Enabling and Starting service..."
systemctl --user enable "$SERVICE_NAME"
systemctl --user restart "$SERVICE_NAME"

# ==========================================
# VALIDATION AND PERMISSIONS
# ==========================================

# Check if user is in the 'input' group (often needed for mic/hotkeys)
if ! groups | grep -q "\binput\b"; then
    echo -e "${YELLOW}------------------------------------------------------------${NC}"
    echo -e "${YELLOW}WARNING: Potential Permission Issue${NC}"
    echo -e "Your user is not in the '${YELLOW}input${NC}' group."
    echo -e "If the app crashes or can't read audio/keys, run:"
    echo -e "${GREEN}sudo usermod -aG input \$USER${NC}"
    echo -e "Then REBOOT your computer."
    echo -e "${YELLOW}------------------------------------------------------------${NC}"
fi

# Final Status Check
sleep 1
if systemctl --user is-active --quiet "$SERVICE_NAME"; then
    echo -e "${GREEN}Success! Service is active and running.${NC}"
    echo "Logs: journalctl --user -xeu $SERVICE_NAME -f"
else
    echo -e "${RED}Service failed to start.${NC}"
    systemctl --user status "$SERVICE_NAME" --no-pager
fi
