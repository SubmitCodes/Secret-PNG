use crate::crypto::StreamEncryptor;
use crate::error::{Result, StowError};
use crate::protocol::{
    HostCategory, PayloadMetadata, TrailerIndex, IO_BUFFER_SIZE, PROTOCOL_VERSION,
};
use crc32fast::Hasher as Crc32Hasher;
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
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "tar" | "gz" => "application/gzip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/x-rar-compressed",
        "exe" => "application/vnd.microsoft.portable-executable",
        "iso" => "application/x-iso9660-image",
        "sqlite" | "db" => "application/x-sqlite3",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

/// Classify carrier file based on extension and content
pub fn classify_host_carrier<P: AsRef<Path>>(path: P) -> (HostCategory, String, Option<u32>, Option<u32>) {
    let path = path.as_ref();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tiff" | "ico" => {
            let dims = if let Ok(file) = File::open(path) {
                let reader = BufReader::new(file);
                if let Ok(img_reader) = image::ImageReader::new(reader).with_guessed_format() {
                    img_reader.into_dimensions().ok()
                } else {
                    None
                }
            } else {
                None
            };
            (
                HostCategory::Image,
                ext.to_uppercase(),
                dims.map(|(w, _)| w),
                dims.map(|(_, h)| h),
            )
        }
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" => {
            (HostCategory::Audio, ext.to_uppercase(), None, None)
        }
        "mp4" | "mkv" | "mov" | "webm" | "avi" | "wmv" | "flv" | "m4v" | "ts" | "3gp" => {
            (HostCategory::Video, ext.to_uppercase(), None, None)
        }
        "pdf" => {
            (HostCategory::Document, "PDF".to_string(), None, None)
        }
        "exe" | "dll" | "iso" | "bin" => {
            (HostCategory::Executable, ext.to_uppercase(), None, None)
        }
        _ => {
            (HostCategory::Other, ext.to_uppercase(), None, None)
        }
    }
}

pub fn inspect_image_header<P: AsRef<Path>>(path: P) -> Result<(String, Option<u32>, Option<u32>)> {
    let (_, fmt, w, h) = classify_host_carrier(path);
    Ok((fmt, w, h))
}

/// Stream host carrier and payload into carrier output file with zero high-RAM allocations.
/// Works for Images, Audio, Video, Documents (PDF), and Executables seamlessly.
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

    // 1. Inspect host carrier format & dimensions
    let (host_cat, host_fmt, host_w, host_h) = classify_host_carrier(host_path);
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
        ".stow_tmp_{}_{}.tmp",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));

    let embed_result = (|| -> Result<(u64, u64, String, u32, bool)> {
        let out_file = File::create(&temp_output_path)?;
        let mut out_writer = BufWriter::with_capacity(IO_BUFFER_SIZE, out_file);

        let mut host_written = 0u64;

        // 2. Stream host carrier in raw high-speed 1MB chunks (Preserves 100% of Audio, Video, PDF, Image, EXE)
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

            if let Some(ref cb) = progress {
                let now = Instant::now();
                if now.duration_since(last_progress_time).as_millis() >= 50 {
                    let elapsed = now.duration_since(last_progress_time).as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (total_processed_bytes - last_progress_bytes) as f64 / elapsed
                    } else {
                        0.0
                    };
                    cb(ProgressUpdate {
                        phase: format!("Streaming Host Carrier ({})", host_fmt),
                        bytes_processed: total_processed_bytes,
                        total_bytes: total_expected_bytes,
                        speed_bytes_sec: speed,
                        percentage: (total_processed_bytes as f32 / total_expected_bytes as f32) * 100.0,
                    });
                    last_progress_time = now;
                    last_progress_bytes = total_processed_bytes;
                }
            }
        }

        // 3. Stream payload with BLAKE3 & CRC32 calculation + Optional ChaCha20-Poly1305 AEAD
        let payload_file = File::open(payload_path)?;
        let mut payload_reader = BufReader::with_capacity(IO_BUFFER_SIZE, payload_file);

        let mut blake3_hasher = blake3::Hasher::new();
        let mut crc_hasher = Crc32Hasher::new();

        let mut encryptor = if let Some(ref pass) = options.password {
            Some(StreamEncryptor::new(pass)?)
        } else {
            None
        };

        let mut payload_written = 0u64;

        while payload_written < payload_raw_size {
            let bytes_to_read = std::cmp::min(buffer.len() as u64, payload_raw_size - payload_written) as usize;
            let n = payload_reader.read(&mut buffer[..bytes_to_read])?;
            if n == 0 {
                break;
            }

            let chunk = &buffer[..n];
            blake3_hasher.update(chunk);
            crc_hasher.update(chunk);

            if let Some(ref mut enc) = encryptor {
                let bytes_enc = enc.encrypt_chunk(chunk, &mut out_writer)?;
                payload_written += bytes_enc as u64;
            } else {
                out_writer.write_all(chunk)?;
                payload_written += n as u64;
            }

            total_processed_bytes += n as u64;

            if let Some(ref cb) = progress {
                let now = Instant::now();
                if now.duration_since(last_progress_time).as_millis() >= 50 {
                    let elapsed = now.duration_since(last_progress_time).as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (total_processed_bytes - last_progress_bytes) as f64 / elapsed
                    } else {
                        0.0
                    };
                    cb(ProgressUpdate {
                        phase: if encryptor.is_some() {
                            "Encrypting & Concealing Payload".into()
                        } else {
                            "Concealing Payload".into()
                        },
                        bytes_processed: total_processed_bytes,
                        total_bytes: total_expected_bytes,
                        speed_bytes_sec: speed,
                        percentage: (total_processed_bytes as f32 / total_expected_bytes as f32) * 100.0,
                    });
                    last_progress_time = now;
                    last_progress_bytes = total_processed_bytes;
                }
            }
        }

        let blake3_hex = blake3_hasher.finalize().to_hex().to_string();
        let crc32 = crc_hasher.finalize();
        let is_encrypted = encryptor.is_some();

        // 4. Build and serialize JSON Metadata
        let enc_metadata = encryptor.map(|enc| enc.metadata);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let metadata = PayloadMetadata {
            protocol_version: PROTOCOL_VERSION,
            original_filename: original_filename.clone(),
            file_extension,
            mime_type,
            original_file_size: payload_raw_size,
            payload_size: payload_written,
            blake3_hex: blake3_hex.clone(),
            crc32,
            timestamp_epoch_sec: timestamp,
            is_encrypted,
            encryption: enc_metadata,
            host_category: Some(host_cat),
            host_format: host_fmt.clone(),
            host_image_format: host_fmt,
            host_image_width: host_w,
            host_image_height: host_h,
        };

        let meta_json_bytes = serde_json::to_vec(&metadata)
            .map_err(|e| StowError::CorruptedMetadata(format!("Serialization error: {}", e)))?;

        let mut meta_crc_hasher = Crc32Hasher::new();
        meta_crc_hasher.update(&meta_json_bytes);
        let meta_crc32 = meta_crc_hasher.finalize();

        let metadata_offset = host_written + payload_written;
        let metadata_length = meta_json_bytes.len() as u32;

        out_writer.write_all(&meta_json_bytes)?;

        // 5. Append Fixed 64-byte Trailer Index
        let trailer = TrailerIndex {
            version: PROTOCOL_VERSION,
            flags: if is_encrypted { TrailerIndex::FLAG_ENCRYPTED } else { 0 },
            host_image_size: host_written,
            payload_offset: host_written,
            payload_length: payload_written,
            metadata_offset,
            metadata_length,
            metadata_crc32: meta_crc32,
        };

        let trailer_bytes = trailer.to_bytes();
        out_writer.write_all(&trailer_bytes)?;
        out_writer.flush()?;

        Ok((host_written, payload_written, blake3_hex, crc32, is_encrypted))
    })();

    match embed_result {
        Ok((host_sz, payload_sz, blake3_h, crc, is_enc)) => {
            if output_path.exists() {
                std::fs::remove_file(output_path)?;
            }
            std::fs::rename(&temp_output_path, output_path)?;

            let total_carrier_size = std::fs::metadata(output_path)?.len();
            let elapsed_millis = start_time.elapsed().as_millis();

            if let Some(cb) = progress {
                cb(ProgressUpdate {
                    phase: "Complete".into(),
                    bytes_processed: total_expected_bytes,
                    total_bytes: total_expected_bytes,
                    speed_bytes_sec: 0.0,
                    percentage: 100.0,
                });
            }

            Ok(EmbedReport {
                host_image_size: host_sz,
                payload_size: payload_sz,
                total_carrier_size,
                original_file_name: original_filename,
                blake3_hex: blake3_h,
                crc32: crc,
                is_encrypted: is_enc,
                elapsed_millis,
            })
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temp_output_path);
            Err(e)
        }
    }
}
