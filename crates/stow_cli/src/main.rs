use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use stow_core::{
    embed_files, extract_payload, has_carrier_payload, inspect_carrier, strip_payload_to_file,
    EmbedOptions, ProgressUpdate,
};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "stow",
    author = "SubmitCodes",
    version = "1.0.0",
    about = "Universal Stealth Carrier Engine — Conceal any file inside Images, Audio, Video, PDFs & Executables with zero size limits.",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Conceal a secret payload inside a host carrier (Image, Audio, Video, PDF, EXE)
    Embed {
        /// Path to the host carrier file (e.g. cover.jpg, song.mp3, clip.mp4, doc.pdf, app.exe)
        #[arg(short = 'c', long)]
        carrier: PathBuf,

        /// Path to the secret file payload to conceal
        #[arg(short = 'p', long)]
        payload: PathBuf,

        /// Path for the output carrier file
        #[arg(short = 'o', long)]
        output: PathBuf,

        /// Optional password to encrypt the payload using ChaCha20-Poly1305 AEAD + Argon2id
        #[arg(short = 'w', long)]
        password: Option<String>,
    },

    /// Extract and restore the concealed payload from a carrier
    Extract {
        /// Path to the carrier file
        #[arg(short = 'c', long)]
        carrier: PathBuf,

        /// Destination path for the extracted file (optional: defaults to original filename)
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,

        /// Password if the carrier is encrypted
        #[arg(short = 'w', long)]
        password: Option<String>,
    },

    /// Inspect internal details and metadata of a carrier
    Inspect {
        /// Path to the carrier file
        #[arg(short = 'c', long)]
        carrier: PathBuf,
    },

    /// Remove the concealed payload and restore the pristine original host file
    Strip {
        /// Path to the carrier file
        #[arg(short = 'c', long)]
        carrier: PathBuf,

        /// Output path for the clean host file
        #[arg(short = 'o', long)]
        output: PathBuf,
    },
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Embed {
            carrier,
            payload,
            output,
            password,
        } => {
            println!("==> STOW: Concealing Payload into Carrier");
            println!("  Host Carrier : {}", carrier.display());
            println!("  Secret File  : {}", payload.display());
            println!("  Output File  : {}", output.display());
            if password.is_some() {
                println!("  Protection   : ChaCha20-Poly1305 AEAD Encrypted");
            } else {
                println!("  Protection   : Unencrypted");
            }

            let pb = Arc::new(ProgressBar::new(100));
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}% ({msg})")
                    .unwrap()
                    .progress_chars("#>-"),
            );

            let pb_clone = pb.clone();
            let progress_cb = Box::new(move |up: ProgressUpdate| {
                pb_clone.set_position(up.percentage as u64);
                pb_clone.set_message(format!("{}: {}", up.phase, format_bytes(up.bytes_processed)));
            });

            let report = embed_files(
                &carrier,
                &payload,
                &output,
                EmbedOptions { password },
                Some(progress_cb),
            )?;

            pb.finish_with_message("Done!");
            println!("\n[OK] Operation Complete:");
            println!("  Host Size      : {}", format_bytes(report.host_image_size));
            println!("  Payload Size   : {}", format_bytes(report.payload_size));
            println!("  Total Carrier  : {}", format_bytes(report.total_carrier_size));
            println!("  BLAKE3 Checksum: {}", report.blake3_hex);
            println!("  Elapsed Time   : {:.2}s", report.elapsed_millis as f64 / 1000.0);
        }

        Commands::Extract {
            carrier,
            output,
            password,
        } => {
            println!("==> STOW: Extracting Payload from Carrier");
            println!("  Carrier File : {}", carrier.display());

            let pb = Arc::new(ProgressBar::new(100));
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}% ({msg})")
                    .unwrap()
                    .progress_chars("#>-"),
            );

            let pb_clone = pb.clone();
            let progress_cb = Box::new(move |up: ProgressUpdate| {
                pb_clone.set_position(up.percentage as u64);
                pb_clone.set_message(format!("{}: {}", up.phase, format_bytes(up.bytes_processed)));
            });

            let report = extract_payload(
                &carrier,
                output.as_ref(),
                password.as_deref(),
                Some(progress_cb),
            )?;

            pb.finish_with_message("Done!");
            println!("\n[OK] Extraction Complete:");
            println!("  Extracted File : {}", report.output_path.display());
            println!("  File Size      : {}", format_bytes(report.file_size));
            println!("  BLAKE3 Checksum: {}", report.blake3_hex);
            println!("  Elapsed Time   : {:.2}s", report.elapsed_millis as f64 / 1000.0);
        }

        Commands::Inspect { carrier } => {
            println!("==> STOW: Inspecting Carrier");
            if !has_carrier_payload(&carrier) {
                println!("  [!] No concealed payload found in {}", carrier.display());
                return Ok(());
            }

            let (_trailer, meta) = inspect_carrier(&carrier)?;
            println!("  [✓] Concealed Payload Found:");
            println!("  Original Filename : {}", meta.original_filename);
            println!("  MIME Type         : {}", meta.mime_type);
            println!("  Payload Size      : {}", format_bytes(meta.original_file_size));
            println!("  Host Carrier Type : {}", meta.host_format);
            println!("  BLAKE3 Checksum   : {}", meta.blake3_hex);
            println!("  Password Protected: {}", if meta.is_encrypted { "Yes" } else { "No" });
        }

        Commands::Strip { carrier, output } => {
            println!("==> STOW: Stripping Concealed Payload");
            println!("  Carrier File : {}", carrier.display());
            println!("  Clean Output : {}", output.display());

            let report = strip_payload_to_file(&carrier, &output)?;
            println!("\n[OK] Clean Complete:");
            println!("  Original Host Size : {}", format_bytes(report.original_host_image_size));
            println!("  Payload Removed    : {}", format_bytes(report.payload_bytes_removed));
        }
    }

    Ok(())
}
