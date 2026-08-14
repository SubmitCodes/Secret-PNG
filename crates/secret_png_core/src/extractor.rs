use crate::crypto::StreamDecryptor;
use crate::embedder::{ProgressCallback, ProgressUpdate};
use crate::error::{Result, SecretPngError};
use crate::protocol::{
    PayloadMetadata, TrailerIndex, IO_BUFFER_SIZE, PNG_IEND_CHUNK, PNG_SECR_TYPE, TRAILER_SIZE,
};
use byteorder::{BigEndian, ByteOrder};
use crc32fast::Hasher as Crc32Hasher;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

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

/// Instant O(1) metadata inspection without scanning the carrier file
pub fn inspect_carrier<P: AsRef<Path>>(carrier_path: P) -> Result<(TrailerIndex, PayloadMetadata)> {
    let carrier_path = carrier_path.as_ref();
    let mut file = File::open(carrier_path)?;
    let total_len = file.metadata()?.len();

    if total_len < (TRAILER_SIZE + 12) as u64 {
        return Err(SecretPngError::NoCarrierDataFound);
    }

    let mut trailer_buf = [0u8; TRAILER_SIZE];

    // 1. Check if file ends with PNG IEND chunk (PNG chunk mode)
    let mut ends_with_iend = false;
    if total_len >= 88 {
        file.seek(SeekFrom::End(-12))?;
        let mut iend_check = [0u8; 12];
        if file.read_exact(&mut iend_check).is_ok() && iend_check == PNG_IEND_CHUNK {
            ends_with_iend = true;
        }
    }

    let trailer_res = if ends_with_iend {
        // Trailer is at EOF - 12 (IEND) - 4 (chunk CRC) - 64 (trailer data) = EOF - 80
        file.seek(SeekFrom::End(-80))?;
        file.read_exact(&mut trailer_buf)?;
        TrailerIndex::from_bytes(&trailer_buf)
    } else {
        // Standard EOF trailer mode (JPEG, etc.)
        file.seek(SeekFrom::End(-(TRAILER_SIZE as i64)))?;
        file.read_exact(&mut trailer_buf)?;
        TrailerIndex::from_bytes(&trailer_buf)
    };

    let trailer = match trailer_res {
        Ok(t) => t,
        Err(_) if ends_with_iend => {
            // Fallback to EOF - 64
            file.seek(SeekFrom::End(-(TRAILER_SIZE as i64)))?;
            file.read_exact(&mut trailer_buf)?;
            TrailerIndex::from_bytes(&trailer_buf)?
        }
        Err(e) => return Err(e),
    };

    // 2. Read metadata block
    file.seek(SeekFrom::Start(trailer.metadata_offset))?;
    let mut metadata_buf = vec![0u8; trailer.metadata_length as usize];
    file.read_exact(&mut metadata_buf)?;

    // 3. Verify metadata CRC32
    let mut crc_hasher = Crc32Hasher::new();
    crc_hasher.update(&metadata_buf);
    let calculated_crc = crc_hasher.finalize();
    if calculated_crc != trailer.metadata_crc32 {
        return Err(SecretPngError::CorruptedMetadata("Metadata CRC32 mismatch".into()));
    }

    // 4. Deserialize metadata JSON
    let metadata: PayloadMetadata = serde_json::from_slice(&metadata_buf)
        .map_err(|e| SecretPngError::CorruptedMetadata(format!("JSON parsing error: {}", e)))?;

    Ok((trailer, metadata))
}

/// Check if a file contains a valid Secret PNG carrier payload
pub fn has_carrier_payload<P: AsRef<Path>>(carrier_path: P) -> bool {
    inspect_carrier(carrier_path).is_ok()
}

/// Stream extraction of embedded video with real-time verification and zero-RAM overhead
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
        return Err(SecretPngError::PasswordRequired);
    }

    // 3. Determine output destination path
    let out_path: PathBuf = match output_path_override {
        Some(ref p) => p.as_ref().to_path_buf(),
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

    let out_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&out_path)?;
    let mut out_writer = BufWriter::with_capacity(IO_BUFFER_SIZE, out_file);

    let mut blake3_hasher = blake3::Hasher::new();
    let mut crc_hasher = Crc32Hasher::new();

    let is_png_chunk_mode = (trailer.flags & TrailerIndex::FLAG_PNG_CHUNK) != 0;

    let result = (|| -> Result<()> {
        if metadata.is_encrypted {
            let enc_meta = metadata.encryption.as_ref().ok_or(SecretPngError::CorruptedMetadata(
                "Missing encryption parameters".into(),
            ))?;
            let pass = password.ok_or(SecretPngError::PasswordRequired)?;
            let mut decryptor = StreamDecryptor::new(pass, enc_meta)?;

            if is_png_chunk_mode {
                let mut extracted_payload_bytes = 0u64;
                let mut chunk_header = [0u8; 8];
                let mut crc_buf = [0u8; 4];

                while extracted_payload_bytes < trailer.payload_length {
                    carrier_reader.read_exact(&mut chunk_header)?;
                    let chunk_len = BigEndian::read_u32(&chunk_header[0..4]) as usize;
                    if &chunk_header[4..8] != PNG_SECR_TYPE {
                        return Err(SecretPngError::CorruptedTrailer);
                    }

                    // The chunk data is an encrypted chunk frame [4B frame_len | ciphertext]
                    let mut chunk_data = vec![0u8; chunk_len];
                    carrier_reader.read_exact(&mut chunk_data)?;
                    carrier_reader.read_exact(&mut crc_buf)?;

                    let mut chunk_cursor = std::io::Cursor::new(&chunk_data);
                    if let Some(plaintext) = decryptor.decrypt_chunk(&mut chunk_cursor)? {
                        blake3_hasher.update(&plaintext);
                        crc_hasher.update(&plaintext);
                        out_writer.write_all(&plaintext)?;
                        total_processed_bytes += plaintext.len() as u64;
                    }
                    extracted_payload_bytes += chunk_len as u64;

                    if let Some(ref cb) = progress {
                        if last_progress_time.elapsed().as_millis() >= 60 || total_processed_bytes >= total_expected_bytes {
                            let elapsed_sec = last_progress_time.elapsed().as_secs_f64();
                            let speed = if elapsed_sec > 0.0 {
                                (total_processed_bytes - last_progress_bytes) as f64 / elapsed_sec
                            } else {
                                0.0
                            };
                            last_progress_time = Instant::now();
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
                let mut carrier_take = (&mut carrier_reader).take(trailer.payload_length);
                while let Some(plaintext) = decryptor.decrypt_chunk(&mut carrier_take)? {
                    blake3_hasher.update(&plaintext);
                    crc_hasher.update(&plaintext);
                    out_writer.write_all(&plaintext)?;
                    total_processed_bytes += plaintext.len() as u64;

                    if let Some(ref cb) = progress {
                        if last_progress_time.elapsed().as_millis() >= 60 || total_processed_bytes >= total_expected_bytes {
                            let elapsed_sec = last_progress_time.elapsed().as_secs_f64();
                            let speed = if elapsed_sec > 0.0 {
                                (total_processed_bytes - last_progress_bytes) as f64 / elapsed_sec
                            } else {
                                0.0
                            };
                            last_progress_time = Instant::now();
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
            }
        } else {
            // Unencrypted extraction
            if is_png_chunk_mode {
                let mut extracted_payload_bytes = 0u64;
                let mut chunk_header = [0u8; 8];
                let mut crc_buf = [0u8; 4];
                let mut chunk_buffer = vec![0u8; 128 * 1024];

                while extracted_payload_bytes < trailer.payload_length {
                    carrier_reader.read_exact(&mut chunk_header)?;
                    let chunk_len = BigEndian::read_u32(&chunk_header[0..4]) as usize;
                    if &chunk_header[4..8] != PNG_SECR_TYPE {
                        return Err(SecretPngError::CorruptedTrailer);
                    }

                    if chunk_buffer.len() < chunk_len {
                        chunk_buffer.resize(chunk_len, 0);
                    }
                    carrier_reader.read_exact(&mut chunk_buffer[..chunk_len])?;
                    carrier_reader.read_exact(&mut crc_buf)?;

                    let chunk = &chunk_buffer[..chunk_len];
                    blake3_hasher.update(chunk);
                    crc_hasher.update(chunk);
                    out_writer.write_all(chunk)?;

                    extracted_payload_bytes += chunk_len as u64;
                    total_processed_bytes += chunk_len as u64;

                    if let Some(ref cb) = progress {
                        if last_progress_time.elapsed().as_millis() >= 60 || total_processed_bytes >= total_expected_bytes {
                            let elapsed_sec = last_progress_time.elapsed().as_secs_f64();
                            let speed = if elapsed_sec > 0.0 {
                                (total_processed_bytes - last_progress_bytes) as f64 / elapsed_sec
                            } else {
                                0.0
                            };
                            last_progress_time = Instant::now();
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
            } else {
                let mut buffer = vec![0u8; IO_BUFFER_SIZE];
                let mut remaining = trailer.payload_length;
                while remaining > 0 {
                    let to_read = std::cmp::min(buffer.len() as u64, remaining) as usize;
                    let n = carrier_reader.read(&mut buffer[..to_read])?;
                    if n == 0 {
                        return Err(SecretPngError::CorruptedTrailer);
                    }

                    let chunk = &buffer[..n];
                    blake3_hasher.update(chunk);
                    crc_hasher.update(chunk);
                    out_writer.write_all(chunk)?;

                    remaining -= n as u64;
                    total_processed_bytes += n as u64;

                    if let Some(ref cb) = progress {
                        if last_progress_time.elapsed().as_millis() >= 60 || total_processed_bytes >= total_expected_bytes {
                            let elapsed_sec = last_progress_time.elapsed().as_secs_f64();
                            let speed = if elapsed_sec > 0.0 {
                                (total_processed_bytes - last_progress_bytes) as f64 / elapsed_sec
                            } else {
                                0.0
                            };
                            last_progress_time = Instant::now();
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
        }

        out_writer.flush()?;
        Ok(())
    })();

    if let Err(e) = result {
        let _ = std::fs::remove_file(&out_path);
        return Err(e);
    }

    // 4. Verify checksum
    let calculated_blake3 = blake3_hasher.finalize().to_hex().to_string();
    let calculated_crc = crc_hasher.finalize();

    if calculated_blake3 != metadata.blake3_hex || calculated_crc != metadata.crc32 {
        let _ = std::fs::remove_file(&out_path);
        return Err(SecretPngError::ChecksumMismatch {
            expected: metadata.blake3_hex,
            calculated: calculated_blake3,
        });
    }

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
