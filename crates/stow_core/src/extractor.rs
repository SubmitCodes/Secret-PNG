use crate::crypto::StreamDecryptor;
use crate::error::{Result, StowError};
use crate::protocol::{
    PayloadMetadata, TrailerIndex, IO_BUFFER_SIZE, MAX_METADATA_SIZE, TRAILER_SIZE,
};
use crc32fast::Hasher as Crc32Hasher;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub type ProgressCallback = Box<dyn Fn(crate::embedder::ProgressUpdate) + Send + Sync>;
pub use crate::embedder::ProgressUpdate;

#[derive(Debug, Clone)]
pub struct ExtractionReport {
    pub output_path: PathBuf,
    pub original_filename: String,
    pub file_size: u64,
    pub blake3_hex: String,
    pub crc32: u32,
    pub is_encrypted: bool,
    pub elapsed_millis: u128,
}

/// Inspect carrier file and read trailer and metadata without extracting
pub fn inspect_carrier<P: AsRef<Path>>(carrier_path: P) -> Result<(TrailerIndex, PayloadMetadata)> {
    let mut file = File::open(carrier_path)?;
    let file_len = file.metadata()?.len();

    if file_len < TRAILER_SIZE as u64 {
        return Err(StowError::NoCarrierDataFound);
    }

    // 1. Read fixed 64-byte trailer from exact EOF - 64
    file.seek(SeekFrom::End(-(TRAILER_SIZE as i64)))?;
    let mut trailer_buf = [0u8; TRAILER_SIZE];
    file.read_exact(&mut trailer_buf)?;

    let trailer = TrailerIndex::from_bytes(&trailer_buf)?;

    // Validate metadata bounds against file size
    if trailer.metadata_length > MAX_METADATA_SIZE {
        return Err(StowError::CorruptedMetadata(format!(
            "Metadata length ({} bytes) exceeds safety ceiling of 10 MB",
            trailer.metadata_length
        )));
    }

    if trailer.metadata_offset + (trailer.metadata_length as u64) > file_len.saturating_sub(TRAILER_SIZE as u64) {
        return Err(StowError::CorruptedTrailer);
    }

    // 2. Read metadata block
    file.seek(SeekFrom::Start(trailer.metadata_offset))?;
    let mut metadata_buf = vec![0u8; trailer.metadata_length as usize];
    file.read_exact(&mut metadata_buf)?;

    // 3. Verify metadata CRC32
    let mut crc_hasher = Crc32Hasher::new();
    crc_hasher.update(&metadata_buf);
    let calculated_crc = crc_hasher.finalize();
    if calculated_crc != trailer.metadata_crc32 {
        return Err(StowError::CorruptedMetadata("Metadata CRC32 mismatch".into()));
    }

    // 4. Deserialize metadata JSON
    let metadata: PayloadMetadata = serde_json::from_slice(&metadata_buf)
        .map_err(|e| StowError::CorruptedMetadata(format!("JSON parsing error: {}", e)))?;

    Ok((trailer, metadata))
}

/// Check if a file contains a valid Stow carrier payload
pub fn has_carrier_payload<P: AsRef<Path>>(carrier_path: P) -> bool {
    inspect_carrier(carrier_path).is_ok()
}

/// Stream extraction of concealed payload with real-time verification and zero-RAM overhead
pub fn extract_payload<P1: AsRef<Path>, P2: AsRef<Path>>(
    carrier_path: P1,
    output_path_override: Option<P2>,
    password: Option<&str>,
    progress: Option<ProgressCallback>,
) -> Result<ExtractionReport> {
    let start_time = Instant::now();
    let carrier_path = carrier_path.as_ref();

    // 1. Inspect trailer and metadata
    let (trailer, metadata) = inspect_carrier(carrier_path)?;

    // 2. Password verification
    if metadata.is_encrypted && password.is_none() {
        return Err(StowError::PasswordRequired);
    }

    // 3. Determine output destination path
    let out_path: PathBuf = match output_path_override {
        Some(ref p) => {
            let p_buf = p.as_ref().to_path_buf();
            // Guard against self-overwrite
            if let (Ok(c_can), Ok(o_can)) = (carrier_path.canonicalize(), p_buf.canonicalize()) {
                if c_can == o_can {
                    return Err(StowError::InvalidParameter(
                        "Destination output path cannot be the same file as the carrier".into(),
                    ));
                }
            }
            p_buf
        }
        None => {
            let parent = carrier_path.parent().unwrap_or_else(|| Path::new("."));
            let mut candidate = parent.join(&metadata.original_filename);
            if candidate == carrier_path {
                let stem = Path::new(&metadata.original_filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("extracted");
                candidate = parent.join(format!("{}_extracted.{}", stem, metadata.file_extension));
            }
            candidate
        }
    };

    let total_expected_bytes = metadata.original_file_size;
    let mut total_processed_bytes: u64 = 0;
    let mut last_progress_time = Instant::now();
    let mut last_progress_bytes = 0u64;

    let carrier_file = File::open(carrier_path)?;
    let mut carrier_reader = BufReader::with_capacity(IO_BUFFER_SIZE, carrier_file);
    carrier_reader.seek(SeekFrom::Start(trailer.payload_offset))?;

    // Use atomic temporary file for safe extraction
    let out_parent = out_path.parent().unwrap_or_else(|| Path::new("."));
    let temp_extract_path = out_parent.join(format!(
        ".stow_extract_tmp_{}_{}.tmp",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));

    let mut blake3_hasher = blake3::Hasher::new();
    let mut crc_hasher = Crc32Hasher::new();

    let result = (|| -> Result<()> {
        let temp_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_extract_path)?;
        let mut out_writer = BufWriter::with_capacity(IO_BUFFER_SIZE, temp_file);

        if metadata.is_encrypted {
            let enc_meta = metadata.encryption.as_ref().ok_or(StowError::CorruptedMetadata(
                "Missing encryption parameters".into(),
            ))?;
            let pass = password.ok_or(StowError::PasswordRequired)?;
            let mut decryptor = StreamDecryptor::new(pass, enc_meta)?;

            let mut carrier_take = (&mut carrier_reader).take(trailer.payload_length);
            while let Some(plaintext) = decryptor.decrypt_chunk(&mut carrier_take)? {
                blake3_hasher.update(&plaintext);
                crc_hasher.update(&plaintext);
                out_writer.write_all(&plaintext)?;
                total_processed_bytes += plaintext.len() as u64;

                if let Some(ref cb) = progress {
                    let now = Instant::now();
                    let elapsed = now.duration_since(last_progress_time).as_secs_f64();
                    if elapsed >= 0.05 || total_processed_bytes >= total_expected_bytes {
                        let speed = if elapsed > 0.0 {
                            (total_processed_bytes - last_progress_bytes) as f64 / elapsed
                        } else {
                            0.0
                        };
                        last_progress_time = now;
                        last_progress_bytes = total_processed_bytes;

                        cb(ProgressUpdate {
                            phase: "Decrypting & Extracting Payload".to_string(),
                            bytes_processed: total_processed_bytes,
                            total_bytes: total_expected_bytes,
                            speed_bytes_sec: speed,
                            percentage: ((total_processed_bytes as f32 / total_expected_bytes as f32) * 100.0).min(99.9),
                        });
                    }
                }
            }
        } else {
            let mut buffer = vec![0u8; IO_BUFFER_SIZE];
            let mut remaining = trailer.payload_length;
            while remaining > 0 {
                let to_read = std::cmp::min(buffer.len() as u64, remaining) as usize;
                let n = carrier_reader.read(&mut buffer[..to_read])?;
                if n == 0 {
                    return Err(StowError::CorruptedTrailer);
                }

                let chunk = &buffer[..n];
                blake3_hasher.update(chunk);
                crc_hasher.update(chunk);
                out_writer.write_all(chunk)?;

                remaining -= n as u64;
                total_processed_bytes += n as u64;

                if let Some(ref cb) = progress {
                    let now = Instant::now();
                    let elapsed = now.duration_since(last_progress_time).as_secs_f64();
                    if elapsed >= 0.05 || total_processed_bytes >= total_expected_bytes {
                        let speed = if elapsed > 0.0 {
                            (total_processed_bytes - last_progress_bytes) as f64 / elapsed
                        } else {
                            0.0
                        };
                        last_progress_time = now;
                        last_progress_bytes = total_processed_bytes;

                        cb(ProgressUpdate {
                            phase: "Streaming & Extracting Payload".to_string(),
                            bytes_processed: total_processed_bytes,
                            total_bytes: total_expected_bytes,
                            speed_bytes_sec: speed,
                            percentage: ((total_processed_bytes as f32 / total_expected_bytes as f32) * 100.0).min(99.9),
                        });
                    }
                }
            }
        }

        out_writer.flush()?;
        Ok(())
    })();

    if let Err(e) = result {
        let _ = std::fs::remove_file(&temp_extract_path);
        return Err(e);
    }

    // 4. Verify checksum
    let calculated_blake3 = blake3_hasher.finalize().to_hex().to_string();
    let calculated_crc = crc_hasher.finalize();

    if calculated_blake3 != metadata.blake3_hex || calculated_crc != metadata.crc32 {
        let _ = std::fs::remove_file(&temp_extract_path);
        return Err(StowError::ChecksumMismatch {
            expected: metadata.blake3_hex,
            calculated: calculated_blake3,
        });
    }

    // Atomic move to final destination
    if out_path.exists() {
        let _ = std::fs::remove_file(&out_path);
    }
    std::fs::rename(&temp_extract_path, &out_path)?;

    if let Some(ref cb) = progress {
        cb(ProgressUpdate {
            phase: "Extraction Complete".to_string(),
            bytes_processed: total_expected_bytes,
            total_bytes: total_expected_bytes,
            speed_bytes_sec: 0.0,
            percentage: 100.0,
        });
    }

    Ok(ExtractionReport {
        output_path: out_path,
        original_filename: metadata.original_filename,
        file_size: metadata.original_file_size,
        blake3_hex: calculated_blake3,
        crc32: calculated_crc,
        is_encrypted: metadata.is_encrypted,
        elapsed_millis: start_time.elapsed().as_millis(),
    })
}
