# Secret PNG: Cross-Platform Video-in-Image Carrier Engine

A high-performance, memory-safe, cross-platform engine and application written in Rust that embeds arbitrary video files (and large media payloads) into host image files without breaking the host image's viewability, and extracts them back with bit-perfect integrity.

---

## 🌟 Key Features

- **Universal Polyglot & Ancillary Container**: Output files remain 100% valid, viewable images (PNG, JPEG, WebP, GIF, BMP) across all standard image decoders (Windows Photos, macOS Preview, iOS Photos, Android Gallery, Chrome/Safari/Firefox).
- **Zero-RAM Multi-Gigabyte Streaming**: Uses buffered I/O stream pipelines (`BufReader` / `BufWriter`) to embed and extract 10GB+ 4K videos with constant $O(1)$ memory consumption (< 15 MB RAM).
- **Deterministic Binary Protocol**: Embeds a 64-byte trailing index and CRC32/BLAKE3 metadata block for instant $O(1)$ header inspection in sub-milliseconds without scanning multi-gigabyte payloads.
- **Cryptographic AEAD Encryption**: Optional password protection using **ChaCha20-Poly1305** authenticated encryption with **Argon2id** key derivation.
- **Bit-Perfect Integrity**: Verifies BLAKE3 and CRC-32 checksums during extraction and guarantees exact byte-for-byte fidelity without residual trailing bytes.
- **Image Sanitizer & Payload Stripper**: Cleanly removes hidden payloads to restore the pristine original cover image.
- **Cross-Platform GUI & CLI**: Modern, dark-mode GUI built with pure Rust (`egui`/`eframe`), responsive file pickers (`rfd`), real-time throughput metrics (MB/s, ETA, progress), and a full-featured CLI tool.
- **Mobile Bridge (Android & iOS)**: C-ABI / Foreign Function Interface (FFI) bindings for Flutter (Dart FFI), Android Kotlin (Scoped Storage / Storage Access Framework), and iOS Swift (Security-Scoped URLs).

---

## 📐 Binary Protocol Specification

```
+-------------------------------------------------------------------------------+
| 🖼️ HOST IMAGE DATA                                                           |
| (PNG / JPEG / WEBP / GIF / BMP)                                               |
| e.g. PNG ends at standard IEND chunk: [00 00 00 00 49 45 4E 44 AE 42 60 82]   |
+-------------------------------------------------------------------------------+
| 🎬 PAYLOAD STREAM (Multi-GB Buffered Copy)                                    |
| (Raw or ChaCha20-Poly1305 Encrypted Video Stream)                             |
+-------------------------------------------------------------------------------+
| 📋 METADATA BLOCK (JSON serialized)                                           |
| - Protocol Version: u16                                                       |
| - Original Filename & Extension (UTF-8)                                       |
| - MIME Type (e.g. video/mp4, video/x-matroska, video/quicktime)               |
| - Original & Payload Sizes: u64                                               |
| - BLAKE3 Checksum: 32 bytes (hex)                                             |
| - CRC32 Checksum: u32                                                         |
| - Encryption Salt & Nonce (if encrypted)                                      |
| - Timestamp: u64                                                              |
+-------------------------------------------------------------------------------+
| 📍 TRAILER INDEX (Fixed 64 bytes at exact EOF)                                |
| 00..16: Magic b"SECRETPNG_V1\x00\x00\x00\x00"                                 |
| 16..18: Version (u16)                                                         |
| 18..20: Flags / Cipher Mode (u16)                                             |
| 20..28: Host Image Size / Payload Offset (u64)                                |
| 28..36: Payload Length (u64)                                                  |
| 36..44: Metadata Offset (u64)                                                 |
| 44..48: Metadata Length (u32)                                                 |
| 48..52: Metadata CRC32 (u32)                                                  |
| 52..56: Reserved Padding (4 bytes)                                            |
| 56..60: Trailer CRC32 (u32)                                                   |
| 60..64: Terminator [0x55, 0xAA, 0x55, 0xAA]                                   |
+-------------------------------------------------------------------------------+
```

---

## 📦 Project Structure

```
.
├── Cargo.toml                     # Workspace configuration
├── crates/
│   ├── secret_png_core/           # Streaming core engine, protocol & crypto
│   │   ├── src/
│   │   │   ├── lib.rs             # Public API
│   │   │   ├── protocol.rs        # Binary trailer and metadata structures
│   │   │   ├── crypto.rs          # ChaCha20-Poly1305 & Argon2id streaming cipher
│   │   │   ├── embedder.rs        # Buffered streaming embedder & progress
│   │   │   ├── extractor.rs       # O(1) inspector & streaming extractor
│   │   │   ├── sanitizer.rs       # Payload stripper / host restoration
│   │   │   └── error.rs           # Granular error types
│   │   └── tests/
│   │       └── roundtrip_test.rs  # Unit, integration & security tests
│   ├── secret_png_cli/            # Command line binary with progress bars
│   │   └── src/main.rs
│   ├── secret_png_gui/            # Modern Desktop & Mobile UI (egui/eframe)
│   │   ├── src/main.rs
│   │   └── src/app.rs
│   └── secret_png_ffi/            # C-ABI and dynamic library for foreign bindings
│       └── src/lib.rs
└── mobile/
    ├── flutter_bridge/            # Flutter Dart FFI implementation
    │   └── secret_png_ffi.dart
    ├── android/                   # Android Kotlin SAF / JNI Bridge
    │   └── SecretPngEngine.kt
    └── ios/                       # iOS Swift DocumentPicker Bridge
        └── SecretPngBridge.swift
```

---

## 🚀 Quick Start

### 1. Build and Run GUI
```bash
cargo run -p secret_png_gui --release
```

### 2. Command-Line Interface (CLI)

#### Embed a video into an image:
```bash
# Unencrypted embedding
cargo run -p secret_png_cli -- embed -i cover.png -v secret.mp4 -o carrier.png

# Password-encrypted embedding
cargo run -p secret_png_cli -- embed -i cover.jpg -v confidential.mkv -o carrier.jpg --password "MySecretPassphrase"
```

#### Inspect carrier metadata in $O(1)$ time:
```bash
cargo run -p secret_png_cli -- info -i carrier.png
```

#### Extract the embedded video:
```bash
# Unencrypted extraction
cargo run -p secret_png_cli -- extract -i carrier.png -o extracted.mp4

# Password-protected extraction
cargo run -p secret_png_cli -- extract -i carrier.jpg -o extracted.mkv --password "MySecretPassphrase"
```

#### Strip payload and restore original host image:
```bash
# Save to clean copy
cargo run -p secret_png_cli -- strip -i carrier.png -o clean_cover.png

# Or truncate in-place
cargo run -p secret_png_cli -- strip -i carrier.png --in-place
```

---

## 📱 Mobile Platform Integration

### Flutter (Android / iOS / Desktop)
Include `mobile/flutter_bridge/secret_png_ffi.dart` in your Flutter project:
```dart
final engine = SecretPngEngine();

// Inspect
final meta = engine.inspect("/path/to/carrier.png");
print("Found embedded: ${meta.originalFilename}, Size: ${meta.originalFileSize} bytes");

// Embed
await engine.embed(
  hostPath: "/path/to/cover.png",
  payloadPath: "/path/to/video.mp4",
  outputPath: "/path/to/output_carrier.png",
  password: "optional_password",
);

// Extract
await engine.extract(
  carrierPath: "/path/to/output_carrier.png",
  outputPath: "/path/to/recovered_video.mp4",
  password: "optional_password",
);
```

### Android (Kotlin with Scoped Storage / SAF)
Compile the C library using `cargo-ndk`:
```bash
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o android/app/src/main/jniLibs build -p secret_png_ffi --release
```
Use `SecretPngEngine.kt` to handle Android Content URIs cleanly:
```kotlin
val engine = SecretPngEngine.instance
val report = engine.embedVideo(context, hostUri, videoUri, outputFile, "secret_password")
```

### iOS (Swift & Xcode)
Cross-compile universal static library:
```bash
cargo build -p secret_png_ffi --target aarch64-apple-ios --release
cargo build -p secret_png_ffi --target aarch64-apple-ios-sim --release
```
Add `SecretPngBridge.swift` into your Xcode project and interact with `UIDocumentPickerViewController` security-scoped URLs seamlessly.

---

## 🧪 Testing & Verification

Run the full test suite:
```bash
cargo test --workspace
```
Tests cover:
- ✅ Bit-perfect video extraction roundtrip on PNG and JPEG images.
- ✅ ChaCha20-Poly1305 password encryption & decryption validation.
- ✅ Wrong-password rejection and tamper detection.
- ✅ Carrier validation ensuring host images remain 100% valid viewable image files.
- ✅ Payload stripping and byte-for-byte host image restoration.
