#!/bin/bash
set -e

INSTALL_DIR="$HOME/.local"
SERVICE_DIR="$HOME/.config/systemd/user"
BIN_DIR="$INSTALL_DIR/bin"

echo "Installing ndict..."

mkdir -p "$INSTALL_DIR/share/ndict"
mkdir -p "$BIN_DIR"
mkdir -p "$SERVICE_DIR"

cargo build --release

cp target/release/ndictd "$BIN_DIR/"
cp target/release/ndict "$BIN_DIR/"

chmod +x "$BIN_DIR/ndictd"
chmod +x "$BIN_DIR/ndict"

cp systemd/ndictd.service "$SERVICE_DIR/"

systemctl --user daemon-reload
systemctl --user enable ndictd

echo "Installation complete!"
echo ""
echo "To start the daemon:"
echo "  systemctl --user start ndictd"
echo ""
echo "To check status:"
echo "  systemctl --user status ndictd"
echo ""
echo "To view logs:"
echo "  journalctl --user -u ndictd -f"
