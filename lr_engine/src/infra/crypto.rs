//! Application-layer encryption for KYC PII.
//!
//! Fields are sealed with AES-256-GCM before they reach the database, so a
//! leaked dump (or a read-only DB compromise) exposes ciphertext only. The
//! key lives in `KYC_ENC_KEY` (64 hex chars = 32 bytes) and never touches
//! the database or the SQL statement log — unlike pgcrypto, where the key
//! rides inside every query.
//!
//! Everything here fails *closed*: with the key missing or malformed, seal
//! and open return errors and the KYC endpoints refuse to operate. PII is
//! never stored in the clear as a fallback.

use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::OnceLock;

/// AES-GCM standard 96-bit nonce, prepended to the ciphertext.
const NONCE_LEN: usize = 12;

fn enc_key() -> Option<&'static [u8; 32]> {
    static KEY: OnceLock<Option<[u8; 32]>> = OnceLock::new();
    KEY.get_or_init(|| {
        let hex_key = std::env::var("KYC_ENC_KEY").ok()?;
        let bytes = hex::decode(hex_key.trim()).ok()?;
        bytes.try_into().ok()
    })
    .as_ref()
}

pub fn is_configured() -> bool {
    enc_key().is_some()
}

/// Encrypt a PII field for storage: random nonce || AES-256-GCM ciphertext.
/// GCM authenticates as well as encrypts, so tampering with a stored blob is
/// detected at decrypt time instead of yielding garbage plaintext.
pub fn seal(plaintext: &str) -> Result<Vec<u8>, &'static str> {
    let key = enc_key().ok_or("KYC_ENC_KEY is not configured")?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce: [u8; NONCE_LEN] = rand::random();
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|_| "encryption failed")?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a blob produced by [`seal`].
pub fn open(sealed: &[u8]) -> Result<String, &'static str> {
    let key = enc_key().ok_or("KYC_ENC_KEY is not configured")?;
    if sealed.len() <= NONCE_LEN {
        return Err("ciphertext too short");
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&sealed[..NONCE_LEN]), &sealed[NONCE_LEN..])
        .map_err(|_| "decryption failed")?;
    String::from_utf8(plaintext).map_err(|_| "invalid utf8")
}

/// Deterministic HMAC-SHA256 blind index over the normalized ID number
/// (uppercased, non-alphanumerics stripped, so "1234-5678" and "1234 5678"
/// collide as intended). Lets the unique index reject the same government ID
/// on a second account without the database ever seeing the number. Keyed
/// with `KYC_HASH_SECRET`; an empty key still hashes (never plaintext), but
/// only a real secret makes offline enumeration of ID numbers infeasible.
pub fn blind_index(id_number: &str) -> String {
    static SECRET: OnceLock<Vec<u8>> = OnceLock::new();
    let secret = SECRET.get_or_init(|| {
        std::env::var("KYC_HASH_SECRET")
            .unwrap_or_default()
            .into_bytes()
    });
    let normalized: String = id_number
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    // fully-qualified: aes-gcm's KeyInit also has a new_from_slice
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(normalized.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
