//! Blob migration + multi-backend read fallback (issue #108,
//! docs/STORAGE-BACKENDS.md §4).
//!
//! Identity is the hash, so migrating or mirroring between backends
//! rewrites NO entity/edge/event/snapshot - only where the bytes live
//! moves. Migration is a plain loop: for every live object hash, read the
//! bytes from whichever backend has them and `put` them on the target.
//! `has`-before-put makes the whole thing resumable for free: a crashed or
//! re-claimed job re-runs and skips everything already copied.

use std::sync::Arc;

use crate::backend::{BackendError, StorageBackend};

/// Reads one object by hash, trying each backend in order (primary first,
/// then mirrors). Every candidate's bytes are BLAKE3-verified by `get`, so
/// a corrupt copy on one backend falls through to the next instead of
/// propagating.
pub async fn read_object_with_fallback(
    backends: &[Arc<dyn StorageBackend>],
    hash: &str,
) -> Result<Vec<u8>, BackendError> {
    let mut last_err = BackendError::NotFound { hash: hash.to_string() };
    for backend in backends {
        match backend.get(hash).await {
            Ok(bytes) => return Ok(bytes),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

/// Reads a whole blob (manifest + chunks) by `blob_ref`, each object
/// resolved with cross-backend fallback. What the frontend's blob fetch
/// rides (issue #109) once a workspace has remote backends.
pub async fn read_blob_with_fallback(
    backends: &[Arc<dyn StorageBackend>],
    blob_ref: &str,
) -> Result<Vec<u8>, BackendError> {
    let manifest_json = read_object_with_fallback(backends, blob_ref).await?;
    let manifest: crate::BlobManifest = serde_json::from_slice(&manifest_json)
        .map_err(|e| BackendError::Other(format!("manifest parse failed for {blob_ref}: {e}")))?;
    let mut out = Vec::new();
    for chunk in &manifest.chunks {
        out.extend(read_object_with_fallback(backends, &chunk.hash).await?);
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MigrationReport {
    /// Objects copied onto the target this run.
    pub migrated: usize,
    /// Objects the target already had (the resume path).
    pub skipped: usize,
    /// Objects no source could produce verified bytes for.
    pub failed: usize,
}

/// Re-puts every hash in `hashes` onto `target`, reading from `sources`
/// with fallback. Skips objects the target already holds (resumability);
/// `on_progress(done, total)` fires after every object so a caller can
/// persist progress into its `jobs` row (drain-friendly).
pub async fn migrate_objects(
    sources: &[Arc<dyn StorageBackend>],
    target: &dyn StorageBackend,
    hashes: &[String],
    mut on_progress: impl FnMut(usize, usize),
) -> Result<MigrationReport, BackendError> {
    let mut report = MigrationReport::default();
    let total = hashes.len();

    for (i, hash) in hashes.iter().enumerate() {
        if target.has(hash).await? {
            report.skipped += 1;
        } else {
            match read_object_with_fallback(sources, hash).await {
                Ok(bytes) => {
                    target.put(hash, &bytes).await?;
                    report.migrated += 1;
                }
                Err(_) => report.failed += 1,
            }
        }
        on_progress(i + 1, total);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::LocalFsBackend;
    use crate::hash::hash_bytes;

    fn backend_in(dir: &tempfile::TempDir) -> Arc<dyn StorageBackend> {
        Arc::new(LocalFsBackend::new(dir.path()))
    }

    #[tokio::test]
    async fn migrating_moves_every_object_and_bytes_verify_on_the_target() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let source = backend_in(&src_dir);
        let target = LocalFsBackend::new(dst_dir.path());

        let mut hashes = Vec::new();
        for i in 0..5u8 {
            let data = format!("object {i}").into_bytes();
            let hash = hash_bytes(&data);
            source.put(&hash, &data).await.unwrap();
            hashes.push(hash);
        }

        let report = migrate_objects(&[source], &target, &hashes, |_, _| {}).await.unwrap();

        assert_eq!(report.migrated, 5);
        for (i, hash) in hashes.iter().enumerate() {
            assert_eq!(target.get(hash).await.unwrap(), format!("object {i}").into_bytes());
        }
    }

    #[tokio::test]
    async fn a_second_run_skips_everything_already_copied() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let source = backend_in(&src_dir);
        let target = LocalFsBackend::new(dst_dir.path());
        let data = b"copy once".to_vec();
        let hash = hash_bytes(&data);
        source.put(&hash, &data).await.unwrap();
        let hashes = vec![hash];

        let first = migrate_objects(std::slice::from_ref(&source), &target, &hashes, |_, _| {}).await.unwrap();
        let second = migrate_objects(&[source], &target, &hashes, |_, _| {}).await.unwrap();

        assert_eq!((first.migrated, first.skipped), (1, 0));
        assert_eq!((second.migrated, second.skipped), (0, 1));
    }

    #[tokio::test]
    async fn reads_fall_back_across_backends_by_hash() {
        let a_dir = tempfile::tempdir().unwrap();
        let b_dir = tempfile::tempdir().unwrap();
        let primary = backend_in(&a_dir);
        let mirror = backend_in(&b_dir);
        let data = b"only on the mirror".to_vec();
        let hash = hash_bytes(&data);
        mirror.put(&hash, &data).await.unwrap();

        let bytes = read_object_with_fallback(&[primary, mirror], &hash).await.unwrap();

        assert_eq!(bytes, data);
    }

    #[tokio::test]
    async fn a_corrupt_copy_on_one_backend_falls_through_to_a_good_one() {
        let a_dir = tempfile::tempdir().unwrap();
        let b_dir = tempfile::tempdir().unwrap();
        let corrupt = LocalFsBackend::new(a_dir.path());
        let good = backend_in(&b_dir);
        let data = b"resilient".to_vec();
        let hash = hash_bytes(&data);
        corrupt.put(&hash, b"rotted bytes").await.unwrap(); // wrong content under the hash
        good.put(&hash, &data).await.unwrap();

        let bytes = read_object_with_fallback(&[Arc::new(corrupt), good], &hash).await.unwrap();

        assert_eq!(bytes, data);
    }

    #[tokio::test]
    async fn missing_objects_are_reported_not_fatal() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let source = backend_in(&src_dir);
        let target = LocalFsBackend::new(dst_dir.path());
        let data = b"present".to_vec();
        let present = hash_bytes(&data);
        source.put(&present, &data).await.unwrap();
        let hashes = vec![present, hash_bytes(b"lost forever")];

        let mut progress_calls = 0;
        let report = migrate_objects(&[source], &target, &hashes, |_, _| progress_calls += 1)
            .await
            .unwrap();

        assert_eq!((report.migrated, report.failed), (1, 1));
        assert_eq!(progress_calls, 2);
    }
}
