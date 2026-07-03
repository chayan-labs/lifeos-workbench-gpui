use std::io;

use serde::{Deserialize, Serialize};

use crate::chunk::{chunk_reader, ChunkRef};
use crate::hash::hash_bytes;
use crate::store::ObjectStore;

/// A blob is a Merkle list of chunk hashes (docs/VERSIONING.md §2.2). The
/// blob's own content-address (`blob_ref`) is the hash of this manifest, not
/// the hash of the raw bytes - that's what makes chunk-level dedup possible
/// for re-exports that only change part of a large file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobManifest {
    pub chunks: Vec<ChunkRef>,
}

/// Chunks `data`, stores each chunk plus the manifest itself as objects
/// (each write is a no-op if the hash already exists - content-addressed
/// dedup applies at both the chunk and the blob level), and returns the
/// blob's content address.
pub fn store_blob(store: &ObjectStore, data: &[u8]) -> io::Result<String> {
    let chunks = chunk_reader(data)?;

    let mut chunk_refs = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        let bytes = &data[chunk.offset as usize..(chunk.offset + chunk.length as u64) as usize];
        store.write_object(&chunk.hash, bytes)?;
        chunk_refs.push(chunk.clone());
    }

    let manifest = BlobManifest { chunks: chunk_refs };
    let manifest_json = serde_json::to_vec(&manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let blob_ref = hash_bytes(&manifest_json);
    store.write_object(&blob_ref, &manifest_json)?;

    Ok(blob_ref)
}

/// Reads the manifest for `blob_ref` and reassembles the original bytes from
/// its chunks, in order.
pub fn read_blob(store: &ObjectStore, blob_ref: &str) -> io::Result<Vec<u8>> {
    let manifest_json = store.read_object(blob_ref)?;
    let manifest: BlobManifest = serde_json::from_slice(&manifest_json)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut out = Vec::new();
    for chunk in &manifest.chunks {
        out.extend(store.read_object(&chunk.hash)?);
    }
    Ok(out)
}
