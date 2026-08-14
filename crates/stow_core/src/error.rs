use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StowError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid host cover image: {0}")]
    InvalidHostImage(String),

    #[error("No concealed carrier data found in the image")]
    NoCarrierDataFound,

    #[error("Corrupted carrier trailer header")]
    CorruptedTrailer,

    #[error("Corrupted metadata block: {0}")]
    CorruptedMetadata(String),

    #[error("Carrier payload is password protected, but no password was provided")]
    PasswordRequired,

    #[error("Decryption failed. Invalid password or corrupted payload")]
    DecryptionFailed,

    #[error("Payload checksum verification failed! Expected {expected}, got {calculated}")]
    ChecksumMismatch {
        expected: String,
        calculated: String,
    },

    #[error("Unsupported protocol version: {0} (engine supports {1})")]
    UnsupportedVersion(u16, u16),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Operation cancelled by user")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, StowError>;
pub type SecretPngError = StowError;
