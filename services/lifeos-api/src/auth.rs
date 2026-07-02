//! Soft, upgrade-ready auth.
//!
//! `register`/`login` issue a `key_token` (an HS256 JWT carrying the user +
//! workspace). Requests MAY present it as `Authorization: Bearer <token>`.
//! When they do, the workspace is taken from the verified claim. When they
//! don't, we fall back to an explicit `workspace_id` (header/query/body) and
//! finally to the seeded default workspace - so the current frontend (which
//! sends no token yet) keeps working while the JWT path is ready to be
//! enforced later.
//!
//! Issue #100 adds the real credential layer behind that token: password
//! hashing (`hash_password`/`verify_password`, argon2id) and session/
//! refresh-token rotation (`new_refresh_token`/`hash_refresh_token`). The
//! access token itself (`Claims`, `issue_token`, `verify_token`,
//! `resolve_workspace`) is intentionally UNCHANGED - every route that
//! already calls `resolve_workspace` keeps working untouched; sessions are a
//! new stateful layer behind it, not a replacement for it.

use crate::config::DEFAULT_WORKSPACE;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::http::HeaderMap;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 30 days, in seconds. Long-lived because this is a local-first personal tool;
/// SaaS hardening (Phase 7) swaps this for real sessions.
const TOKEN_TTL_SECS: i64 = 60 * 60 * 24 * 30;

/// Refresh tokens are opaque and shorter-lived than the access token they
/// mint - the whole point of rotation is that a leaked refresh token has a
/// bounded window, unlike the 30-day access JWT above.
pub const REFRESH_TOKEN_TTL_SECS: i64 = 60 * 60 * 24 * 14;

/// Argon2id-hash a plaintext password for storage in `users.password_hash`.
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("password hashing failed: {e}"))
}

/// Constant-time verification of a plaintext password against a stored hash.
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// A random 256-bit opaque refresh token, hex-encoded. Never stored in
/// plaintext - only `hash_refresh_token`'s output goes in `sessions`.
pub fn new_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// SHA-256 of a refresh token, for storage/lookup in
/// `sessions.refresh_token_hash`. Refresh tokens are high-entropy random
/// bytes (unlike passwords), so a fast hash is fine here - the threat this
/// defends against is DB-read exposure, not brute force.
pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,          // user_id
    pub workspace_id: String, // tenant
    pub email: String,
    pub exp: usize,
}

pub fn issue_token(secret: &str, user_id: &str, workspace_id: &str, email: &str) -> String {
    let exp = (crate::ids::now_secs() + TOKEN_TTL_SECS) as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        workspace_id: workspace_id.to_string(),
        email: email.to_string(),
        exp,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .unwrap_or_else(|e| {
            tracing::error!("failed to sign key_token: {e}");
            String::new()
        })
}

pub fn verify_token(secret: &str, token: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .ok()
}

/// Returns the verified claims from the request's bearer token, if present
/// and valid. Used by `/api/me` to surface the authenticated user.
pub fn bearer_claims(headers: &HeaderMap, secret: &str) -> Option<Claims> {
    bearer(headers).and_then(|token| verify_token(secret, &token))
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
}

/// Resolve the workspace a request operates on, in priority order:
/// verified JWT claim > `X-Workspace-Id` header > explicit param > default.
pub fn resolve_workspace(headers: &HeaderMap, secret: &str, explicit: Option<&str>) -> String {
    if let Some(token) = bearer(headers) {
        if let Some(claims) = verify_token(secret, &token) {
            return claims.workspace_id;
        }
    }
    if let Some(h) = headers.get("x-workspace-id").and_then(|v| v.to_str().ok()) {
        if !h.is_empty() {
            return h.to_string();
        }
    }
    match explicit {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => DEFAULT_WORKSPACE.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_round_trips() {
        let secret = "test-secret";
        let token = issue_token(secret, "usr_1", "ws_1", "a@b.c");
        let claims = verify_token(secret, &token).expect("valid token");
        assert_eq!(claims.sub, "usr_1");
        assert_eq!(claims.workspace_id, "ws_1");
    }

    #[test]
    fn wrong_secret_rejects() {
        let token = issue_token("secret-a", "usr_1", "ws_1", "a@b.c");
        assert!(verify_token("secret-b", &token).is_none());
    }

    #[test]
    fn resolve_prefers_token_then_header_then_explicit_then_default() {
        let secret = "s";
        // default
        let h = HeaderMap::new();
        assert_eq!(resolve_workspace(&h, secret, None), DEFAULT_WORKSPACE);
        // explicit
        assert_eq!(resolve_workspace(&h, secret, Some("ws_x")), "ws_x");
        // header beats explicit
        let mut h2 = HeaderMap::new();
        h2.insert("x-workspace-id", "ws_h".parse().unwrap());
        assert_eq!(resolve_workspace(&h2, secret, Some("ws_x")), "ws_h");
        // token beats header
        let token = issue_token(secret, "u", "ws_tok", "e");
        let mut h3 = HeaderMap::new();
        h3.insert("x-workspace-id", "ws_h".parse().unwrap());
        h3.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
        assert_eq!(resolve_workspace(&h3, secret, Some("ws_x")), "ws_tok");
    }

    #[test]
    fn password_hash_roundtrips_and_rejects_wrong_password() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn password_hash_is_salted_so_identical_passwords_hash_differently() {
        let a = hash_password("same-password").unwrap();
        let b = hash_password("same-password").unwrap();
        assert_ne!(a, b, "argon2 must salt each hash independently");
        assert!(verify_password("same-password", &a));
        assert!(verify_password("same-password", &b));
    }

    #[test]
    fn garbage_stored_hash_fails_closed_rather_than_panicking() {
        assert!(!verify_password("anything", "not-a-real-argon2-hash"));
    }

    #[test]
    fn refresh_tokens_are_random_and_their_hash_is_deterministic() {
        let a = new_refresh_token();
        let b = new_refresh_token();
        assert_ne!(a, b);
        assert_eq!(hash_refresh_token(&a), hash_refresh_token(&a));
        assert_ne!(hash_refresh_token(&a), hash_refresh_token(&b));
    }
}
