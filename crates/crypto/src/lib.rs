//! Cryptographic primitives and secure key management for Littmaily.
//!
//! Provides AES-256-GCM encryption/decryption for local data at rest,
//! and interfaces with the OS native keychain to securely store the master key.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use thiserror::Error;

/// Errors originating from cryptographic operations or OS keychain interactions.
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    /// The provided ciphertext was too short to contain a valid nonce.
    #[error("Invalid ciphertext length")]
    InvalidCiphertextLength,
    #[error("Keyring error: {0}")]
    KeyringError(String),
    #[error("Hex decode error: {0}")]
    HexError(String),
}

/// Encrypts `data` using AES-256-GCM with a randomly generated 12-byte nonce.
///
/// The returned vector is formatted as `[12-byte nonce][ciphertext + auth tag]`.
/// Prepending the nonce allows decryption without requiring external nonce storage.
pub fn encrypt_blob(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::EncryptionFailed)?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypts a blob previously encrypted by `encrypt_blob`.
///
/// Expects the input format to be exactly `[12-byte nonce][ciphertext + auth tag]`.
/// Returns `DecryptionFailed` if the authentication tag is invalid or the key is incorrect.
pub fn decrypt_blob(key: &[u8; 32], encrypted: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if encrypted.len() < 12 {
        return Err(CryptoError::InvalidCiphertextLength);
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::DecryptionFailed)?;
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Manages the master encryption key using the OS native credential store.
///
/// The master key is stored as a hex-encoded string in the system keychain
/// (e.g., Keychain on macOS, Credential Manager on Windows, Secret Service on Linux).
pub struct MasterKeyManager;

impl MasterKeyManager {
    const SERVICE_NAME: &'static str = "com.littmaily.desktop";
    const ACCOUNT_NAME: &'static str = "master-key";

    /// Retrieves the master key from the OS keychain, generating and saving a new one if it doesn't exist.
    ///
    /// If a key exists but has an invalid length (e.g., corrupted or from an older version),
    /// it is overwritten with a newly generated key to prevent cascading decryption failures.
    pub fn get_or_create_key() -> Result<[u8; 32], CryptoError> {
        let entry = keyring::Entry::new(Self::SERVICE_NAME, Self::ACCOUNT_NAME)
            .map_err(|e| CryptoError::KeyringError(e.to_string()))?;

        match entry.get_password() {
            Ok(hex_key) => {
                let bytes =
                    hex::decode(hex_key).map_err(|e| CryptoError::HexError(e.to_string()))?;
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    Ok(key)
                } else {
                    // Invalid length, recreate
                    Self::generate_and_save_key(&entry)
                }
            }
            Err(keyring::Error::NoEntry) => Self::generate_and_save_key(&entry),
            Err(e) => Err(CryptoError::KeyringError(e.to_string())),
        }
    }

    /// Generates a cryptographically secure 256-bit key and persists it to the OS keychain.
    fn generate_and_save_key(entry: &keyring::Entry) -> Result<[u8; 32], CryptoError> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        // Store as hex string because some OS keychains (like older Secret Service implementations)
        // have issues storing raw binary data or null bytes in password fields.
        let hex_key = hex::encode(key);
        entry
            .set_password(&hex_key)
            .map_err(|e| CryptoError::KeyringError(e.to_string()))?;
        Ok(key)
    }
}
