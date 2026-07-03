use blake3::Hasher;

/// Computes the BLAKE3 hash of a byte slice, hex-encoded.
pub fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize().to_hex().to_string()
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
    fn test_hash_bytes_is_deterministic() {
        let data = b"hello life-os";
        assert_eq!(hash_bytes(data), hash_bytes(data));
    }
}
