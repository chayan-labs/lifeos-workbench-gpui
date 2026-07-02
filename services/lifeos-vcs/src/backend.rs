//! Pluggable storage backends (issue #105, docs/STORAGE-BACKENDS.md §2).
//!
//! A `blob_ref` is a BLAKE3 content hash, not a path - the hash identifies
//! the bytes and a backend only answers "given this hash, where are the
//! bytes and how do I get them?". `lifeos-vcs` depends only on this trait;
//! which backend a workspace uses is configuration, never code.
//!
//! Two impls live here:
//! - [`LocalFsBackend`] - today's `objects/<hh>/<hash>` layout (the default;
//!   with no backend configured nothing regresses).
//! - [`ExternalObjectStoreBackend`] - one impl over the `object_store` crate
//!   covering R2/S3/GCS/Azure/local-directory remotes.
//!
//! Integrity is non-negotiable: every `get` re-hashes the fetched bytes with
//! BLAKE3 and rejects a mismatch, so an untrusted/remote backend cannot
//! silently corrupt or swap content. `delete` exists for GC/maintenance only
//! and is never wrapped as an agent-callable tool (docs/STORAGE-BACKENDS.md §6).

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore as ExternalObjectStore, PutPayload};

use crate::hash::hash_bytes;
use crate::store::ObjectStore;

#[derive(Debug)]
pub enum BackendError {
    /// The bytes fetched for a hash don't hash back to it - never trust a
    /// backend's bytes blindly (docs/STORAGE-BACKENDS.md §2).
    IntegrityMismatch { expected: String, actual: String },
    NotFound { hash: String },
    Io(std::io::Error),
    Store(object_store::Error),
    Other(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::IntegrityMismatch { expected, actual } => {
                write!(f, "integrity mismatch: expected {expected}, got {actual}")
            }
            BackendError::NotFound { hash } => write!(f, "object {hash} not found on backend"),
            BackendError::Io(e) => write!(f, "backend io error: {e}"),
            BackendError::Store(e) => write!(f, "object store error: {e}"),
            BackendError::Other(msg) => write!(f, "backend error: {msg}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// Where the bytes physically live, from any storage backend a workspace
/// chooses. Content-addressing decouples identity (the hash) from location
/// (this trait's concern), which is exactly what makes migration/mirroring
/// free: the `blob_ref` never changes, only the backend mapping moves.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Stores `bytes` under `hash`. Idempotent (CAS): re-putting an existing
    /// hash is a no-op or an equal-content overwrite, never an error.
    async fn put(&self, hash: &str, bytes: &[u8]) -> Result<(), BackendError>;

    /// Fetches the stored bytes without integrity verification. Implementor
    /// detail - callers use [`StorageBackend::get`]. On the trait (rather
    /// than private) so wrappers like the encryption layer (issue #110) can
    /// transform stored bytes back into plaintext *before* the plaintext
    /// hash check runs.
    async fn fetch_unverified(&self, hash: &str) -> Result<Vec<u8>, BackendError>;

    /// Fetches the bytes for `hash`, re-hashing them with BLAKE3 and
    /// rejecting a mismatch before returning. Provided: every backend gets
    /// the same non-negotiable integrity check.
    async fn get(&self, hash: &str) -> Result<Vec<u8>, BackendError> {
        verify_bytes(hash, self.fetch_unverified(hash).await?)
    }

    async fn has(&self, hash: &str) -> Result<bool, BackendError>;

    /// GC/maintenance only - never exposed as an agent tool
    /// (docs/STORAGE-BACKENDS.md §6, docs/AGENT-CONTROL.md §1).
    async fn delete(&self, hash: &str) -> Result<(), BackendError>;

    /// The backend-native locator for `hash` (path, object key, provider
    /// file id) - what the backend index persists (issue #106).
    fn location(&self, hash: &str) -> String;
}

/// Shared integrity check: every backend `get` funnels through this.
pub(crate) fn verify_bytes(hash: &str, bytes: Vec<u8>) -> Result<Vec<u8>, BackendError> {
    let actual = hash_bytes(&bytes);
    if actual != hash {
        return Err(BackendError::IntegrityMismatch { expected: hash.to_string(), actual });
    }
    Ok(bytes)
}

/// The default backend: today's local CAS at `objects/<hh>/<hash>`.
/// Wraps [`ObjectStore`] so the on-disk layout stays byte-identical to the
/// pre-trait world - blobs written before this abstraction existed are
/// readable through it and vice versa.
pub struct LocalFsBackend {
    store: ObjectStore,
}

impl LocalFsBackend {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { store: ObjectStore::new(root) }
    }
}

#[async_trait]
impl StorageBackend for LocalFsBackend {
    async fn put(&self, hash: &str, bytes: &[u8]) -> Result<(), BackendError> {
        self.store.write_object(hash, bytes).map_err(BackendError::Io)?;
        Ok(())
    }

    async fn fetch_unverified(&self, hash: &str) -> Result<Vec<u8>, BackendError> {
        self.store.read_object(hash).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => BackendError::NotFound { hash: hash.to_string() },
            _ => BackendError::Io(e),
        })
    }

    async fn has(&self, hash: &str) -> Result<bool, BackendError> {
        Ok(self.store.has_object(hash))
    }

    async fn delete(&self, hash: &str) -> Result<(), BackendError> {
        match std::fs::remove_file(self.store.root().join("objects").join(&hash[..2]).join(hash)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(BackendError::Io(e)),
        }
    }

    fn location(&self, hash: &str) -> String {
        self.store
            .root()
            .join("objects")
            .join(&hash[..2])
            .join(hash)
            .to_string_lossy()
            .into_owned()
    }
}

/// One impl over the `object_store` crate covering R2/S3/GCS/Azure and
/// local-directory remotes (docs/STORAGE-BACKENDS.md §2) - generalizes the
/// old hardcoded R2/S3 mirror into a user-choosable backend.
pub struct ExternalObjectStoreBackend {
    inner: Arc<dyn ExternalObjectStore>,
}

impl ExternalObjectStoreBackend {
    pub fn new(inner: Arc<dyn ExternalObjectStore>) -> Self {
        Self { inner }
    }

    fn object_path(hash: &str) -> ObjectPath {
        ObjectPath::from(format!("objects/{}/{}", &hash[..2], hash))
    }
}

#[async_trait]
impl StorageBackend for ExternalObjectStoreBackend {
    async fn put(&self, hash: &str, bytes: &[u8]) -> Result<(), BackendError> {
        self.inner
            .put(&Self::object_path(hash), PutPayload::from(bytes.to_vec()))
            .await
            .map_err(BackendError::Store)?;
        Ok(())
    }

    async fn fetch_unverified(&self, hash: &str) -> Result<Vec<u8>, BackendError> {
        let result = self.inner.get(&Self::object_path(hash)).await.map_err(|e| match e {
            object_store::Error::NotFound { .. } => BackendError::NotFound { hash: hash.to_string() },
            other => BackendError::Store(other),
        })?;
        let bytes = result.bytes().await.map_err(BackendError::Store)?;
        Ok(bytes.to_vec())
    }

    async fn has(&self, hash: &str) -> Result<bool, BackendError> {
        match self.inner.head(&Self::object_path(hash)).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(e) => Err(BackendError::Store(e)),
        }
    }

    async fn delete(&self, hash: &str) -> Result<(), BackendError> {
        match self.inner.delete(&Self::object_path(hash)).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(BackendError::Store(e)),
        }
    }

    fn location(&self, hash: &str) -> String {
        Self::object_path(hash).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::local::LocalFileSystem;

    fn external_backend(dir: &std::path::Path) -> ExternalObjectStoreBackend {
        ExternalObjectStoreBackend::new(Arc::new(LocalFileSystem::new_with_prefix(dir).unwrap()))
    }

    #[tokio::test]
    async fn local_fs_backend_round_trips_a_blob() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(dir.path());
        let data = b"local backend bytes".to_vec();
        let hash = hash_bytes(&data);

        backend.put(&hash, &data).await.unwrap();

        assert!(backend.has(&hash).await.unwrap());
        assert_eq!(backend.get(&hash).await.unwrap(), data);
    }

    #[tokio::test]
    async fn local_fs_backend_matches_the_legacy_objects_layout() {
        // No regression: blobs written through the trait are readable by the
        // pre-trait ObjectStore, and vice versa (identical on-disk layout).
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(dir.path());
        let legacy = ObjectStore::new(dir.path());
        let data = b"same layout".to_vec();
        let hash = hash_bytes(&data);

        backend.put(&hash, &data).await.unwrap();
        assert_eq!(legacy.read_object(&hash).unwrap(), data);

        let data2 = b"written by legacy".to_vec();
        let hash2 = hash_bytes(&data2);
        legacy.write_object(&hash2, &data2).unwrap();
        assert_eq!(backend.get(&hash2).await.unwrap(), data2);
    }

    #[tokio::test]
    async fn object_store_backend_round_trips_a_blob() {
        let dir = tempfile::tempdir().unwrap();
        let backend = external_backend(dir.path());
        let data = b"remote backend bytes".to_vec();
        let hash = hash_bytes(&data);

        backend.put(&hash, &data).await.unwrap();

        assert!(backend.has(&hash).await.unwrap());
        assert_eq!(backend.get(&hash).await.unwrap(), data);
    }

    #[tokio::test]
    async fn get_rejects_tampered_bytes_on_any_backend() {
        let dir = tempfile::tempdir().unwrap();
        let backend = external_backend(dir.path());
        // Store tampered bytes under a hash they don't match.
        backend.put("not-the-real-hash", b"tampered").await.unwrap();

        let result = backend.get("not-the-real-hash").await;

        assert!(matches!(result, Err(BackendError::IntegrityMismatch { .. })));
    }

    #[tokio::test]
    async fn local_fs_get_rejects_corrupted_object_files() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(dir.path());
        let data = b"soon corrupted".to_vec();
        let hash = hash_bytes(&data);
        backend.put(&hash, &data).await.unwrap();
        // Corrupt in place (bit-rot / partial write).
        let path = dir.path().join("objects").join(&hash[..2]).join(&hash);
        std::fs::write(&path, b"rotted").unwrap();

        let result = backend.get(&hash).await;

        assert!(matches!(result, Err(BackendError::IntegrityMismatch { .. })));
    }

    #[tokio::test]
    async fn missing_object_is_not_found_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let backend = external_backend(dir.path());
        let hash = hash_bytes(b"never stored");

        assert!(!backend.has(&hash).await.unwrap());
        assert!(matches!(backend.get(&hash).await, Err(BackendError::NotFound { .. })));
    }

    #[tokio::test]
    async fn put_is_idempotent_and_delete_is_gc_only_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(dir.path());
        let data = b"idempotent".to_vec();
        let hash = hash_bytes(&data);

        backend.put(&hash, &data).await.unwrap();
        backend.put(&hash, &data).await.unwrap(); // second put: no error

        backend.delete(&hash).await.unwrap();
        assert!(!backend.has(&hash).await.unwrap());
        backend.delete(&hash).await.unwrap(); // deleting a missing hash: no error
    }

    #[tokio::test]
    async fn backends_are_usable_as_trait_objects() {
        // lifeos-vcs depends only on the trait (issue #105 scope) - callers
        // hold `Arc<dyn StorageBackend>` and swap impls freely.
        let local_dir = tempfile::tempdir().unwrap();
        let remote_dir = tempfile::tempdir().unwrap();
        let backends: Vec<Arc<dyn StorageBackend>> = vec![
            Arc::new(LocalFsBackend::new(local_dir.path())),
            Arc::new(external_backend(remote_dir.path())),
        ];
        let data = b"polymorphic".to_vec();
        let hash = hash_bytes(&data);

        for backend in &backends {
            backend.put(&hash, &data).await.unwrap();
            assert_eq!(backend.get(&hash).await.unwrap(), data);
            assert!(!backend.location(&hash).is_empty());
        }
    }
}
