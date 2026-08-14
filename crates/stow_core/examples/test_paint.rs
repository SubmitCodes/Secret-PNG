use image::{ImageBuffer, Rgb};
use stow_core::{embed_files, extract_payload, inspect_carrier, EmbedOptions};
use std::fs::File;
use std::io::Write;

fn main() {
    let host_path = "test_fixtures/test_paint_host.png";
    let video_path = "test_fixtures/test_paint_video.mp4";
    let carrier_path = "test_fixtures/test_paint_carrier.png";
    let extracted_path = "test_fixtures/test_paint_extracted.mp4";

    // 1. Create a 400x300 host PNG
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(400, 300, |x, y| {
        Rgb([(x % 255) as u8, (y % 255) as u8, 180])
    });
    img.save(host_path).unwrap();

    // 2. Create a simulated 20 MB video payload
    let mut video_data = Vec::with_capacity(20 * 1024 * 1024);
    video_data.extend_from_slice(b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00isommp42");
    while video_data.len() < 20 * 1024 * 1024 {
        video_data.extend_from_slice(&[0x41, 0x42, 0x43, 0x44, 0x55, 0x66, 0x77, 0x88]);
    }
    File::create(video_path).unwrap().write_all(&video_data).unwrap();

    // 3. Embed video into PNG using new seCr ancillary chunk mode
    let embed_rep = embed_files(host_path, video_path, carrier_path, EmbedOptions::default(), None).unwrap();
    println!("Embedded successfully: total carrier size = {} bytes", embed_rep.total_carrier_size);

    // 4. Verify carrier is a valid image
    let loaded = image::open(carrier_path).unwrap();
    println!("Loaded image via image-rs: {}x{}", loaded.width(), loaded.height());

    // 5. Inspect in O(1) time
    let (_trailer, meta) = inspect_carrier(carrier_path).unwrap();
    println!("Inspected meta: name = {}, size = {}", meta.original_filename, meta.original_file_size);

    // 6. Extract
    let ext_rep = extract_payload(carrier_path, Some(extracted_path), None, None).unwrap();
    println!("Extracted successfully: size = {}, blake3 = {}", ext_rep.file_size, ext_rep.blake3_hex);
}
