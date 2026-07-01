mod blob;
mod chunk;
mod commit;
mod hash;
mod mirror;
mod store;

pub use blob::{read_blob, store_blob, BlobManifest};
pub use chunk::{chunk_reader, ChunkRef};
pub use commit::{commit_version, history, VersionEntry};
pub use hash::hash_bytes;
pub use mirror::{pull_on_demand, BlobMirror, MirrorError};
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
