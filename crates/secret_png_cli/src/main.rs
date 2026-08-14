use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use secret_png_core::{
    embed_files, extract_payload, inspect_carrier, strip_payload_in_place,
    strip_payload_to_file, EmbedOptions, ProgressUpdate,
};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "secret-png",
    author = "Antigravity Developer",
    version = "0.1.0",
    about = "High-performance streaming tool for embedding and extracting video files inside images without breaking viewability."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Embed a video file into a host image
    Embed {
        /// Host cover image (PNG, JPEG, WebP, GIF, BMP)
        #[arg(short = 'i', long = "image")]
        image: PathBuf,

        /// Video payload file to embed
        #[arg(short = 'v', long = "video")]
        video: PathBuf,

        /// Output carrier image path
        #[arg(short = 'o', long = "output")]
        output: PathBuf,

        /// Optional password to encrypt payload using ChaCha20-Poly1305 + Argon2id
        #[arg(short = 'p', long = "password")]
        password: Option<String>,
    },

    /// Extract embedded video from a carrier image
    Extract {
        /// Carrier image path
        #[arg(short = 'i', long = "image")]
        image: PathBuf,

        /// Destination output video path (defaults to original filename)
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,

        /// Password if the embedded payload is encrypted
        #[arg(short = 'p', long = "password")]
        password: Option<String>,
    },

    /// Inspect and display embedded carrier metadata in O(1) time
    Info {
        /// Carrier image path
        #[arg(short = 'i', long = "image")]
        image: PathBuf,
    },

    /// Strip embedded payload to restore the pristine host image
    Strip {
        /// Carrier image path
        #[arg(short = 'i', long = "image")]
        image: PathBuf,

        /// Output clean image path (optional if --in-place is used)
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,

        /// Truncate the file in-place without creating a copy
        #[arg(long = "in-place")]
        in_place: bool,
    },
}

fn create_progress_bar(total_bytes: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total_bytes);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA {eta}) {msg}")
            .expect("Failed to set progress bar template")
            .progress_chars("#>-")
    );
    pb.set_message(message.to_string());
    pb
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Embed {
            image,
            video,
            output,
            password,
        } => {
            println!("🔒 Embedding video into carrier image...");
            println!("  Host Image: {}", image.display());
            println!("  Video File: {}", video.display());
            println!("  Output:     {}", output.display());
            if password.is_some() {
                println!("  Security:   ChaCha20-Poly1305 (Password Encrypted)");
            } else {
                println!("  Security:   Unencrypted Raw Stream");
            }

            let pb = Arc::new(create_progress_bar(100, "Processing..."));
            let pb_clone = Arc::clone(&pb);

            let callback = Box::new(move |update: ProgressUpdate| {
                pb_clone.set_length(update.total_bytes);
                pb_clone.set_position(update.bytes_processed);
                pb_clone.set_message(update.phase);
            });

            match embed_files(
                &image,
                &video,
                &output,
                EmbedOptions { password },
                Some(callback),
            ) {
                Ok(report) => {
                    pb.finish_with_message("Embedding Completed Successfully!");
                    println!("\n✅ Carrier image generated successfully!");
                    println!("  Original Video:     {}", report.original_file_name);
                    println!("  Host Image Size:    {}", format_bytes(report.host_image_size));
                    println!("  Video Payload Size: {}", format_bytes(report.payload_size));
                    println!("  Total Carrier Size: {}", format_bytes(report.total_carrier_size));
                    println!("  BLAKE3 Integrity:   {}", report.blake3_hex);
                    println!("  CRC32:              0x{:08X}", report.crc32);
                    println!("  Elapsed Time:       {:.2}s", report.elapsed_millis as f64 / 1000.0);
                }
                Err(e) => {
                    pb.abandon_with_message("Embedding Failed");
                    eprintln!("\n❌ Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Extract {
            image,
            output,
            password,
        } => {
            println!("🔓 Extracting embedded video from carrier image...");
            println!("  Carrier Image: {}", image.display());

            let pb = Arc::new(create_progress_bar(100, "Extracting..."));
            let pb_clone = Arc::clone(&pb);

            let callback = Box::new(move |update: ProgressUpdate| {
                pb_clone.set_length(update.total_bytes);
                pb_clone.set_position(update.bytes_processed);
                pb_clone.set_message(update.phase);
            });

            match extract_payload(
                &image,
                output.as_ref(),
                password.as_deref(),
                Some(callback),
            ) {
                Ok(report) => {
                    pb.finish_with_message("Extraction Completed Successfully!");
                    println!("\n✅ Video extracted successfully!");
                    println!("  Extracted File:     {}", report.output_path.display());
                    println!("  Original Name:      {}", report.original_filename);
                    println!("  Payload Size:       {}", format_bytes(report.file_size));
                    println!("  BLAKE3 Integrity:   {}", report.blake3_hex);
                    println!("  CRC32:              0x{:08X}", report.crc32);
                    println!("  Encrypted:          {}", if report.is_encrypted { "Yes" } else { "No" });
                    println!("  Elapsed Time:       {:.2}s", report.elapsed_millis as f64 / 1000.0);
                }
                Err(e) => {
                    pb.abandon_with_message("Extraction Failed");
                    eprintln!("\n❌ Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Info { image } => {
            println!("🔍 Inspecting carrier image: {}", image.display());
            match inspect_carrier(&image) {
                Ok((trailer, meta)) => {
                    println!("\n📦 Carrier Metadata Detected:");
                    println!("  Protocol Version:   v{}", meta.protocol_version);
                    println!("  Original Filename:  {}", meta.original_filename);
                    println!("  Extension / MIME:   .{} ({})", meta.file_extension, meta.mime_type);
                    println!("  Original Size:      {}", format_bytes(meta.original_file_size));
                    println!("  Payload Stream:     {}", format_bytes(meta.payload_size));
                    println!("  Host Image Format:  {}", meta.host_image_format);
                    if let (Some(w), Some(h)) = (meta.host_image_width, meta.host_image_height) {
                        println!("  Host Dimensions:    {}x{} px", w, h);
                    }
                    println!("  Host Image Size:    {}", format_bytes(trailer.host_image_size));
                    println!("  Payload Offset:     @ byte {}", trailer.payload_offset);
                    println!("  Encrypted:          {}", if meta.is_encrypted { "YES (Password Protected)" } else { "NO (Raw Stream)" });
                    if let Some(enc) = &meta.encryption {
                        println!("  Cipher:             {}", enc.cipher);
                    }
                    println!("  BLAKE3 Checksum:    {}", meta.blake3_hex);
                    println!("  CRC-32 Checksum:    0x{:08X}", meta.crc32);
                }
                Err(e) => {
                    eprintln!("❌ Failed to inspect image: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Strip {
            image,
            output,
            in_place,
        } => {
            if in_place {
                println!("🧹 Stripping payload in-place from: {}", image.display());
                match strip_payload_in_place(&image) {
                    Ok(report) => {
                        println!("\n✅ Payload removed in-place!");
                        println!("  Restored Host Size: {}", format_bytes(report.original_host_image_size));
                        println!("  Bytes Removed:      {}", format_bytes(report.payload_bytes_removed));
                    }
                    Err(e) => {
                        eprintln!("❌ Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                let target_out = match output {
                    Some(p) => p,
                    None => {
                        eprintln!("❌ Error: Specify --output <PATH> or use --in-place to truncate.");
                        std::process::exit(1);
                    }
                };
                println!("🧹 Stripping payload from {} -> {}", image.display(), target_out.display());
                match strip_payload_to_file(&image, &target_out) {
                    Ok(report) => {
                        println!("\n✅ Pristine host image restored!");
                        println!("  Saved To:           {}", target_out.display());
                        println!("  Restored Host Size: {}", format_bytes(report.original_host_image_size));
                        println!("  Bytes Removed:      {}", format_bytes(report.payload_bytes_removed));
                    }
                    Err(e) => {
                        eprintln!("❌ Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}
