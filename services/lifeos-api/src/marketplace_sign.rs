//! ed25519 sign/verify for module marketplace manifests (issue #101,
//! `docs/PLATFORM-SYSTEMS.md`, `docs/SECURITY.md` §3). Publish signs a
//! manifest's canonical JSON bytes with the platform's signing key; install
//! (or any third party) verifies the signature against the publisher's
//! public key. A single byte of tamper in the manifest changes its bytes and
//! therefore fails verification - no separate "detect tampering" logic needed.

use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

/// Resolves the platform signing key from `LIFEOS_MARKETPLACE_SIGNING_SEED`
/// (base64, 32 raw bytes - `openssl rand -base64 32`). `None` means
/// marketplace publish/verify routes return `NotImplemented` rather than
/// signing with an implicit, unconfigured key (docs/SECURITY.md §3: "keys
/// managed securely").
pub fn parse_signing_key(base64_seed: &str) -> Result<SigningKey, String> {
    let bytes = STANDARD
        .decode(base64_seed.trim())
        .map_err(|e| format!("LIFEOS_MARKETPLACE_SIGNING_SEED is not valid base64: {e}"))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|v: Vec<u8>| format!("LIFEOS_MARKETPLACE_SIGNING_SEED must decode to 32 bytes, got {}", v.len()))?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Generates a fresh signing key. Used only by the CLI/setup helper that
/// prints a seed for `LIFEOS_MARKETPLACE_SIGNING_SEED` - never called at
/// request time.
#[allow(dead_code)]
pub fn generate_signing_key() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

pub fn public_key_b64(key: &SigningKey) -> String {
    STANDARD.encode(key.verifying_key().to_bytes())
}

/// Signs `manifest_bytes`, returning a base64 signature.
pub fn sign(key: &SigningKey, manifest_bytes: &[u8]) -> String {
    let sig: Signature = key.sign(manifest_bytes);
    STANDARD.encode(sig.to_bytes())
}

/// Verifies `signature_b64` over `manifest_bytes` against `pubkey_b64`.
/// Returns `false` (never panics/errors) on any malformed input - fail
/// closed, same posture as `crypto::decrypt`.
pub fn verify(pubkey_b64: &str, manifest_bytes: &[u8], signature_b64: &str) -> bool {
    let Ok(pubkey_bytes) = STANDARD.decode(pubkey_b64.trim()) else { return false };
    let Ok(pubkey_bytes): Result<[u8; 32], _> = pubkey_bytes.try_into() else { return false };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pubkey_bytes) else { return false };
    let Ok(sig_bytes) = STANDARD.decode(signature_b64.trim()) else { return false };
    let Ok(sig_bytes): Result<[u8; 64], _> = sig_bytes.try_into() else { return false };
    let signature = Signature::from_bytes(&sig_bytes);
    verifying_key.verify(manifest_bytes, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_and_verifies_a_manifest() {
        let key = generate_signing_key();
        let pubkey = public_key_b64(&key);
        let manifest = br#"{"id":"reading","version":"1.0.0"}"#;
        let sig = sign(&key, manifest);
        assert!(verify(&pubkey, manifest, &sig));
    }

    #[test]
    fn a_tampered_manifest_fails_verification() {
        let key = generate_signing_key();
        let pubkey = public_key_b64(&key);
        let manifest = br#"{"id":"reading","version":"1.0.0"}"#;
        let sig = sign(&key, manifest);
        let tampered = br#"{"id":"reading","version":"9.9.9"}"#;
        assert!(!verify(&pubkey, tampered, &sig));
    }

    #[test]
    fn a_wrong_pubkey_fails_verification() {
        let key = generate_signing_key();
        let other_pubkey = public_key_b64(&generate_signing_key());
        let manifest = br#"{"id":"reading"}"#;
        let sig = sign(&key, manifest);
        assert!(!verify(&other_pubkey, manifest, &sig));
    }

    #[test]
    fn parses_a_valid_base64_seed_deterministically() {
        let seed_b64 = STANDARD.encode([3u8; 32]);
        let a = parse_signing_key(&seed_b64).unwrap();
        let b = parse_signing_key(&seed_b64).unwrap();
        assert_eq!(a.to_bytes(), b.to_bytes());
    }

    #[test]
    fn rejects_malformed_signature_or_pubkey_without_panicking() {
        assert!(!verify("not-base64!!", b"x", "also-not-base64!!"));
        assert!(!verify(&STANDARD.encode([1u8; 32]), b"x", "short"));
    }
}
