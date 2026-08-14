use crate::error::{Result, SecretPngError};
use crate::protocol::{EncryptionMetadata, DEFAULT_CHUNK_SIZE};
use argon2::Argon2;
use byteorder::{BigEndian, ByteOrder};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use std::io::{Read, Write};

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const CIPHER_NAME: &str = "ChaCha20-Poly1305-Argon2id";

pub struct StreamEncryptor {
    cipher: ChaCha20Poly1305,
    base_nonce: [u8; NONCE_LEN],
    chunk_index: u64,
    pub metadata: EncryptionMetadata,
}

impl StreamEncryptor {
    pub fn new(password: &str) -> Result<Self> {
        let mut salt = [0u8; SALT_LEN];
        let mut base_nonce = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        rand::thread_rng().fill_bytes(&mut base_nonce);

        let mut key_bytes = [0u8; 32];
        let argon2 = Argon2::default();
        argon2
            .hash_password_into(password.as_bytes(), &salt, &mut key_bytes)
            .map_err(|e| SecretPngError::InvalidParameter(format!("Argon2 key derivation error: {}", e)))?;

        let key = Key::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);

        let metadata = EncryptionMetadata {
            salt_hex: hex::encode(salt),
            nonce_hex: hex::encode(base_nonce),
            chunk_size: DEFAULT_CHUNK_SIZE as u32,
            cipher: CIPHER_NAME.to_string(),
        };

        Ok(Self {
            cipher,
            base_nonce,
            chunk_index: 0,
            metadata,
        })
    }

    /// Encrypt next chunk and write [4-byte len | ciphertext + tag] to writer
    pub fn encrypt_chunk<W: Write>(&mut self, plaintext: &[u8], writer: &mut W) -> Result<usize> {
        let mut nonce_bytes = self.base_nonce;
        // Nonce is 12 bytes: first 4 bytes base, last 8 bytes chunk index
        BigEndian::write_u64(&mut nonce_bytes[4..12], self.chunk_index);
        self.chunk_index += 1;

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| SecretPngError::DecryptionFailed)?;

        let mut len_buf = [0u8; 4];
        BigEndian::write_u32(&mut len_buf, ciphertext.len() as u32);
        writer.write_all(&len_buf)?;
        writer.write_all(&ciphertext)?;

        Ok(4 + ciphertext.len())
    }
}

pub struct StreamDecryptor {
    cipher: ChaCha20Poly1305,
    base_nonce: [u8; NONCE_LEN],
    chunk_index: u64,
}

impl StreamDecryptor {
    pub fn new(password: &str, metadata: &EncryptionMetadata) -> Result<Self> {
        let salt = hex::decode(&metadata.salt_hex)
            .map_err(|_| SecretPngError::CorruptedMetadata("Invalid salt hex in metadata".into()))?;
        let base_nonce_vec = hex::decode(&metadata.nonce_hex)
            .map_err(|_| SecretPngError::CorruptedMetadata("Invalid nonce hex in metadata".into()))?;

        if salt.len() != SALT_LEN || base_nonce_vec.len() != NONCE_LEN {
            return Err(SecretPngError::CorruptedMetadata("Invalid salt/nonce length".into()));
        }

        let mut base_nonce = [0u8; NONCE_LEN];
        base_nonce.copy_from_slice(&base_nonce_vec);

        let mut key_bytes = [0u8; 32];
        let argon2 = Argon2::default();
        argon2
            .hash_password_into(password.as_bytes(), &salt, &mut key_bytes)
            .map_err(|e| SecretPngError::InvalidParameter(format!("Argon2 key derivation error: {}", e)))?;

        let key = Key::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);

        Ok(Self {
            cipher,
            base_nonce,
            chunk_index: 0,
        })
    }

    /// Read next encrypted frame [4-byte len | ciphertext] from reader and decrypt to plaintext
    pub fn decrypt_chunk<R: Read>(&mut self, reader: &mut R) -> Result<Option<Vec<u8>>> {
        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(SecretPngError::Io(e)),
        }

        let chunk_len = BigEndian::read_u32(&len_buf) as usize;
        // Limit max chunk length for safety (chunk_size + 16-byte tag + reasonable headroom)
        if chunk_len > (DEFAULT_CHUNK_SIZE * 2) {
            return Err(SecretPngError::CorruptedMetadata("Corrupted encrypted chunk length".into()));
        }

        let mut ciphertext = vec![0u8; chunk_len];
        reader.read_exact(&mut ciphertext)?;

        let mut nonce_bytes = self.base_nonce;
        BigEndian::write_u64(&mut nonce_bytes[4..12], self.chunk_index);
        self.chunk_index += 1;

        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| SecretPngError::DecryptionFailed)?;

        Ok(Some(plaintext))
    }
}

// Simple hex helper module
mod hex {
    pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
        let bytes = data.as_ref();
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push_str(&format!("{:02x}", b));
        }
        s
    }

    pub fn decode(hex_str: &str) -> std::result::Result<Vec<u8>, ()> {
        if hex_str.len() % 2 != 0 {
            return Err(());
        }
        let mut bytes = Vec::with_capacity(hex_str.len() / 2);
        for i in (0..hex_str.len()).step_by(2) {
            let byte = u8::from_str_radix(&hex_str[i..i + 2], 16).map_err(|_| ())?;
            bytes.push(byte);
        }
        Ok(bytes)
    }
}
