use crate::error::{Result, SecretPngError};
use crate::extractor::inspect_carrier;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SanitizeReport {
    pub original_host_image_size: u64,
    pub payload_bytes_removed: u64,
    pub host_image_format: String,
}

/// Strip embedded payload and metadata from carrier image, saving pristine original image to output_path
pub fn strip_payload_to_file<P1: AsRef<Path>, P2: AsRef<Path>>(
    carrier_path: P1,
    output_path: P2,
) -> Result<SanitizeReport> {
    let carrier_path = carrier_path.as_ref();
    let output_path = output_path.as_ref();

    let (trailer, metadata) = inspect_carrier(carrier_path)?;
    let carrier_len = std::fs::metadata(carrier_path)?.len();

    let in_file = File::open(carrier_path)?;
    let mut reader = BufReader::with_capacity(128 * 1024, in_file);

    let out_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)?;
    let mut writer = BufWriter::with_capacity(128 * 1024, out_file);

    let mut remaining = trailer.host_image_size;
    let mut buffer = vec![0u8; 64 * 1024];

    while remaining > 0 {
        let to_read = std::cmp::min(buffer.len() as u64, remaining) as usize;
        let n = reader.read(&mut buffer[..to_read])?;
        if n == 0 {
            return Err(SecretPngError::CorruptedTrailer);
        }
        writer.write_all(&buffer[..n])?;
        remaining -= n as u64;
    }

    writer.flush()?;

    Ok(SanitizeReport {
        original_host_image_size: trailer.host_image_size,
        payload_bytes_removed: carrier_len - trailer.host_image_size,
        host_image_format: metadata.host_image_format,
    })
}

/// Strip embedded payload in-place by truncating file at host_image_size
pub fn strip_payload_in_place<P: AsRef<Path>>(carrier_path: P) -> Result<SanitizeReport> {
    let carrier_path = carrier_path.as_ref();
    let (trailer, metadata) = inspect_carrier(carrier_path)?;
    let carrier_len = std::fs::metadata(carrier_path)?.len();

    let file = OpenOptions::new().write(true).open(carrier_path)?;
    file.set_len(trailer.host_image_size)?;

    Ok(SanitizeReport {
        original_host_image_size: trailer.host_image_size,
        payload_bytes_removed: carrier_len - trailer.host_image_size,
        host_image_format: metadata.host_image_format,
    })
}
