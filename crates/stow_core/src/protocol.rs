use crate::error::{Result, StowError};
use byteorder::{BigEndian, ByteOrder};
use crc32fast::Hasher as Crc32Hasher;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const TRAILER_MAGIC_STOW: &[u8; 16] = b"STOW_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00";
pub const TRAILER_MAGIC_LEGACY: &[u8; 16] = b"SECRETPNG_V1\x00\x00\x00\x00";
pub const TRAILER_MAGIC: &[u8; 16] = TRAILER_MAGIC_STOW;
pub const TRAILER_TERMINATOR: [u8; 4] = [0x55, 0xAA, 0x55, 0xAA];
pub const TRAILER_SIZE: usize = 64;

/// Ultra-high throughput streaming buffer (1 MB)
pub const IO_BUFFER_SIZE: usize = 1024 * 1024;
pub const DEFAULT_CHUNK_SIZE: usize = IO_BUFFER_SIZE;

/// Maximum sanity limit for JSON metadata block (10 MB)
pub const MAX_METADATA_SIZE: u32 = 10 * 1024 * 1024;

/// Classification of host carrier file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HostCategory {
    Image,
    Audio,
    Video,
    Document,
    Executable,
    Other,
}

impl HostCategory {
    pub fn icon(&self) -> &'static str {
        match self {
            HostCategory::Image => "🖼️ Image",
            HostCategory::Audio => "🎵 Audio",
            HostCategory::Video => "🎬 Video",
            HostCategory::Document => "📄 Document",
            HostCategory::Executable => "⚙️ Executable",
            HostCategory::Other => "📁 File",
        }
    }
}

/// Encryption metadata attached when payload is encrypted with a password
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptionMetadata {
    /// Hex-encoded 16-byte random salt for Argon2id KDF
    pub salt_hex: String,
    /// Hex-encoded 12-byte random base nonce for ChaCha20-Poly1305
    pub nonce_hex: String,
    /// AEAD chunk size in bytes (e.g., 1048576)
    pub chunk_size: u32,
    /// Cipher algorithm identifier
    pub cipher: String,
}

/// Metadata stored in the carrier describing the embedded payload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayloadMetadata {
    pub protocol_version: u16,
    pub original_filename: String,
    pub file_extension: String,
    pub mime_type: String,
    pub original_file_size: u64,
    pub payload_size: u64,
    pub blake3_hex: String,
    pub crc32: u32,
    pub timestamp_epoch_sec: u64,
    pub is_encrypted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<EncryptionMetadata>,

    // Carrier properties
    #[serde(default)]
    pub host_category: Option<HostCategory>,
    pub host_format: String,
    #[serde(default)]
    pub host_image_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_image_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_image_height: Option<u32>,
}

/// Fixed 64-byte trailing index structure at exact EOF - 64
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailerIndex {
    pub version: u16,
    pub flags: u16,
    pub host_image_size: u64,
    pub payload_offset: u64,
    pub payload_length: u64,
    pub metadata_offset: u64,
    pub metadata_length: u32,
    pub metadata_crc32: u32,
}

impl TrailerIndex {
    pub const FLAG_ENCRYPTED: u16 = 0x0001;

    /// Serialize into exact 64-byte array
    pub fn to_bytes(&self) -> [u8; TRAILER_SIZE] {
        let mut buf = [0u8; TRAILER_SIZE];

        // 0..16: Magic (STOW_V1)
        buf[0..16].copy_from_slice(TRAILER_MAGIC_STOW);
        // 16..18: Version
        BigEndian::write_u16(&mut buf[16..18], self.version);
        // 18..20: Flags
        BigEndian::write_u16(&mut buf[18..20], self.flags);
        // 20..28: Host image size / payload offset (u64)
        BigEndian::write_u64(&mut buf[20..28], self.host_image_size);
        // 28..36: Payload length (u64)
        BigEndian::write_u64(&mut buf[28..36], self.payload_length);
        // 36..44: Metadata offset (u64)
        BigEndian::write_u64(&mut buf[36..44], self.metadata_offset);
        // 44..48: Metadata length (u32)
        BigEndian::write_u32(&mut buf[44..48], self.metadata_length);
        // 48..52: Metadata CRC32 (u32)
        BigEndian::write_u32(&mut buf[48..52], self.metadata_crc32);
        // 52..56: Reserved (zero padding)
        buf[52..56].fill(0);

        // 56..60: CRC32 of first 56 bytes
        let mut hasher = Crc32Hasher::new();
        hasher.update(&buf[0..56]);
        let trailer_crc = hasher.finalize();
        BigEndian::write_u32(&mut buf[56..60], trailer_crc);

        // 60..64: Terminator
        buf[60..64].copy_from_slice(&TRAILER_TERMINATOR);

        buf
    }

    /// Parse and validate from 64-byte slice
    pub fn from_bytes(buf: &[u8]) -> Result<Self> {
        if buf.len() != TRAILER_SIZE {
            return Err(StowError::CorruptedTrailer);
        }

        // Validate Terminator
        if buf[60..64] != TRAILER_TERMINATOR {
            return Err(StowError::NoCarrierDataFound);
        }

        // Validate Magic (Support both STOW_V1 and SECRETPNG_V1)
        let magic = &buf[0..16];
        if magic != TRAILER_MAGIC_STOW && magic != TRAILER_MAGIC_LEGACY {
            return Err(StowError::NoCarrierDataFound);
        }

        // Validate CRC32 of trailer
        let mut hasher = Crc32Hasher::new();
        hasher.update(&buf[0..56]);
        let expected_crc = hasher.finalize();
        let stored_crc = BigEndian::read_u32(&buf[56..60]);
        if expected_crc != stored_crc {
            return Err(StowError::CorruptedTrailer);
        }

        let version = BigEndian::read_u16(&buf[16..18]);
        if version > PROTOCOL_VERSION {
            return Err(StowError::UnsupportedVersion(version, PROTOCOL_VERSION));
        }

        let flags = BigEndian::read_u16(&buf[18..20]);
        let host_image_size = BigEndian::read_u64(&buf[20..28]);
        let payload_length = BigEndian::read_u64(&buf[28..36]);
        let metadata_offset = BigEndian::read_u64(&buf[36..44]);
        let metadata_length = BigEndian::read_u32(&buf[44..48]);
        let metadata_crc32 = BigEndian::read_u32(&buf[48..52]);

        Ok(Self {
            version,
            flags,
            host_image_size,
            payload_offset: host_image_size,
            payload_length,
            metadata_offset,
            metadata_length,
            metadata_crc32,
        })
    }
}
