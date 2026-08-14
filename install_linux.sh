#!/usr/bin/env bash
set -e

echo "=== Installing Secret PNG on Linux ==="

# Build release binaries if not already built
if [ ! -f "target/release/secret_png_gui" ]; then
    echo "Compiling release binaries..."
    ./build_linux.sh
fi

echo "Installing binaries to /usr/local/bin..."
sudo install -m 755 target/release/secret_png_gui /usr/local/bin/secret_png_gui
sudo install -m 755 target/release/secret_png_cli /usr/local/bin/secret-png

# Install Desktop Entry
if [ -d "$HOME/.local/share/applications" ]; then
    echo "Installing desktop launcher..."
    cp SecretPNG.desktop "$HOME/.local/share/applications/SecretPNG.desktop"
    chmod +x "$HOME/.local/share/applications/SecretPNG.desktop"
fi

echo "=== Installation Complete! ==="
echo "You can now run 'secret_png_gui' or launch 'Secret PNG' from your application menu!"
