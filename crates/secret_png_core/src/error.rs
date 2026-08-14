use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecretPngError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Host image is invalid or unrecognized format: {0}")]
    InvalidHostImage(String),

    #[error("No secret carrier data found in the image")]
    NoCarrierDataFound,

    #[error("Unsupported carrier protocol version: {0} (max supported: {1})")]
    UnsupportedVersion(u16, u16),

    #[error("Trailer checksum validation failed (file may be truncated or corrupted)")]
    CorruptedTrailer,

    #[error("Corrupted metadata block: {0}")]
    CorruptedMetadata(String),

    #[error("Integrity check failed: payload checksum mismatch (expected: {expected}, calculated: {calculated})")]
    ChecksumMismatch {
        expected: String,
        calculated: String,
    },

    #[error("Decryption failed: incorrect password or corrupted ciphertext")]
    DecryptionFailed,

    #[error("This embedded payload is encrypted with a password, but none was provided")]
    PasswordRequired,

    #[error("Password was provided, but the embedded payload is unencrypted")]
    PasswordNotExpected,

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Image format error: {0}")]
    ImageError(String),
}

pub type Result<T> = std::result::Result<T, SecretPngError>;
