//! Chunk-level blob put/get over any [`StorageBackend`] (issue #106,
//! docs/STORAGE-BACKENDS.md §2, docs/VERSIONING.md §2.1).
//!
//! FastCDC chunking is backend-agnostic: a blob stays a Merkle manifest of
//! chunk hashes, and chunks are `put`/`get` individually. That gives every
//! backend - including partial-read remotes - dedup and incremental sync for
//! free: a re-export that only changes part of a large file re-stores only
//! the changed chunks, not the whole file. `has` is checked before each
//! `put`, so unchanged chunks cost one HEAD, not a re-upload.
//!
//! The manifest format is identical to the local-CAS one (`BlobManifest`),
//! so a `blob_ref` stored through a backend is the same `blob_ref` the local
//! store would produce for the same bytes - identity never depends on
//! location.

use crate::backend::{BackendError, StorageBackend};
use crate::blob::BlobManifest;
use crate::chunk::chunk_reader;
use crate::hash::hash_bytes;

/// Chunks `data`, puts each chunk the backend doesn't already have, puts the
/// manifest, and returns the blob's content address. Byte-for-byte the same
/// `blob_ref` as the local `store_blob` for the same content.
pub async fn store_blob_on_backend(
    backend: &dyn StorageBackend,
    data: &[u8],
) -> Result<String, BackendError> {
    let chunks = chunk_reader(data).map_err(BackendError::Io)?;

    for chunk in &chunks {
        if backend.has(&chunk.hash).await? {
            continue; // content-addressed dedup: unchanged chunk, no re-upload
        }
        let bytes = &data[chunk.offset as usize..(chunk.offset + chunk.length as u64) as usize];
        backend.put(&chunk.hash, bytes).await?;
    }

    let manifest = BlobManifest { chunks };
    let manifest_json = serde_json::to_vec(&manifest)
        .map_err(|e| BackendError::Other(format!("manifest serialization failed: {e}")))?;
    let blob_ref = hash_bytes(&manifest_json);
    if !backend.has(&blob_ref).await? {
        backend.put(&blob_ref, &manifest_json).await?;
    }
    Ok(blob_ref)
}

/// Fetches the manifest for `blob_ref`, then each chunk, reassembling the
/// original bytes. Every fetched object (manifest and chunks) is
/// BLAKE3-verified by the backend's `get`, so a tampered remote fails here
/// rather than returning corrupt content.
pub async fn read_blob_from_backend(
    backend: &dyn StorageBackend,
    blob_ref: &str,
) -> Result<Vec<u8>, BackendError> {
    let manifest_json = backend.get(blob_ref).await?;
    let manifest: BlobManifest = serde_json::from_slice(&manifest_json)
        .map_err(|e| BackendError::Other(format!("manifest parse failed for {blob_ref}: {e}")))?;

    let mut out = Vec::new();
    for chunk in &manifest.chunks {
        out.extend(backend.get(&chunk.hash).await?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::LocalFsBackend;
    use crate::store::ObjectStore;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Counts `put` calls so tests can assert dedup/incremental behavior.
    struct CountingBackend {
        inner: LocalFsBackend,
        puts: AtomicUsize,
    }

    impl CountingBackend {
        fn new(dir: &std::path::Path) -> Self {
            Self { inner: LocalFsBackend::new(dir), puts: AtomicUsize::new(0) }
        }

        fn put_count(&self) -> usize {
            self.puts.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl StorageBackend for CountingBackend {
        async fn put(&self, hash: &str, bytes: &[u8]) -> Result<(), BackendError> {
            self.puts.fetch_add(1, Ordering::SeqCst);
            self.inner.put(hash, bytes).await
        }
        async fn fetch_unverified(&self, hash: &str) -> Result<Vec<u8>, BackendError> {
            self.inner.fetch_unverified(hash).await
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

    /// Deterministic pseudo-random content large enough to span many chunks
    /// (FastCDC min chunk is 16KB).
    fn big_content(len: usize, seed: u8) -> Vec<u8> {
        let mut data = vec![0u8; len];
        let mut state = seed as u32 | 1;
        for byte in data.iter_mut() {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            *byte = (state >> 16) as u8;
        }
        data
    }

    #[tokio::test]
    async fn blob_round_trips_chunk_by_chunk_through_a_backend() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(dir.path());
        let data = big_content(1_000_000, 7);

        let blob_ref = store_blob_on_backend(&backend, &data).await.unwrap();
        let restored = read_blob_from_backend(&backend, &blob_ref).await.unwrap();

        assert_eq!(restored, data);
    }

    #[tokio::test]
    async fn backend_blob_ref_matches_the_local_store_blob_ref() {
        // Identity never depends on location: the same bytes produce the
        // same blob_ref whether stored locally or through a backend.
        let backend_dir = tempfile::tempdir().unwrap();
        let local_dir = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(backend_dir.path());
        let local = ObjectStore::new(local_dir.path());
        let data = big_content(300_000, 3);

        let via_backend = store_blob_on_backend(&backend, &data).await.unwrap();
        let via_local = crate::store_blob(&local, &data).unwrap();

        assert_eq!(via_backend, via_local);
    }

    #[tokio::test]
    async fn re_storing_identical_content_uploads_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let backend = CountingBackend::new(dir.path());
        let data = big_content(500_000, 5);

        store_blob_on_backend(&backend, &data).await.unwrap();
        let puts_after_first = backend.put_count();
        store_blob_on_backend(&backend, &data).await.unwrap();

        assert_eq!(backend.put_count(), puts_after_first);
    }

    #[tokio::test]
    async fn an_edit_near_the_start_re_stores_only_the_changed_chunks() {
        // The issue-#106 acceptance: a re-export with a trimmed/changed
        // intro re-stores ~one chunk plus the new manifest, not the file.
        let dir = tempfile::tempdir().unwrap();
        let backend = CountingBackend::new(dir.path());
        let original = big_content(2_000_000, 9);

        store_blob_on_backend(&backend, &original).await.unwrap();
        let puts_for_original = backend.put_count();
        assert!(puts_for_original > 5, "fixture must span many chunks");

        let mut edited = original.clone();
        for byte in edited.iter_mut().take(100) {
            *byte = byte.wrapping_add(1); // touch only the intro
        }
        store_blob_on_backend(&backend, &edited).await.unwrap();
        let puts_for_edit = backend.put_count() - puts_for_original;

        // Changed intro chunk(s) + the new manifest; far below a full re-upload.
        assert!(
            puts_for_edit <= 3,
            "expected ~1 chunk + manifest, got {puts_for_edit} puts (of {puts_for_original} total chunks)"
        );
    }

    #[tokio::test]
    async fn a_tampered_chunk_fails_the_read_not_just_the_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(dir.path());
        let data = big_content(200_000, 11);
        let blob_ref = store_blob_on_backend(&backend, &data).await.unwrap();

        // Corrupt one chunk object on disk, leaving the manifest intact.
        let manifest_json = backend.get(&blob_ref).await.unwrap();
        let manifest: BlobManifest = serde_json::from_slice(&manifest_json).unwrap();
        let victim = &manifest.chunks[0].hash;
        let path = dir.path().join("objects").join(&victim[..2]).join(victim);
        std::fs::write(&path, b"tampered chunk bytes").unwrap();

        let result = read_blob_from_backend(&backend, &blob_ref).await;

        assert!(matches!(result, Err(BackendError::IntegrityMismatch { .. })));
    }
}
