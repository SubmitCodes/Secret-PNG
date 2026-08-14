use image::RgbImage;
use stow_core::{embed_files, extract_payload, inspect_carrier, EmbedOptions};
use std::fs::File;
use std::io::Write;

fn main() {
    let host_png = "test_fixtures/user_test_host.png";
    let video_payload = "test_fixtures/user_test_video.mkv";
    let carrier_png = "test_fixtures/user_test_carrier.png";
    let extracted_mkv = "test_fixtures/user_test_extracted.mkv";

    // 1. Create realistic host PNG
    let img = RgbImage::new(800, 600);
    img.save(host_png).unwrap();

    // 2. Create 100MB dummy MKV
    let size_100mb = 100 * 1024 * 1024;
    let mut data = vec![0x42u8; size_100mb];
    data[0..4].copy_from_slice(b"\x1A\x45\xDF\xA3"); // Matroska magic
    File::create(video_payload).unwrap().write_all(&data).unwrap();

    // 3. Embed with password
    println!("Embedding 100MB MKV into PNG with password...");
    let rep = embed_files(
        host_png,
        video_payload,
        carrier_png,
        EmbedOptions { password: Some("mypassword".to_string()) },
        None,
    ).unwrap();
    println!("Embedded successfully: total carrier size = {} bytes", rep.total_carrier_size);

    // 4. Verify carrier is byte-for-byte pure
    let (_trailer, meta) = inspect_carrier(carrier_png).unwrap();
    println!("Carrier inspected: original name = {}, size = {}", meta.original_filename, meta.original_file_size);

    // 5. Extract
    let ext = extract_payload(carrier_png, Some(extracted_mkv), Some("mypassword"), None).unwrap();
    println!("Extracted successfully: size = {}, blake3 = {}", ext.file_size, ext.blake3_hex);
    assert_eq!(rep.blake3_hex, ext.blake3_hex);
}
