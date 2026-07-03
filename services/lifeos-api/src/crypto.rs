//! Envelope encryption for `connections.secret_enc` - the handful of
//! non-Nango secrets (Kite's daily access token, WhatsApp's session token,
//! docs/INTEGRATIONS.md §3) that don't fit Nango's OAuth vault. AES-256-GCM
//! with a random nonce per call; the server-held master key never leaves this
//! process (docs/SECURITY.md §1: "never in agent context, never in logs").

use crate::error::ApiError;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;

/// AES-256-GCM key, resolved once at boot from `LIFEOS_SECRET_ENCRYPTION_KEY`
/// (base64, 32 raw bytes - `openssl rand -base64 32`).
pub type EncryptionKey = [u8; 32];

pub fn parse_key(base64_key: &str) -> Result<EncryptionKey, String> {
    let bytes = STANDARD
        .decode(base64_key.trim())
        .map_err(|e| format!("LIFEOS_SECRET_ENCRYPTION_KEY is not valid base64: {e}"))?;
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| format!("LIFEOS_SECRET_ENCRYPTION_KEY must decode to 32 bytes, got {}", v.len()))
}

/// Encrypts `plaintext`, returning a base64 blob of `nonce || ciphertext`
/// suitable for `connections.secret_enc`.
pub fn encrypt(plaintext: &str, key: &EncryptionKey) -> Result<String, ApiError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| ApiError::Internal("envelope encryption failed".into()))?;
    let mut blob = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(blob))
}

/// Reverses [`encrypt`]. Fails closed (Internal) on any tamper/format/key
/// mismatch rather than returning partial plaintext.
pub fn decrypt(blob: &str, key: &EncryptionKey) -> Result<String, ApiError> {
    let raw = STANDARD
        .decode(blob)
        .map_err(|_| ApiError::Internal("secret_enc is not valid base64".into()))?;
    if raw.len() < 12 {
        return Err(ApiError::Internal("secret_enc blob too short".into()));
    }
    let (nonce_bytes, ciphertext) = raw.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| ApiError::Internal("envelope decryption failed - wrong key or tampered blob".into()))?;
    String::from_utf8(plaintext).map_err(|_| ApiError::Internal("decrypted secret_enc is not valid UTF-8".into()))
}

/// Generates a fresh random 32-byte envelope key (per-workspace envelope
/// keys, issue #104 - `docs/DATA-MODEL.md` §4, `docs/SECURITY.md` §5).
pub fn random_key() -> EncryptionKey {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

/// Ensures `workspaces.envelope_key_enc` is set, generating + storing one
/// under the server's master key if it isn't yet. Idempotent. Shared by
/// database-per-workspace provisioning (issue #104) and client-side blob
/// encryption (issue #110) so both derive the same per-workspace key.
pub async fn ensure_envelope_key(
    conn: &libsql::Connection,
    master_key: &EncryptionKey,
    workspace_id: &str,
) -> Result<EncryptionKey, ApiError> {
    let mut rows = conn
        .query(
            "SELECT envelope_key_enc FROM workspaces WHERE id = ?1",
            libsql::params![workspace_id],
        )
        .await?;
    let existing: Option<String> = match rows.next().await? {
        Some(row) => row.get(0)?,
        None => return Err(ApiError::BadRequest(format!("unknown workspace '{workspace_id}'"))),
    };
    if let Some(enc) = existing {
        let raw = decrypt(&enc, master_key)?;
        let bytes = STANDARD
            .decode(raw)
            .map_err(|_| ApiError::Internal("envelope_key_enc did not decode to raw key bytes".into()))?;
        return bytes
            .try_into()
            .map_err(|_| ApiError::Internal("envelope key is not 32 bytes".into()));
    }

    let key = random_key();
    let key_b64 = STANDARD.encode(key);
    let enc = encrypt(&key_b64, master_key)?;
    conn.execute(
        "UPDATE workspaces SET envelope_key_enc = ?1, updated_at = ?2 WHERE id = ?3",
        libsql::params![enc, crate::ids::now_secs(), workspace_id],
    )
    .await?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> EncryptionKey {
        [7u8; 32]
    }

    #[test]
    fn roundtrips_plaintext() {
        let key = test_key();
        let blob = encrypt("kite-access-token-abc123", &key).unwrap();
        assert_ne!(blob, "kite-access-token-abc123", "must not store plaintext");
        assert_eq!(decrypt(&blob, &key).unwrap(), "kite-access-token-abc123");
    }

    #[test]
    fn two_encryptions_of_same_plaintext_differ() {
        let key = test_key();
        let a = encrypt("same-secret", &key).unwrap();
        let b = encrypt("same-secret", &key).unwrap();
        assert_ne!(a, b, "random nonce must make ciphertexts unlinkable");
    }

    #[test]
    fn wrong_key_fails_closed() {
        let blob = encrypt("secret", &test_key()).unwrap();
        let wrong_key = [9u8; 32];
        assert!(decrypt(&blob, &wrong_key).is_err());
    }

    #[test]
    fn parses_valid_base64_key() {
        let encoded = STANDARD.encode([1u8; 32]);
        assert_eq!(parse_key(&encoded).unwrap(), [1u8; 32]);
    }

    #[test]
    fn rejects_wrong_length_key() {
        let encoded = STANDARD.encode([1u8; 16]);
        assert!(parse_key(&encoded).is_err());
    }
}
