mod backend;
mod backend_blob;
mod backend_index;
mod backend_proxy;
mod blob;
mod chunk;
mod commit;
mod diff;
mod encrypted;
mod gc;
mod hash;
mod migrate;
mod mirror;
mod snapshot;
mod store;

pub use backend::{BackendError, ExternalObjectStoreBackend, LocalFsBackend, StorageBackend};
pub use backend_blob::{read_blob_from_backend, store_blob_on_backend};
pub use backend_index::{forget_location, hashes_on_backend, location_for, record_location};
pub use backend_proxy::{BackendProxy, ProxiedFileBackend, ProxyProvider, ProxyRequest, ProxyResponse};
pub use blob::{read_blob, store_blob, BlobManifest};
pub use chunk::{chunk_reader, ChunkRef};
pub use commit::{commit_version, history, VersionEntry};
pub use diff::{diff_blobs, diff_text, strategy_for, DiffError, DiffLine, DiffStrategy, LineTag, TextDiffResult};
pub use encrypted::EncryptedBackend;
pub use gc::{live_object_hashes, mark_and_sweep, GcError, GcReport};
pub use hash::hash_bytes;
pub use migrate::{migrate_objects, read_blob_with_fallback, read_object_with_fallback, MigrationReport};
pub use mirror::{pull_on_demand, BlobMirror, MirrorError};
pub use snapshot::{
    all_ref_snapshots, create_snapshot, get_ref, list_refs, read_snapshot, set_branch, set_tag, RefEntry, SnapshotError,
    SnapshotManifest,
};
pub use store::ObjectStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_retrieve_roundtrip_by_hash() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let data = b"the quick brown fox jumps over the lazy dog".repeat(1000);

        let blob_ref = store_blob(&store, &data).unwrap();
        let retrieved = read_blob(&store, &blob_ref).unwrap();

        assert_eq!(retrieved, data);
    }

    #[test]
    fn storing_the_same_content_twice_dedups() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let data = b"identical payload".repeat(5000);

        let first_ref = store_blob(&store, &data).unwrap();
        let count_after_first = count_objects(dir.path());

        let second_ref = store_blob(&store, &data).unwrap();
        let count_after_second = count_objects(dir.path());

        assert_eq!(first_ref, second_ref);
        assert_eq!(count_after_first, count_after_second);
    }

    #[test]
    fn different_content_produces_different_blob_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());

        let a = store_blob(&store, b"content a").unwrap();
        let b = store_blob(&store, b"content b").unwrap();

        assert_ne!(a, b);
    }

    #[test]
    fn objects_are_laid_out_under_objects_hh_hash() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let data = b"layout check";

        let blob_ref = store_blob(&store, data).unwrap();

        let expected = dir
            .path()
            .join("objects")
            .join(&blob_ref[..2])
            .join(&blob_ref);
        assert!(expected.exists());
    }

    #[test]
    fn write_object_reports_whether_it_was_new() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let h = hash_bytes(b"dedup me");

        assert!(store.write_object(&h, b"dedup me").unwrap());
        assert!(!store.write_object(&h, b"dedup me").unwrap());
    }

    fn count_objects(root: &std::path::Path) -> usize {
        walkdir::WalkDir::new(root.join("objects"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .count()
    }
}
