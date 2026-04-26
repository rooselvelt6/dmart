#![allow(dead_code)]

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const NONCE_SIZE: usize = 12;
pub const KEY_SIZE: usize = 32;
pub const SALT_SIZE: usize = 16;
pub const IV_SIZE: usize = 16;

pub const ENCRYPTED_MAGIC: &[u8] = b"DMART_V1";

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Invalid key")]
    InvalidKey,
    #[error("Invalid data format")]
    InvalidFormat,
    #[error("Key management error")]
    KeyManagementError,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; KEY_SIZE]);

impl MasterKey {
    pub fn new() -> Self {
        let mut key = [0u8; KEY_SIZE];
        OsRng.fill_bytes(&mut key);
        Self(key)
    }

    pub fn from_password(password: &str) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(b"dmart-uci-key-v1");
        let result = hasher.finalize();
        let mut key = [0u8; KEY_SIZE];
        key.copy_from_slice(&result[..KEY_SIZE.min(result.len())]);
        Self(key)
    }

    pub fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.0
    }
}

impl Default for MasterKey {
    fn default() -> Self {
        Self::from_password("dmart-default-key-change-me")
    }
}

pub struct CryptoService {
    master_key: MasterKey,
    chacha: ChaCha20Poly1305,
}

impl CryptoService {
    pub fn new(key: MasterKey) -> Self {
        let chacha = ChaCha20Poly1305::new_from_slice(key.as_bytes())
            .expect("Valid key for ChaCha20");
        Self { master_key: key, chacha }
    }

    pub fn new_from_password(password: &str) -> Self {
        Self::new(MasterKey::from_password(password))
    }

    pub fn new_software_hsm() -> Self {
        let key = MasterKey::new();
        Self::new(key)
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .chacha
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        let mut result = Vec::with_capacity(
            ENCRYPTED_MAGIC.len() + NONCE_SIZE + ciphertext.len(),
        );
        result.extend_from_slice(ENCRYPTED_MAGIC);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if encrypted.len() < ENCRYPTED_MAGIC.len() + NONCE_SIZE {
            return Err(CryptoError::InvalidFormat);
        }

        let magic = &encrypted[..ENCRYPTED_MAGIC.len()];
        if magic != ENCRYPTED_MAGIC {
            return Err(CryptoError::InvalidFormat);
        }

        let nonce_bytes = &encrypted[ENCRYPTED_MAGIC.len()..ENCRYPTED_MAGIC.len() + NONCE_SIZE];
        let ciphertext = &encrypted[ENCRYPTED_MAGIC.len() + NONCE_SIZE..];

        let nonce = Nonce::from_slice(nonce_bytes);

        self.chacha
            .decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::DecryptionFailed)
    }

    pub fn encrypt_str(&self, plaintext: &str) -> Result<String, CryptoError> {
        let encrypted = self.encrypt(plaintext.as_bytes())?;
        Ok(base64_encode(&encrypted))
    }

    pub fn decrypt_str(&self, encrypted: &str) -> Result<String, CryptoError> {
        let data = base64_decode(encrypted).map_err(|_| CryptoError::InvalidFormat)?;
        let decrypted = self.decrypt(&data)?;
        String::from_utf8(decrypted).map_err(|_| CryptoError::DecryptionFailed)
    }

    pub fn encrypt_json<T: serde::Serialize>(&self, data: &T) -> Result<String, CryptoError> {
        let json = serde_json::to_vec(data).map_err(|_| CryptoError::EncryptionFailed)?;
        self.encrypt_str(&String::from_utf8_lossy(&json))
    }

    pub fn decrypt_json<T: serde::de::DeserializeOwned>(&self, encrypted: &str) -> Result<T, CryptoError> {
        let decrypted = self.decrypt_str(encrypted)?;
        serde_json::from_str(&decrypted).map_err(|_| CryptoError::DecryptionFailed)
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    BASE64.encode(data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    BASE64.decode(data)
}

pub fn encrypt_data(plaintext: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
    let service = CryptoService::new_from_password(password);
    service.encrypt(plaintext)
}

pub fn decrypt_data(encrypted: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
    let service = CryptoService::new_from_password(password);
    service.decrypt(encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chacha_encrypt_decrypt() {
        let service = CryptoService::new_software_hsm();
        let plaintext = b"Hola mundo secreto";

        let encrypted = service.encrypt(plaintext).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_string_encryption() {
        let service = CryptoService::new_software_hsm();
        let plaintext = "Datos sensibles del paciente";

        let encrypted = service.encrypt_str(plaintext).unwrap();
        let decrypted = service.decrypt_str(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }
}