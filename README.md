<div align="center">

# 📦 Stow
### *Universal Stealth Carrier Engine*
**Conceal Any File of Any Size Inside Images, Audio, Video, PDFs & Executables Without Breaking Their Viewability or Playability.**

[![Rust](https://img.shields.io/badge/Rust-1.75+-black.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License: Komorebi 2.0.0](https://img.shields.io/badge/License-Komorebi_2.0.0-blue.svg?style=flat-square)](LICENSE.md)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Android-green.svg?style=flat-square)]()
[![Throughput](https://img.shields.io/badge/Throughput-1.3%2B_GB%2Fs-orange.svg?style=flat-square)]()
[![Security](https://img.shields.io/badge/Cipher-ChaCha20--Poly1305_AEAD-purple.svg?style=flat-square)]()

---

</div>

## 🌟 What Makes Stow Unique?

Traditional steganography tools are limited to hiding tiny kilobytes of text in pixels (LSB manipulation), causing visual distortion and crashing on large files.

**Stow** is the world's first **Universal Multi-Carrier Engine**. It uses a high-throughput, zero-RAM streaming architecture with a deterministic $O(1)$ trailing container protocol. You can conceal **multi-gigabyte files, databases, videos, and archives** inside ordinary everyday files while keeping the host carrier **100% valid, playable, and viewable**:

| Host Carrier Type | Supported Formats | Host Behavior After Concealment |
| :--- | :--- | :--- |
| **🖼️ Images** | `.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.bmp` | Opens instantly in **Windows Photos, Apple Preview, Android Gallery, MS Paint** |
| **🎵 Audio** | `.mp3`, `.wav`, `.flac`, `.aac`, `.ogg`, `.m4a` | Plays smoothly in **Spotify, Apple Music, VLC, Windows Media Player** |
| **🎬 Video** | `.mp4`, `.mkv`, `.mov`, `.webm`, `.avi`, `.wmv` | Streams seamlessly in **VLC, QuickTime, Chrome, TV Players** |
| **📄 Documents** | `.pdf` | Opens normally in **Adobe Acrobat, Chrome, Preview** |
| **⚙️ Executables** | `.exe`, `.dll`, `.iso`, `.bin` | Executes and runs normally as valid Windows PE Overlay data |

---

## ✨ Features

- **⚡ Zero-Limit Streaming Pipeline**: Stream 100 MB up to 50+ GB payloads with minimal constant RAM consumption (< 15 MB) at **1.3+ GB/s**.
- **🔐 Military-Grade Password Protection**: Authenticated encryption with **ChaCha20-Poly1305 AEAD** and **Argon2id** key derivation.
- **🛡️ Bit-Perfect BLAKE3 Integrity**: Instant 256-bit BLAKE3 cryptographic hash verification during extraction.
- **⏱️ Sub-Millisecond $O(1)$ Trailer Index**: Inspect carrier metadata, sizes, and encryption state in microseconds without scanning through gigabytes of data.
- **🎨 Built-in Theme Switcher**: 5 curated themes (Cyber Cyan, Midnight Violet, Emerald Matrix, Crimson Ruby, Monochrome Dark).
- **🧹 One-Click Carrier Cleaner**: Strip hidden payloads to restore pristine, untouched original host files.
- **📱 Universal Cross-Platform Ecosystem**: Pure Rust native GUI on desktop (Windows, macOS, Linux) and native Android app.

---

## 📦 Downloads (1 Clean File per Platform)

| Platform | Download | Instructions |
| :--- | :--- | :--- |
| **🪟 Windows (64-bit)** | [`Stow_Windows_x64.exe`](https://github.com/SubmitCodes/Stow/releases/latest/download/Stow_Windows_x64.exe) | Standalone `.exe` with embedded icon. Just double-click to run! |
| **🍎 macOS (Universal)** | [`Stow_macOS.dmg`](https://github.com/SubmitCodes/Stow/releases/latest/download/Stow_macOS.dmg) | Standard Apple `.dmg` installer. Drag `Stow` into Applications. |
| **🐧 Linux (64-bit)** | [`Stow_Linux_x86_64`](https://github.com/SubmitCodes/Stow/releases/latest/download/Stow_Linux_x86_64) | `chmod +x Stow_Linux_x86_64` and execute. |
| **📱 Android (APK)** | [`Stow_Android.apk`](https://github.com/SubmitCodes/Stow/releases/latest/download/Stow_Android.apk) | Download and install on Android. |

---

## 📖 How to Use

### 1. 📦 Embed & Cloak (Conceal)
1. Open **Stow** and stay on the **Embed & Cloak** tab.
2. Click **Browse Host...** to pick any cover file (`.png`, `.jpg`, `.mp3`, `.wav`, `.mp4`, `.pdf`, `.exe`).
3. Click **Browse Payload...** to select any secret file or archive (`.zip`, `.sqlite`, `.mkv`, `.iso`, `.txt`, etc.).
4. *(Optional)* Check **Protect with a Password** to encrypt with 256-bit ChaCha20-Poly1305.
5. Click **Conceal Payload into Carrier**. Your carrier is created and remains completely viewable/playable!

### 2. 🔓 Extract & Restore
1. Switch to the **Extract & Restore** tab.
2. Click **Browse Carrier...** and select your carrier file.
3. Stow automatically inspects the carrier in milliseconds and displays the concealed file metadata.
4. *(If password-protected)* Enter the password.
5. Click **Extract Payload from Carrier** to restore your file with full BLAKE3 checksum verification!

### 3. 🧹 Inspect & Clean
1. Switch to the **Inspect & Clean** tab.
2. Select any carrier file to view its internal container geometry and payload breakdown.
3. Click **Remove Concealed Payload** to strip the hidden payload and restore the original pristine host file.

---

## 💻 Command Line Interface (CLI)

```bash
# Embed a secret database into a cover song
stow embed -c song.mp3 -p database.sqlite -o song_carrier.mp3

# Embed with password protection
stow embed -c cover.jpg -p confidential.zip -o secret.jpg -w "MyMasterPassword!#99"

# Inspect carrier metadata in 0.001s
stow inspect -c secret.jpg

# Extract concealed payload
stow extract -c secret.jpg -w "MyMasterPassword!#99"

# Strip payload and restore pristine host file
stow strip -c secret.jpg -o pristine_cover.jpg
```

---

## 📄 License

Distributed under the **[Komorebi License Version 2.0.0](LICENSE.md)**.
