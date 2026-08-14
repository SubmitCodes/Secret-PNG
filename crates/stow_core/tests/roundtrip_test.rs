use stow_core::{
    embed_files, extract_payload, has_carrier_payload, inspect_carrier,
    strip_payload_to_file, EmbedOptions,
};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

/// Helper to generate a minimal valid MP3 audio header
fn create_dummy_audio(path: &std::path::Path, size_bytes: usize) {
    let mut data = Vec::with_capacity(size_bytes);
    // Standard MP3 frame sync header (0xFF, 0xFB)
    data.extend_from_slice(b"\xFF\xFB\x90\x00\x00\x00\x00\x00");
    while data.len() < size_bytes {
        data.push(0x55);
    }
    let mut f = File::create(path).unwrap();
    f.write_all(&data).unwrap();
}

/// Helper to generate a minimal valid MP4 video container
fn create_dummy_video(path: &std::path::Path, size_bytes: usize) {
    let mut data = Vec::with_capacity(size_bytes);
    data.extend_from_slice(b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00isomiso2mp41");
    while data.len() < size_bytes {
        data.push(0xAA);
    }
    let mut f = File::create(path).unwrap();
    f.write_all(&data).unwrap();
}

/// Helper to generate a minimal valid PDF document
fn create_dummy_pdf(path: &std::path::Path, size_bytes: usize) {
    let mut data = Vec::with_capacity(size_bytes);
    data.extend_from_slice(b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n%%EOF\n");
    while data.len() < size_bytes {
        data.push(0x20); // space padding before EOF
    }
    let mut f = File::create(path).unwrap();
    f.write_all(&data).unwrap();
}

/// Helper to generate a minimal valid Windows PE executable
fn create_dummy_exe(path: &std::path::Path, size_bytes: usize) {
    let mut data = Vec::with_capacity(size_bytes);
    data.extend_from_slice(b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xFF\xFF\x00\x00");
    while data.len() < size_bytes {
        data.push(0x90); // NOP sled
    }
    let mut f = File::create(path).unwrap();
    f.write_all(&data).unwrap();
}

/// Helper to generate a synthetic payload
fn create_dummy_payload(path: &std::path::Path, size_bytes: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size_bytes);
    for i in 0..size_bytes {
        data.push(((i * 37 + 13) % 256) as u8);
    }
    let mut f = File::create(path).unwrap();
    f.write_all(&data).unwrap();
    data
}

#[test]
fn test_audio_carrier_roundtrip() {
    let dir = tempdir().unwrap();
    let host_audio = dir.path().join("song.mp3");
    let payload = dir.path().join("secret_database.sqlite");
    let carrier = dir.path().join("song_carrier.mp3");
    let extracted = dir.path().join("extracted_database.sqlite");

    create_dummy_audio(&host_audio, 128 * 1024); // 128 KB MP3
    let original_bytes = create_dummy_payload(&payload, 256 * 1024); // 256 KB payload

    // 1. Embed inside MP3 Audio Carrier
    let embed_report = embed_files(&host_audio, &payload, &carrier, EmbedOptions::default(), None).unwrap();
    assert_eq!(embed_report.original_file_name, "secret_database.sqlite");
    assert_eq!(embed_report.payload_size, 256 * 1024);

    // 2. Carrier maintains data
    assert!(has_carrier_payload(&carrier));
    let (_trailer, meta) = inspect_carrier(&carrier).unwrap();
    assert_eq!(meta.original_filename, "secret_database.sqlite");
    assert_eq!(meta.host_format, "MP3");

    // 3. Extract back with bit-perfect BLAKE3 integrity
    let extract_report = extract_payload(&carrier, Some(&extracted), None, None).unwrap();
    assert_eq!(extract_report.file_size, 256 * 1024);
    assert_eq!(extract_report.blake3_hex, embed_report.blake3_hex);

    let extracted_bytes = std::fs::read(&extracted).unwrap();
    assert_eq!(extracted_bytes, original_bytes);
}

#[test]
fn test_video_carrier_encrypted_roundtrip() {
    let dir = tempdir().unwrap();
    let host_video = dir.path().join("clip.mp4");
    let payload = dir.path().join("financials.xlsx");
    let carrier = dir.path().join("clip_carrier.mp4");
    let extracted = dir.path().join("extracted_financials.xlsx");

    create_dummy_video(&host_video, 512 * 1024);
    let original_bytes = create_dummy_payload(&payload, 100 * 1024);
    let password = "SecretMasterPassword!#99";

    // 1. Embed with ChaCha20-Poly1305 inside MP4 Video
    let embed_report = embed_files(
        &host_video,
        &payload,
        &carrier,
        EmbedOptions {
            password: Some(password.to_string()),
        },
        None,
    ).unwrap();
    assert!(embed_report.is_encrypted);

    // 2. Inspect reveals encrypted metadata
    assert!(has_carrier_payload(&carrier));
    let (_trailer, meta) = inspect_carrier(&carrier).unwrap();
    assert!(meta.is_encrypted);
    assert_eq!(meta.host_format, "MP4");

    // 3. Extract with password
    let extract_report = extract_payload(&carrier, Some(&extracted), Some(password), None).unwrap();
    assert_eq!(extract_report.blake3_hex, embed_report.blake3_hex);

    let extracted_bytes = std::fs::read(&extracted).unwrap();
    assert_eq!(extracted_bytes, original_bytes);
}

#[test]
fn test_pdf_carrier_roundtrip() {
    let dir = tempdir().unwrap();
    let host_pdf = dir.path().join("whitepaper.pdf");
    let payload = dir.path().join("keys.txt");
    let carrier = dir.path().join("whitepaper_carrier.pdf");
    let extracted = dir.path().join("extracted_keys.txt");

    create_dummy_pdf(&host_pdf, 64 * 1024);
    let original_bytes = create_dummy_payload(&payload, 16 * 1024);

    embed_files(&host_pdf, &payload, &carrier, EmbedOptions::default(), None).unwrap();
    assert!(has_carrier_payload(&carrier));

    let extract_report = extract_payload(&carrier, Some(&extracted), None, None).unwrap();
    let extracted_bytes = std::fs::read(&extracted).unwrap();
    assert_eq!(extracted_bytes, original_bytes);
    assert_eq!(extract_report.file_size, 16 * 1024);
}

#[test]
fn test_exe_carrier_roundtrip() {
    let dir = tempdir().unwrap();
    let host_exe = dir.path().join("installer.exe");
    let payload = dir.path().join("firmware.bin");
    let carrier = dir.path().join("installer_carrier.exe");
    let extracted = dir.path().join("extracted_firmware.bin");

    create_dummy_exe(&host_exe, 256 * 1024);
    let original_bytes = create_dummy_payload(&payload, 64 * 1024);

    embed_files(&host_exe, &payload, &carrier, EmbedOptions::default(), None).unwrap();
    assert!(has_carrier_payload(&carrier));

    let extract_report = extract_payload(&carrier, Some(&extracted), None, None).unwrap();
    let extracted_bytes = std::fs::read(&extracted).unwrap();
    assert_eq!(extracted_bytes, original_bytes);
    assert_eq!(extract_report.file_size, 64 * 1024);
}

#[test]
fn test_sanitizer_multi_carrier_stripping() {
    let dir = tempdir().unwrap();
    let host_mp3 = dir.path().join("audio.mp3");
    let payload = dir.path().join("archive.zip");
    let carrier = dir.path().join("audio_carrier.mp3");
    let cleaned = dir.path().join("audio_cleaned.mp3");

    create_dummy_audio(&host_mp3, 100 * 1024);
    let original_host_bytes = std::fs::read(&host_mp3).unwrap();
    create_dummy_payload(&payload, 50 * 1024);

    embed_files(&host_mp3, &payload, &carrier, EmbedOptions::default(), None).unwrap();
    assert!(has_carrier_payload(&carrier));

    // Strip payload and restore original host
    let report = strip_payload_to_file(&carrier, &cleaned).unwrap();
    assert_eq!(report.original_host_image_size, original_host_bytes.len() as u64);
    assert!(!has_carrier_payload(&cleaned));

    let cleaned_bytes = std::fs::read(&cleaned).unwrap();
    assert_eq!(cleaned_bytes, original_host_bytes);
}
