use std::io;

use fastcdc::v2020::FastCDC;
use serde::{Deserialize, Serialize};

use crate::hash::hash_bytes;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkRef {
    pub hash: String,
    pub offset: u64,
    pub length: u32,
}

/// Splits `data` into content-defined chunks via FastCDC (16KB-64KB-256KB
/// default constraints, docs/VERSIONING.md §2.2) and hashes each one.
pub fn chunk_reader(data: &[u8]) -> io::Result<Vec<ChunkRef>> {
    let min_size = 16_384;
    let avg_size = 65_536;
    let max_size = 262_144;

    let chunker = FastCDC::new(data, min_size, avg_size, max_size);
    let mut chunks = Vec::new();

    for c in chunker {
        let chunk_data = &data[c.offset..c.offset + c.length];
        chunks.push(ChunkRef {
            hash: hash_bytes(chunk_data),
            offset: c.offset as u64,
            length: c.length as u32,
        });
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunking() {
        let mut data = vec![0u8; 200_000];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }
        let chunks = chunk_reader(&data).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_chunking_empty_input_yields_no_chunks() {
        let chunks = chunk_reader(&[]).unwrap();
        assert!(chunks.is_empty());
    }
}
