pub mod crypto;
pub mod embedder;
pub mod error;
pub mod extractor;
pub mod protocol;
pub mod sanitizer;

pub use embedder::{
    classify_host_carrier, embed_files, infer_mime_type, inspect_image_header, EmbedOptions,
    EmbedReport, ProgressCallback, ProgressUpdate,
};
pub use error::{Result, SecretPngError, StowError};
pub use extractor::{
    extract_payload, has_carrier_payload, inspect_carrier, ExtractionReport,
};
pub use protocol::{
    EncryptionMetadata, HostCategory, PayloadMetadata, TrailerIndex, DEFAULT_CHUNK_SIZE, MAX_METADATA_SIZE,
    PROTOCOL_VERSION, TRAILER_MAGIC, TRAILER_MAGIC_LEGACY, TRAILER_MAGIC_STOW, TRAILER_SIZE,
};
pub use sanitizer::{strip_payload_in_place, strip_payload_to_file, SanitizeReport};
