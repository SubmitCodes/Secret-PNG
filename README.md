<p align="center">
  <img src="Stow.png" width="120" alt="Stow Logo" />
</p>

<h1 align="center">Stow</h1>

<p align="center">
  <b>High-speed steganographic video concealer and extractor with zero size limits.</b><br/>
  Embed multi-gigabyte videos, movies, and archives into everyday images without breaking their viewability.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust%20%7C%20Kotlin-orange?style=flat-square" />
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Android-blue?style=flat-square" />
  <img src="https://img.shields.io/badge/Encryption-ChaCha20--Poly1305%20%2B%20Argon2id-green?style=flat-square" />
  <img src="https://img.shields.io/badge/License-MIT-purple?style=flat-square" />
</p>

---

## 🌟 Key Features

- **🖼️ 100% Image Viewability**: Output carrier files remain completely valid, normal images across all standard image viewers (Windows Photos, MS Paint, macOS Preview, iOS Photos, Android Gallery, Chrome/Safari).
- **⚡ Universal Zero-Limit Streaming**: Embed massive multi-gigabyte files (10 GB+, 50 GB+ 4K movies) with constant minimal memory usage (< 15 MB RAM) and up to 1.3+ GB/s throughput.
- **🔐 Military-Grade Encryption**: Optional password protection powered by **ChaCha20-Poly1305 AEAD** with **Argon2id** key derivation.
- **🛡️ Bit-Perfect Integrity**: Verifies BLAKE3 checksums during extraction for flawless byte-for-byte fidelity.
- **🎨 Built-in Theme Switcher**: 5 curated themes (Cyber Cyan, Midnight Violet, Emerald Matrix, Crimson Ruby, Monochrome Dark).
- **📱 Native Android App**: Jetpack Compose Android app with Scoped Storage support and password protection.
- **🧹 Image Cleaner**: One-click payload removal to restore pristine original cover images.

---

## 📦 Direct Downloads

Grab the latest standalone release for your platform from the [Releases Page](https://github.com/SubmitCodes/Stow/releases):

| Platform | Download File | Description |
| :--- | :--- | :--- |
| **🪟 Windows (64-bit)** | `Stow_Windows_x64.exe` | Standalone `.exe` with embedded icon (No installation needed). |
| **🍎 macOS (App Bundle)** | `Stow_macOS.zip` | Full macOS `Stow.app` with native `.icns` icon (Apple Silicon + Intel). |
| **🍎 macOS (Universal Binary)** | `Stow_macOS_universal` | Standalone universal command-line / desktop binary. |
| **🐧 Linux (64-bit)** | `Stow_Linux_x86_64` | Standalone Linux desktop binary. |
| **📱 Android (APK)** | `Stow_Android.apk` | Native Android application. |

---

## 🛠️ Building from Source

### 1. Windows & Linux & macOS
Ensure you have [Rust](https://rustup.rs/) installed:
```bash
cargo build --release --workspace
```
The compiled binaries will be in `target/release/` (`stow-gui` / `secret_png_gui.exe`).

### 2. Linux One-Click Script
```bash
chmod +x build_linux.sh
./build_linux.sh
```

### 3. Android APK
Ensure you have JDK 17 and Android SDK:
```bash
cd android_app
./gradlew assembleDebug
```
The output APK will be in `android_app/app/build/outputs/apk/debug/app-debug.apk`.

---

## 📐 Binary Protocol Specification

```
+-------------------------------------------------------------------------------+
| 🖼️ HOST IMAGE COVER                                                           |
| (PNG / JPEG / WebP / GIF / BMP)                                               |
+-------------------------------------------------------------------------------+
| 🎬 PAYLOAD DATA STREAM                                                        |
| (Raw or ChaCha20-Poly1305 Encrypted Stream)                                   |
+-------------------------------------------------------------------------------+
| 📋 METADATA BLOCK (JSON serialized)                                           |
| - Original Filename & Extension (UTF-8)                                       |
| - MIME Type (e.g. video/mp4, video/x-matroska)                                |
| - Original & Payload Sizes: u64                                               |
| - BLAKE3 Checksum: 32 bytes (hex)                                             |
| - Encryption Salt & Nonce (if encrypted)                                      |
+-------------------------------------------------------------------------------+
| 📍 TRAILER INDEX (Fixed 64 bytes at exact EOF)                                |
| Magic b"SECRETPNG_V1\x00\x00\x00\x00" | Offset Pointers | CRC32 Checksums      |
+-------------------------------------------------------------------------------+
```

---

## 📄 License
This project is open-source under the MIT License.
