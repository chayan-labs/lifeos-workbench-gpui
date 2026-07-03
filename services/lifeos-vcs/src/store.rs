use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Content-addressed object store rooted at `<root>/objects/<hh>/<hash>`,
/// where `<hh>` is the first two hex chars of the BLAKE3 hash (docs/VERSIONING.md §2.1).
pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn object_path(&self, hash: &str) -> PathBuf {
        let prefix = &hash[..2];
        self.root.join("objects").join(prefix).join(hash)
    }

    pub fn has_object(&self, hash: &str) -> bool {
        self.object_path(hash).exists()
    }

    /// Writes `data` under `hash`, skipping the write if the object already
    /// exists (content-addressed dedup). Returns whether a new object was
    /// written (`false` means it was already present).
    pub fn write_object(&self, hash: &str, data: &[u8]) -> io::Result<bool> {
        let path = self.object_path(hash);
        if path.exists() {
            return Ok(false);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, data)?;
        Ok(true)
    }

    pub fn read_object(&self, hash: &str) -> io::Result<Vec<u8>> {
        fs::read(self.object_path(hash))
    }
}
