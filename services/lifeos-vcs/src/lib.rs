use std::io::Read;
use blake3::Hasher;
use fastcdc::v2020::FastCDC;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub hash: String,
    pub offset: u64,
    pub length: u32,
}

/// Computes the BLAKE3 hash of a byte slice
pub fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize().to_hex().to_string()
}

/// Chunks a reader using FastCDC and returns the list of chunks
pub fn chunk_reader<R: Read>(mut reader: R) -> std::io::Result<Vec<Chunk>> {
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    // Configure FastCDC with default 16KB-64KB-256KB constraints
    let min_size = 16_384;
    let avg_size = 65_536;
    let max_size = 262_144;

    let chunker = FastCDC::new(&buffer, min_size, avg_size, max_size);
    let mut chunks = Vec::new();

    for c in chunker {
        let chunk_data = &buffer[c.offset..c.offset + c.length];
        let hash = hash_bytes(chunk_data);
        chunks.push(Chunk {
            hash,
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
    fn test_hash_bytes() {
        let data = b"hello life-os";
        let h = hash_bytes(data);
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn test_chunking() {
        let mut data = vec![0u8; 200_000];
        // add some pattern
        for i in 0..data.len() {
            data[i] = (i % 251) as u8;
        }
        let chunks = chunk_reader(&data[..]).unwrap();
        assert!(!chunks.is_empty());
    }
}
