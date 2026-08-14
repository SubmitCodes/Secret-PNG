use crate::crypto::StreamEncryptor;
use crate::error::{Result, SecretPngError};
use crate::protocol::{
    PayloadMetadata, TrailerIndex, IO_BUFFER_SIZE, PROTOCOL_VERSION,
};
use crc32fast::Hasher as Crc32Hasher;
use image::codecs::jpeg::JpegEncoder;
use image::ColorType;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub phase: String,
    pub bytes_processed: u64,
    pub total_bytes: u64,
    pub speed_bytes_sec: f64,
    pub percentage: f32,
}

pub type ProgressCallback = Box<dyn Fn(ProgressUpdate) + Send + Sync>;

#[derive(Debug, Clone, Default)]
pub struct EmbedOptions {
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmbedReport {
    pub host_image_size: u64,
    pub payload_size: u64,
    pub total_carrier_size: u64,
    pub original_file_name: String,
    pub blake3_hex: String,
    pub crc32: u32,
    pub is_encrypted: bool,
    pub elapsed_millis: u128,
}

/// Infer MIME type from file extension
pub fn infer_mime_type(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "flv" => "video/x-flv",
        "wmv" => "video/x-ms-wmv",
        "ts" => "video/mp2t",
        "3gp" => "video/3gpp",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "zip" => "application/zip",
        "tar" | "gz" => "application/gzip",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Validate image format and retrieve dimensions using header sniffing
pub fn inspect_image_header<P: AsRef<Path>>(path: P) -> Result<(String, Option<u32>, Option<u32>)> {
    let file = File::open(path.as_ref())?;
    let reader = BufReader::new(file);
    let img_reader = image::ImageReader::new(reader)
        .with_guessed_format()
        .map_err(|e| SecretPngError::InvalidHostImage(format!("Could not guess image format: {}", e)))?;

    let format = match img_reader.format() {
        Some(fmt) => format!("{:?}", fmt),
        None => return Err(SecretPngError::InvalidHostImage("Unknown or unsupported image format".into())),
    };

    let dimensions = match img_reader.into_dimensions() {
        Ok((w, h)) => (Some(w), Some(h)),
        Err(_) => (None, None),
    };

    Ok((format, dimensions.0, dimensions.1))
}

/// Stream host image and payload video into carrier output file with zero high-RAM allocations.
/// Automatically formats the image stream into universal JPEG carrier structure for unlimited file size viewer compatibility.
pub fn embed_files<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
    host_path: P1,
    payload_path: P2,
    output_path: P3,
    options: EmbedOptions,
    progress: Option<ProgressCallback>,
) -> Result<EmbedReport> {
    let start_time = Instant::now();
    let host_path = host_path.as_ref();
    let payload_path = payload_path.as_ref();
    let output_path = output_path.as_ref();

    // 1. Inspect host image format & dimensions
    let (host_fmt, host_w, host_h) = inspect_image_header(host_path)?;
    let host_file = File::open(host_path)?;
    let host_raw_size = host_file.metadata()?.len();

    let payload_meta = std::fs::metadata(payload_path)?;
    let payload_raw_size = payload_meta.len();

    let total_expected_bytes = host_raw_size + payload_raw_size;
    let mut total_processed_bytes: u64 = 0;
    let mut last_progress_time = Instant::now();
    let mut last_progress_bytes = 0u64;

    let original_filename = payload_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("payload.bin")
        .to_string();

    let file_extension = payload_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_string();

    let mime_type = infer_mime_type(&file_extension).to_string();

    // Use atomic temporary file to prevent truncating host if host == output
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let temp_output_path = parent.join(format!(
        ".secret_png_tmp_{}_{}.tmp",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));

    let embed_result = (|| -> Result<(u64, u64, String, u32, bool)> {
        let out_file = File::create(&temp_output_path)?;
        let mut out_writer = BufWriter::with_capacity(IO_BUFFER_SIZE, out_file);

        let mut host_written = 0u64;

        // 2. Stream host image: if already JPEG, copy directly; otherwise convert to JPEG stream for universal >4GB viewer compatibility
        let is_already_jpeg = host_fmt.to_lowercase().contains("jpeg") || host_fmt.to_lowercase().contains("jpg");
        if is_already_jpeg {
            let mut host_reader = BufReader::with_capacity(IO_BUFFER_SIZE, host_file);
            let mut buffer = vec![0u8; IO_BUFFER_SIZE];
            while host_written < host_raw_size {
                let bytes_to_read = std::cmp::min(buffer.len() as u64, host_raw_size - host_written) as usize;
                let n = host_reader.read(&mut buffer[..bytes_to_read])?;
                if n == 0 {
                    break;
                }
                out_writer.write_all(&buffer[..n])?;
                host_written += n as u64;
                total_processed_bytes += n as u64;
            }
        } else {
            // Load and encode to pristine JPEG stream
            let img = image::open(host_path)
                .map_err(|e| SecretPngError::InvalidHostImage(format!("Could not decode image: {}", e)))?;
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();

            let mut encoder = JpegEncoder::new_with_quality(&mut out_writer, 95);
            encoder
                .encode(rgb.as_raw(), w, h, ColorType::Rgb8.into())
                .map_err(|e| SecretPngError::InvalidHostImage(format!("JPEG encoding failed: {}", e)))?;
            out_writer.flush()?;

            // Get written host size
            host_written = std::fs::metadata(&temp_output_path)?.len();
            total_processed_bytes += host_raw_size;
        }

        let payload_file = File::open(payload_path)?;
        let mut payload_reader = BufReader::with_capacity(IO_BUFFER_SIZE, payload_file);
        let mut buffer = vec![0u8; IO_BUFFER_SIZE];

        // 3. Stream payload with checksum and optional ChaCha20-Poly1305 encryption
        let is_encrypted = options.password.is_some();
        let mut encryptor = if let Some(ref pass) = options.password {
            Some(StreamEncryptor::new(pass)?)
        } else {
            None
        };

        let mut blake3_hasher = blake3::Hasher::new();
        let mut payload_crc_hasher = Crc32Hasher::new();
        let mut payload_stream_size = 0u64;
        let mut payload_raw_read = 0u64;

        while payload_raw_read < payload_raw_size {
            let bytes_to_read = std::cmp::min(buffer.len() as u64, payload_raw_size - payload_raw_read) as usize;
            let n = payload_reader.read(&mut buffer[..bytes_to_read])?;
            if n == 0 {
                break;
            }

            let chunk = &buffer[..n];
            blake3_hasher.update(chunk);
            payload_crc_hasher.update(chunk);
            payload_raw_read += n as u64;
            total_processed_bytes += n as u64;

            if let Some(ref mut enc) = encryptor {
                let written = enc.encrypt_chunk(chunk, &mut out_writer)?;
                payload_stream_size += written as u64;
            } else {
                out_writer.write_all(chunk)?;
                payload_stream_size += n as u64;
            }

            if let Some(ref cb) = progress {
                if last_progress_time.elapsed().as_millis() >= 60 || payload_raw_read >= payload_raw_size {
                    let elapsed_sec = last_progress_time.elapsed().as_secs_f64();
                    let speed = if elapsed_sec > 0.0 {
                        (total_processed_bytes - last_progress_bytes) as f64 / elapsed_sec
                    } else {
                        0.0
                    };
                    last_progress_time = Instant::now();
                    last_progress_bytes = total_processed_bytes;

                    cb(ProgressUpdate {
                        phase: if is_encrypted { "Encrypting & Embedding Video" } else { "Streaming & Embedding Video" }.to_string(),
                        bytes_processed: total_processed_bytes,
                        total_bytes: total_expected_bytes,
                        speed_bytes_sec: speed,
                        percentage: ((total_processed_bytes as f32 / total_expected_bytes as f32) * 100.0).min(99.9),
                    });
                }
            }
        }

        let blake3_final = blake3_hasher.finalize();
        let blake3_hex = blake3_final.to_hex().to_string();
        let crc32_final = payload_crc_hasher.finalize();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // 4. Serialize metadata block
        let metadata = PayloadMetadata {
            protocol_version: PROTOCOL_VERSION,
            original_filename: original_filename.clone(),
            file_extension,
            mime_type,
            original_file_size: payload_raw_size,
            payload_size: payload_stream_size,
            blake3_hex: blake3_hex.clone(),
            crc32: crc32_final,
            timestamp_epoch_sec: timestamp,
            is_encrypted,
            encryption: encryptor.map(|e| e.metadata),
            host_image_format: host_fmt,
            host_image_width: host_w,
            host_image_height: host_h,
        };

        let metadata_json = serde_json::to_vec(&metadata)
            .map_err(|e| SecretPngError::CorruptedMetadata(format!("Serialization error: {}", e)))?;
        let metadata_len = metadata_json.len() as u32;

        let mut meta_crc_hasher = Crc32Hasher::new();
        meta_crc_hasher.update(&metadata_json);
        let metadata_crc32 = meta_crc_hasher.finalize();

        let metadata_offset = host_written + payload_stream_size;
        out_writer.write_all(&metadata_json)?;

        // 5. Build and write Trailer Index (exact 64 bytes at EOF)
        let mut flags = 0u16;
        if is_encrypted {
            flags |= TrailerIndex::FLAG_ENCRYPTED;
        }

        let trailer = TrailerIndex {
            version: PROTOCOL_VERSION,
            flags,
            host_image_size: host_written,
            payload_offset: host_written,
            payload_length: payload_stream_size,
            metadata_offset,
            metadata_length: metadata_len,
            metadata_crc32,
        };

        let trailer_bytes = trailer.to_bytes();
        out_writer.write_all(&trailer_bytes)?;
        out_writer.flush()?;
        drop(out_writer);

        Ok((host_written, payload_raw_size, blake3_hex, crc32_final, is_encrypted))
    })();

    match embed_result {
        Ok((host_written, payload_raw_size, blake3_hex, crc32_final, is_encrypted)) => {
            // Atomically replace target output file
            if output_path.exists() {
                let _ = std::fs::remove_file(output_path);
            }
            std::fs::rename(&temp_output_path, output_path)?;

            let total_carrier_size = std::fs::metadata(output_path)?.len();

            if let Some(ref cb) = progress {
                cb(ProgressUpdate {
                    phase: "Complete".to_string(),
                    bytes_processed: total_expected_bytes,
                    total_bytes: total_expected_bytes,
                    speed_bytes_sec: 0.0,
                    percentage: 100.0,
                });
            }

            Ok(EmbedReport {
                host_image_size: host_written,
                payload_size: payload_raw_size,
                total_carrier_size,
                original_file_name: original_filename,
                blake3_hex,
                crc32: crc32_final,
                is_encrypted,
                elapsed_millis: start_time.elapsed().as_millis(),
            })
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temp_output_path);
            Err(e)
        }
    }
}
