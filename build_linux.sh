#!/usr/bin/env bash
set -e

echo "=== Building Stow for Linux ==="

# Check for required development packages on Debian/Ubuntu
if command -v apt-get &> /dev/null; then
    echo "Checking required system dependencies..."
    sudo apt-get update -qq || true
    sudo apt-get install -y -qq pkg-config libssl-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libfontconfig1-dev || true
fi

echo "Building release workspace binaries..."
cargo build --release --workspace

echo "Packaging Linux release binaries into dist/linux/..."
mkdir -p dist/linux
cp target/release/secret_png_gui dist/linux/Stow_Linux_x86_64
cp target/release/secret_png_cli dist/linux/stow-cli
chmod +x dist/linux/Stow_Linux_x86_64 dist/linux/stow-cli

echo "=== Build Complete! ==="
echo "GUI Binary: dist/linux/Stow_Linux_x86_64"
echo "CLI Binary: dist/linux/stow-cli"
