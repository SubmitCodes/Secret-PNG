use secret_png_core::{embed_files, extract_payload, inspect_carrier, EmbedOptions};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

fn main() {
    let png_host = "test_fixtures/large_test_host.png";
    let jpg_host = "test_fixtures/large_test_host.jpg";
    let video_100mb = "test_fixtures/large_test_video.mp4";
    let png_carrier = "test_fixtures/large_carrier.png";
    let jpg_carrier = "test_fixtures/large_carrier.jpg";
    let extracted_from_png = "test_fixtures/extracted_from_png.mp4";
    let extracted_from_jpg = "test_fixtures/extracted_from_jpg.mp4";

    // 1. Create host PNG and host JPEG
    let img = image::RgbImage::new(400, 300);
    img.save(png_host).unwrap();
    img.save(jpg_host).unwrap();

    // 2. Create 100 MB simulated video
    let size_100mb = 100 * 1024 * 1024;
    let mut dummy_video = vec![0x33u8; size_100mb];
    dummy_video[0..12].copy_from_slice(b"\x00\x00\x00\x18ftypmp42");
    File::create(video_100mb).unwrap().write_all(&dummy_video).unwrap();

    println!("============================================================");
    println!("TEST 1: 100 MB Video into PNG (Safe 64KB Ancillary Chunks)");
    println!("============================================================");
    let t0 = Instant::now();
    let rep_png = embed_files(
        png_host,
        video_100mb,
        png_carrier,
        EmbedOptions { password: Some("SecretPass123!".to_string()) },
        None,
    ).unwrap();
    println!("  PNG Embed: {:.2} MB/s (elapsed: {:.3}s)", (size_100mb as f64 / 1e6) / t0.elapsed().as_secs_f64(), t0.elapsed().as_secs_f64());

    let t1 = Instant::now();
    let ext_png = extract_payload(
        png_carrier,
        Some(extracted_from_png),
        Some("SecretPass123!"),
        None,
    ).unwrap();
    println!("  PNG Extract: {:.2} MB/s (elapsed: {:.3}s)", (size_100mb as f64 / 1e6) / t1.elapsed().as_secs_f64(), t1.elapsed().as_secs_f64());
    assert_eq!(rep_png.blake3_hex, ext_png.blake3_hex);
    println!("  PNG Integrity: Bit-Perfect BLAKE3 Match!");

    println!("\n============================================================");
    println!("TEST 2: 100 MB Video into JPEG (Direct Zero-Overhead Stream)");
    println!("============================================================");
    let t2 = Instant::now();
    let rep_jpg = embed_files(
        jpg_host,
        video_100mb,
        jpg_carrier,
        EmbedOptions { password: Some("SecretPass123!".to_string()) },
        None,
    ).unwrap();
    println!("  JPEG Embed: {:.2} MB/s (elapsed: {:.3}s)", (size_100mb as f64 / 1e6) / t2.elapsed().as_secs_f64(), t2.elapsed().as_secs_f64());

    let t3 = Instant::now();
    let ext_jpg = extract_payload(
        jpg_carrier,
        Some(extracted_from_jpg),
        Some("SecretPass123!"),
        None,
    ).unwrap();
    println!("  JPEG Extract: {:.2} MB/s (elapsed: {:.3}s)", (size_100mb as f64 / 1e6) / t3.elapsed().as_secs_f64(), t3.elapsed().as_secs_f64());
    assert_eq!(rep_jpg.blake3_hex, ext_jpg.blake3_hex);
    println!("  JPEG Integrity: Bit-Perfect BLAKE3 Match!");
}
