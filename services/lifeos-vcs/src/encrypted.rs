//! Client-side envelope encryption for semi-trusted backends (issue #110,
//! docs/STORAGE-BACKENDS.md §3/§6): wrap any [`StorageBackend`] so the
//! provider (Drive/Dropbox/...) stores ciphertext only.
//!
//! The content hash is computed over PLAINTEXT - identity stays stable, so
//! turning encryption on/off or migrating between encrypted and plain
//! backends never rewrites a `blob_ref`. The stored bytes are
//! `nonce || AES-256-GCM(plaintext)` under the per-workspace envelope key
//! (the same key model as `connections.secret_enc`, docs/SECURITY.md §5).
//!
//! Integrity still holds end-to-end: `get` decrypts and then the trait's
//! provided BLAKE3 check verifies the *plaintext* against the hash; GCM's
//! auth tag additionally rejects ciphertext tampering at decrypt time.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use async_trait::async_trait;
use rand::RngCore;
use std::sync::Arc;

use crate::backend::{BackendError, StorageBackend};

const NONCE_LEN: usize = 12;

pub struct EncryptedBackend {
    inner: Arc<dyn StorageBackend>,
    key: [u8; 32],
}

impl EncryptedBackend {
    pub fn new(inner: Arc<dyn StorageBackend>, key: [u8; 32]) -> Self {
        Self { inner, key }
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key))
    }
}

#[async_trait]
impl StorageBackend for EncryptedBackend {
    async fn put(&self, hash: &str, bytes: &[u8]) -> Result<(), BackendError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let ciphertext = self
            .cipher()
            .encrypt(Nonce::from_slice(&nonce_bytes), bytes)
            .map_err(|_| BackendError::Other("envelope encryption failed".into()))?;
        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        self.inner.put(hash, &blob).await
    }

    /// Fetches ciphertext from the inner backend and decrypts it; the
    /// provided `get` then BLAKE3-verifies the plaintext. Fails closed on
    /// any tamper/format/key mismatch (GCM tag).
    async fn fetch_unverified(&self, hash: &str) -> Result<Vec<u8>, BackendError> {
        let blob = self.inner.fetch_unverified(hash).await?;
        if blob.len() < NONCE_LEN {
            return Err(BackendError::Other(format!("encrypted blob for {hash} is too short")));
        }
        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        self.cipher()
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| BackendError::Other(format!("envelope decryption failed for {hash} - wrong key or tampered ciphertext")))
    }

    async fn has(&self, hash: &str) -> Result<bool, BackendError> {
        self.inner.has(hash).await
    }

    async fn delete(&self, hash: &str) -> Result<(), BackendError> {
        self.inner.delete(hash).await
    }

    fn location(&self, hash: &str) -> String {
        self.inner.location(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::LocalFsBackend;
    use crate::hash::hash_bytes;

    fn key() -> [u8; 32] {
        [42u8; 32]
    }

    #[tokio::test]
    async fn round_trips_with_the_blob_ref_computed_over_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let backend = EncryptedBackend::new(Arc::new(LocalFsBackend::new(dir.path())), key());
        let data = b"private notes".to_vec();
        let hash = hash_bytes(&data); // plaintext hash = stable identity

        backend.put(&hash, &data).await.unwrap();

        assert_eq!(backend.get(&hash).await.unwrap(), data);
    }

    #[tokio::test]
    async fn the_provider_stores_ciphertext_only() {
        let dir = tempfile::tempdir().unwrap();
        let inner = Arc::new(LocalFsBackend::new(dir.path()));
        let backend = EncryptedBackend::new(inner.clone(), key());
        let data = b"the provider must never see this plaintext".to_vec();
        let hash = hash_bytes(&data);

        backend.put(&hash, &data).await.unwrap();

        let stored = inner.fetch_unverified(&hash).await.unwrap();
        assert_ne!(stored, data);
        assert!(
            !stored.windows(data.len()).any(|w| w == data.as_slice()),
            "plaintext leaked into the stored bytes"
        );
    }

    #[tokio::test]
    async fn wrong_key_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let inner = Arc::new(LocalFsBackend::new(dir.path()));
        let backend = EncryptedBackend::new(inner.clone(), key());
        let data = b"sealed".to_vec();
        let hash = hash_bytes(&data);
        backend.put(&hash, &data).await.unwrap();

        let wrong = EncryptedBackend::new(inner, [9u8; 32]);
        assert!(wrong.get(&hash).await.is_err());
    }

    #[tokio::test]
    async fn tampered_ciphertext_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let inner = Arc::new(LocalFsBackend::new(dir.path()));
        let backend = EncryptedBackend::new(inner.clone(), key());
        let data = b"integrity matters".to_vec();
        let hash = hash_bytes(&data);
        backend.put(&hash, &data).await.unwrap();

        let mut stored = inner.fetch_unverified(&hash).await.unwrap();
        let last = stored.len() - 1;
        stored[last] ^= 0xFF;
        inner.delete(&hash).await.unwrap();
        inner.put(&hash, &stored).await.unwrap();

        assert!(backend.get(&hash).await.is_err());
    }

    #[tokio::test]
    async fn chunked_blob_store_works_over_an_encrypted_backend() {
        let dir = tempfile::tempdir().unwrap();
        let backend = EncryptedBackend::new(Arc::new(LocalFsBackend::new(dir.path())), key());
        let mut data = vec![0u8; 300_000];
        let mut state = 13u32;
        for byte in data.iter_mut() {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            *byte = (state >> 16) as u8;
        }

        let blob_ref = crate::store_blob_on_backend(&backend, &data).await.unwrap();
        let restored = crate::read_blob_from_backend(&backend, &blob_ref).await.unwrap();

        assert_eq!(restored, data);
    }
}
