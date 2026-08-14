use stow_core::{embed_files, extract_payload, EmbedOptions};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

fn main() {
    let host_path = "test_fixtures/bench_host.png";
    let video_path = "test_fixtures/bench_video.mp4";
    let carrier_path = "test_fixtures/bench_carrier.png";
    let extracted_path = "test_fixtures/bench_extracted.mp4";

    // 1. Create a minimal valid PNG host image
    let mut host = Vec::new();
    let img = image::RgbImage::new(100, 100);
    img.write_to(&mut std::io::Cursor::new(&mut host), image::ImageFormat::Png).unwrap();
    File::create(host_path).unwrap().write_all(&host).unwrap();

    // 2. Create a 50 MB simulated video payload
    let size_50mb = 50 * 1024 * 1024;
    let mut payload_data = vec![0xABu8; size_50mb];
    payload_data[0..4].copy_from_slice(b"ftyp");
    File::create(video_path).unwrap().write_all(&payload_data).unwrap();

    println!("Benchmarking 50MB Password-Encrypted Embedding...");
    let t0 = Instant::now();
    let rep = embed_files(
        host_path,
        video_path,
        carrier_path,
        EmbedOptions { password: Some("TestBenchPassword123!".to_string()) },
        None,
    ).unwrap();
    let embed_elapsed = t0.elapsed().as_secs_f64();
    let embed_speed = (size_50mb as f64 / 1_000_000.0) / embed_elapsed;
    println!("  Encrypted Embed Speed: {:.2} MB/s (elapsed: {:.3}s)", embed_speed, embed_elapsed);

    println!("Benchmarking 50MB Password-Encrypted Extraction...");
    let t1 = Instant::now();
    let ext_rep = extract_payload(
        carrier_path,
        Some(extracted_path),
        Some("TestBenchPassword123!"),
        None,
    ).unwrap();
    let ext_elapsed = t1.elapsed().as_secs_f64();
    let ext_speed = (size_50mb as f64 / 1_000_000.0) / ext_elapsed;
    println!("  Encrypted Extract Speed: {:.2} MB/s (elapsed: {:.3}s)", ext_speed, ext_elapsed);

    assert_eq!(rep.blake3_hex, ext_rep.blake3_hex);
    println!("  Integrity Verified Bit-Perfect: BLAKE3 {}", rep.blake3_hex);
}
