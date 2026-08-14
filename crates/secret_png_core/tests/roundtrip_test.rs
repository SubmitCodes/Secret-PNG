use image::{ImageBuffer, Rgb};
use secret_png_core::{
    embed_files, extract_payload, has_carrier_payload, inspect_carrier,
    strip_payload_to_file, EmbedOptions, SecretPngError,
};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

/// Helper to generate a minimal valid PNG file
fn create_dummy_png(path: &std::path::Path, width: u32, height: u32) {
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([(x % 255) as u8, (y % 255) as u8, 128])
        });
    img.save(path).expect("Failed to save dummy PNG");
}

/// Helper to generate a minimal valid JPEG file
fn create_dummy_jpeg(path: &std::path::Path, width: u32, height: u32) {
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([(x % 255) as u8, (y % 255) as u8, 200])
        });
    img.save(path).expect("Failed to save dummy JPEG");
}

/// Helper to generate a synthetic video payload (e.g. simulated MP4 bytes)
fn create_dummy_video(path: &std::path::Path, size_bytes: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size_bytes);
    // Simulated MP4 header ftypbox
    let ftyp = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00isommp42";
    data.extend_from_slice(ftyp);
    while data.len() < size_bytes {
        let val = (data.len() * 31 + 17) % 256;
        data.push(val as u8);
    }
    let mut file = File::create(path).expect("Failed to create dummy video");
    file.write_all(&data).expect("Failed to write video data");
    data
}

#[test]
fn test_unencrypted_roundtrip_png() {
    let dir = tempdir().unwrap();
    let host_path = dir.path().join("cover.png");
    let video_path = dir.path().join("secret_recording.mp4");
    let carrier_path = dir.path().join("carrier.png");
    let extracted_path = dir.path().join("extracted.mp4");

    create_dummy_png(&host_path, 200, 150);
    let original_video_bytes = create_dummy_video(&video_path, 256 * 1024); // 256 KB

    // 1. Embed video into PNG
    let embed_report = embed_files(
        &host_path,
        &video_path,
        &carrier_path,
        EmbedOptions::default(),
        None,
    )
    .expect("Embedding failed");

    assert_eq!(embed_report.original_file_name, "secret_recording.mp4");
    assert_eq!(embed_report.payload_size, 256 * 1024);
    assert!(!embed_report.is_encrypted);

    // 2. Verify carrier file is STILL a 100% valid viewable image!
    assert!(has_carrier_payload(&carrier_path));
    let loaded_image = image::open(&carrier_path).expect("Carrier MUST remain a valid image!");
    assert_eq!(loaded_image.width(), 200);
    assert_eq!(loaded_image.height(), 150);

    // 3. Inspect carrier metadata in O(1) time
    let (trailer, meta) = inspect_carrier(&carrier_path).expect("Inspect failed");
    assert_eq!(meta.original_filename, "secret_recording.mp4");
    assert_eq!(meta.original_file_size, 256 * 1024);
    assert_eq!(meta.mime_type, "video/mp4");
    assert_eq!(trailer.host_image_size, std::fs::metadata(&host_path).unwrap().len());

    // 4. Extract payload
    let extract_report = extract_payload(
        &carrier_path,
        Some(&extracted_path),
        None,
        None,
    )
    .expect("Extraction failed");

    assert_eq!(extract_report.file_size, 256 * 1024);
    assert_eq!(extract_report.blake3_hex, embed_report.blake3_hex);
    assert_eq!(extract_report.crc32, embed_report.crc32);

    // 5. Compare extracted bytes byte-for-byte
    let extracted_bytes = std::fs::read(&extracted_path).expect("Read extracted file");
    assert_eq!(extracted_bytes, original_video_bytes);
}

#[test]
fn test_encrypted_roundtrip_jpeg() {
    let dir = tempdir().unwrap();
    let host_path = dir.path().join("scenery.jpg");
    let video_path = dir.path().join("confidential_drone.mkv");
    let carrier_path = dir.path().join("carrier.jpg");
    let extracted_path = dir.path().join("extracted.mkv");

    create_dummy_jpeg(&host_path, 300, 200);
    let original_video_bytes = create_dummy_video(&video_path, 512 * 1024); // 512 KB

    let password = "SuperSecretPassword123!#";

    // 1. Embed video with encryption
    let embed_report = embed_files(
        &host_path,
        &video_path,
        &carrier_path,
        EmbedOptions {
            password: Some(password.to_string()),
        },
        None,
    )
    .expect("Encrypted embedding failed");

    assert!(embed_report.is_encrypted);

    // 2. Verify carrier is STILL a valid JPEG
    let loaded_img = image::open(&carrier_path).expect("Encrypted carrier must remain valid JPEG");
    assert_eq!(loaded_img.width(), 300);
    assert_eq!(loaded_img.height(), 200);

    // 3. Extraction without password must fail with PasswordRequired
    let err_no_pass = extract_payload(&carrier_path, Some(&extracted_path), None, None)
        .expect_err("Should require password");
    match err_no_pass {
        SecretPngError::PasswordRequired => {}
        other => panic!("Expected PasswordRequired error, got {:?}", other),
    }

    // 4. Extraction with wrong password must fail with DecryptionFailed
    let err_wrong_pass = extract_payload(
        &carrier_path,
        Some(&extracted_path),
        Some("WrongPassword456!"),
        None,
    )
    .expect_err("Should fail with wrong password");
    match err_wrong_pass {
        SecretPngError::DecryptionFailed => {}
        other => panic!("Expected DecryptionFailed error, got {:?}", other),
    }

    // 5. Extraction with correct password succeeds and matches bit-for-bit
    let extract_report = extract_payload(
        &carrier_path,
        Some(&extracted_path),
        Some(password),
        None,
    )
    .expect("Extraction with correct password should succeed");

    assert_eq!(extract_report.file_size, 512 * 1024);
    assert_eq!(extract_report.blake3_hex, embed_report.blake3_hex);
    assert_eq!(extract_report.crc32, embed_report.crc32);

    let extracted_bytes = std::fs::read(&extracted_path).unwrap();
    assert_eq!(extracted_bytes, original_video_bytes);
}

#[test]
fn test_sanitizer_payload_stripping() {
    let dir = tempdir().unwrap();
    let host_path = dir.path().join("original.png");
    let video_path = dir.path().join("clip.mp4");
    let carrier_path = dir.path().join("carrier.png");
    let clean_restored_path = dir.path().join("cleaned.png");

    create_dummy_png(&host_path, 100, 100);
    let original_host_bytes = std::fs::read(&host_path).unwrap();
    create_dummy_video(&video_path, 64 * 1024);

    embed_files(
        &host_path,
        &video_path,
        &carrier_path,
        EmbedOptions::default(),
        None,
    )
    .unwrap();

    assert!(has_carrier_payload(&carrier_path));

    // Strip payload to restored file
    let sanitize_report = strip_payload_to_file(&carrier_path, &clean_restored_path).unwrap();
    assert_eq!(sanitize_report.original_host_image_size, original_host_bytes.len() as u64);

    // Restored file should NOT have carrier payload anymore
    assert!(!has_carrier_payload(&clean_restored_path));

    // Restored file must be byte-for-byte identical to the original host image
    let restored_bytes = std::fs::read(&clean_restored_path).unwrap();
    assert_eq!(restored_bytes, original_host_bytes);
}
