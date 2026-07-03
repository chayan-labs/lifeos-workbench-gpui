//! Zerodha Kite Connect - a native custom connector (Nango doesn't model
//! Kite's daily request-token dance), issue #51, docs/INTEGRATIONS.md §3.
//!
//! Deliberately READ-ONLY: this trait exposes no place/modify/cancel/GTT
//! method, and never will (docs/SECURITY.md §1 - "no order tool registered
//! anywhere"). `broker-guard` is the belt to this module's suspenders: even
//! if some future agent tool tried to call an order endpoint, no such
//! endpoint exists here to call.

use crate::error::{ApiError, ApiResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Deserialize)]
pub struct KiteSession {
    pub access_token: String,
    pub user_id: String,
}

#[async_trait]
pub trait KiteClient: Send + Sync {
    /// Exchange the daily `request_token` (from Kite's login redirect) for an
    /// `access_token`. Kite requires `checksum = sha256(api_key + request_token
    /// + api_secret)` as a tamper check on this call.
    async fn generate_session(&self, request_token: &str) -> ApiResult<KiteSession>;

    /// Read-only positions snapshot. The only market-data method this trait
    /// exposes for the base - deliberately no write/order capability exists.
    async fn positions(&self, access_token: &str) -> ApiResult<Value>;
}

pub fn login_url(api_key: &str) -> String {
    format!("https://kite.zerodha.com/connect/login?api_key={api_key}&v=3")
}

/// Real implementation, calling the Kite Connect REST API.
pub struct HttpKiteClient {
    api_key: String,
    api_secret: String,
    http: reqwest::Client,
}

impl HttpKiteClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self { api_key, api_secret, http: reqwest::Client::new() }
    }

    fn checksum(&self, request_token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.api_key.as_bytes());
        hasher.update(request_token.as_bytes());
        hasher.update(self.api_secret.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[async_trait]
impl KiteClient for HttpKiteClient {
    async fn generate_session(&self, request_token: &str) -> ApiResult<KiteSession> {
        let checksum = self.checksum(request_token);
        let resp = self
            .http
            .post("https://api.kite.trade/session/token")
            .header("X-Kite-Version", "3")
            .form(&[
                ("api_key", self.api_key.as_str()),
                ("request_token", request_token),
                ("checksum", checksum.as_str()),
            ])
            .send()
            .await
            .map_err(kite_unreachable)?;
        if !resp.status().is_success() {
            tracing::error!("kite session/token returned {}", resp.status());
            return Err(ApiError::Upstream("kite rejected request_token".into()));
        }
        #[derive(Deserialize)]
        struct Envelope {
            data: KiteSession,
        }
        resp.json::<Envelope>()
            .await
            .map(|e| e.data)
            .map_err(|e| {
                tracing::error!("kite session/token response decode failed: {e}");
                ApiError::Upstream("malformed kite response".into())
            })
    }

    async fn positions(&self, access_token: &str) -> ApiResult<Value> {
        let resp = self
            .http
            .get("https://api.kite.trade/portfolio/positions")
            .header("X-Kite-Version", "3")
            .header("Authorization", format!("token {}:{access_token}", self.api_key))
            .send()
            .await
            .map_err(kite_unreachable)?;
        if !resp.status().is_success() {
            tracing::error!("kite positions returned {}", resp.status());
            return Err(ApiError::Upstream("kite rejected positions request".into()));
        }
        resp.json().await.map_err(|e| {
            tracing::error!("kite positions response decode failed: {e}");
            ApiError::Upstream("malformed kite response".into())
        })
    }
}

fn kite_unreachable(e: reqwest::Error) -> ApiError {
    tracing::error!("kite request failed: {e}");
    ApiError::Upstream("kite unreachable".into())
}

/// In-memory fake for tests - no real Kite account/app needed to exercise the
/// API surface. Exposed unconditionally so `tests/` can construct one too.
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockKiteClient {
        /// request_token -> session Kite would have returned.
        sessions: Mutex<std::collections::HashMap<String, KiteSession>>,
        /// access_token -> positions payload Kite would have returned.
        positions: Mutex<std::collections::HashMap<String, Value>>,
    }

    impl MockKiteClient {
        pub fn new() -> Self {
            Self { sessions: Mutex::new(std::collections::HashMap::new()), positions: Mutex::new(std::collections::HashMap::new()) }
        }

        pub fn seed_session(&self, request_token: &str, access_token: &str, user_id: &str) {
            self.sessions.lock().unwrap().insert(
                request_token.to_string(),
                KiteSession { access_token: access_token.to_string(), user_id: user_id.to_string() },
            );
        }

        pub fn seed_positions(&self, access_token: &str, payload: Value) {
            self.positions.lock().unwrap().insert(access_token.to_string(), payload);
        }
    }

    impl Default for MockKiteClient {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl KiteClient for MockKiteClient {
        async fn generate_session(&self, request_token: &str) -> ApiResult<KiteSession> {
            self.sessions
                .lock()
                .unwrap()
                .get(request_token)
                .cloned()
                .ok_or_else(|| ApiError::Upstream("kite rejected request_token".into()))
        }

        async fn positions(&self, access_token: &str) -> ApiResult<Value> {
            self.positions
                .lock()
                .unwrap()
                .get(access_token)
                .cloned()
                .ok_or_else(|| ApiError::Upstream("kite rejected positions request".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_url_carries_api_key_and_version() {
        let url = login_url("myapikey");
        assert!(url.starts_with("https://kite.zerodha.com/connect/login?"));
        assert!(url.contains("api_key=myapikey"));
        assert!(url.contains("v=3"));
    }

    #[test]
    fn checksum_is_stable_sha256_of_concatenation() {
        let client = HttpKiteClient::new("key".into(), "secret".into());
        let got = client.checksum("token");
        let mut hasher = Sha256::new();
        hasher.update(b"keytokensecret");
        let want = hex::encode(hasher.finalize());
        assert_eq!(got, want);
    }
}
