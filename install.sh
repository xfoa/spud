#!/bin/bash
set -e

echo "[install] Building spud in release mode..."
cargo build --release

echo "[install] Installing binary to /usr/local/bin/spud"
sudo install -Dm755 target/release/spud /usr/local/bin/spud

echo "[install] Installing polkit rules for auth caching"
sudo install -Dm644 resources/50-spud-injection.pkla /etc/polkit-1/localauthority/50-local.d/50-spud-injection.pkla

echo "[install] Installing desktop entry"
sudo install -Dm644 resources/spud.desktop /usr/share/applications/spud.desktop

echo "[install] Done. You can now run: spud"
echo "[install] Note: log out and back in for desktop entry changes to take effect."
