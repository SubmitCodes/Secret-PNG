#!/usr/bin/env bash
set -e

echo "=== Installing Stow on Linux ==="

# Build release binaries if not already built
if [ ! -f "target/release/stow_gui" ]; then
    echo "Compiling release binaries..."
    ./build_linux.sh
fi

echo "Installing binaries to /usr/local/bin..."
sudo install -m 755 target/release/stow_gui /usr/local/bin/stow-gui
sudo install -m 755 target/release/stow_cli /usr/local/bin/stow-cli

# Install Icon
if [ -d "/usr/share/icons/hicolor/512x512/apps" ]; then
    echo "Installing app icon..."
    sudo cp Stow.png /usr/share/icons/hicolor/512x512/apps/stow.png || true
fi

# Install Desktop Entry
if [ -d "$HOME/.local/share/applications" ]; then
    echo "Installing desktop launcher..."
    cp Stow.desktop "$HOME/.local/share/applications/Stow.desktop"
    chmod +x "$HOME/.local/share/applications/Stow.desktop"
fi

echo "=== Installation Complete! ==="
echo "You can now run 'stow-gui' or launch 'Stow' from your application menu!"
