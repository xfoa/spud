#!/usr/bin/env bash
# Installs the icon and desktop file for the current user so the window icon
# appears correctly on Wayland (GNOME and other compositors that use .desktop files).
set -e

ICON_DIR="$HOME/.local/share/icons/hicolor/256x256/apps"
APPS_DIR="$HOME/.local/share/applications"

mkdir -p "$ICON_DIR" "$APPS_DIR"
cp resources/icon.png "$ICON_DIR/spud.png"
cp resources/spud.desktop "$APPS_DIR/spud.desktop"
sudo ln -s `pwd`/target/debug/spud -t /usr/local/bin

gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
update-desktop-database "$APPS_DIR" 2>/dev/null || true

echo "Installed icon and desktop file."
echo "You may need to log out and back in, or run: killall -HUP gnome-shell"
