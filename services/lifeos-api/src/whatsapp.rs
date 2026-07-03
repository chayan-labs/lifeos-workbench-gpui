//! WhatsApp via a self-hosted GOWA instance
//! (github.com/aldinokemal/go-whatsapp-web-multidevice, `infra/gowa/`,
//! docs/INTEGRATIONS.md) - a REST wrapper around `whatsmeow` with a real
//! multi-tenant device API. No Meta app, no OAuth; devices pair by scanning
//! a QR like WhatsApp Web.
//!
//! We register one GOWA "device" per workspace with `device_id = workspace_id`,
//! and GOWA's webhook payloads carry that same value back as `session_id`
//! (confirmed against GOWA's `webhook_forward.go`), so inbound events route
//! to the right workspace with no lookup table on our side.
//!
//! Deliberately no `send` method on this trait: sending is a gated action
//! (docs/SECURITY.md §2) that only produces a draft entity in
//! `routes/whatsapp.rs`, never calls GOWA. The same "the capability doesn't
//! structurally exist here" guarantee as `kite::KiteClient`.

use crate::error::{ApiError, ApiResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

#[async_trait]
pub trait WhatsAppClient: Send + Sync {
    /// Create a device slot keyed by our own id (`device_id = workspace_id`).
    async fn create_device(&self, device_id: &str) -> ApiResult<()>;

    /// Start QR-code pairing for a device; returns a link to the QR image
    /// GOWA serves itself.
    async fn login_qr(&self, device_id: &str) -> ApiResult<String>;

    /// Whether this device is fully paired (`state == "logged_in"`).
    async fn status(&self, device_id: &str) -> ApiResult<bool>;
}

/// Real implementation, calling a self-hosted GOWA REST API.
pub struct HttpWhatsAppClient {
    base_url: String,
    /// `(user, pass)` for GOWA's server-wide Basic Auth.
    basic_auth: (String, String),
    http: reqwest::Client,
}

impl HttpWhatsAppClient {
    /// `basic_auth` is the raw `"user:pass"` string from config.
    pub fn new(base_url: String, basic_auth: String) -> Self {
        let (user, pass) = basic_auth.split_once(':').unwrap_or((basic_auth.as_str(), ""));
        Self {
            base_url,
            basic_auth: (user.to_string(), pass.to_string()),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl WhatsAppClient for HttpWhatsAppClient {
    async fn create_device(&self, device_id: &str) -> ApiResult<()> {
        let resp = self
            .http
            .post(format!("{}/devices", self.base_url))
            .basic_auth(&self.basic_auth.0, Some(&self.basic_auth.1))
            .json(&json!({ "device_id": device_id }))
            .send()
            .await
            .map_err(gowa_unreachable)?;
        expect_ok(resp, "create_device").await
    }

    async fn login_qr(&self, device_id: &str) -> ApiResult<String> {
        let resp = self
            .http
            .get(format!("{}/devices/{device_id}/login", self.base_url))
            .basic_auth(&self.basic_auth.0, Some(&self.basic_auth.1))
            .send()
            .await
            .map_err(gowa_unreachable)?;
        if !resp.status().is_success() {
            tracing::error!("gowa login returned {}", resp.status());
            return Err(ApiError::Upstream("gowa rejected login request".into()));
        }
        #[derive(Deserialize)]
        struct LoginResponse {
            results: LoginResults,
        }
        #[derive(Deserialize)]
        struct LoginResults {
            qr_link: String,
        }
        resp.json::<LoginResponse>().await.map(|r| r.results.qr_link).map_err(|e| {
            tracing::error!("gowa login response decode failed: {e}");
            ApiError::Upstream("malformed gowa response".into())
        })
    }

    async fn status(&self, device_id: &str) -> ApiResult<bool> {
        let resp = self
            .http
            .get(format!("{}/devices/{device_id}", self.base_url))
            .basic_auth(&self.basic_auth.0, Some(&self.basic_auth.1))
            .send()
            .await
            .map_err(gowa_unreachable)?;
        if !resp.status().is_success() {
            tracing::error!("gowa device status returned {}", resp.status());
            return Err(ApiError::Upstream("gowa rejected device status request".into()));
        }
        #[derive(Deserialize)]
        struct DeviceInfoResponse {
            results: DeviceInfo,
        }
        #[derive(Deserialize)]
        struct DeviceInfo {
            state: String,
        }
        resp.json::<DeviceInfoResponse>()
            .await
            .map(|r| r.results.state == "logged_in")
            .map_err(|e| {
                tracing::error!("gowa device status response decode failed: {e}");
                ApiError::Upstream("malformed gowa response".into())
            })
    }
}

fn gowa_unreachable(e: reqwest::Error) -> ApiError {
    tracing::error!("gowa request failed: {e}");
    ApiError::Upstream("gowa unreachable".into())
}

async fn expect_ok(resp: reqwest::Response, op: &str) -> ApiResult<()> {
    if resp.status().is_success() {
        Ok(())
    } else {
        tracing::error!("gowa {op} returned {}", resp.status());
        Err(ApiError::Upstream(format!("gowa rejected {op}")))
    }
}

/// In-memory fake for tests - no real GOWA instance/phone needed to
/// exercise the API surface. Exposed unconditionally so `tests/` can
/// construct one too.
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct MockWhatsAppClient {
        /// device_id -> registered.
        devices: Mutex<std::collections::HashMap<String, ()>>,
        /// device_id -> qr_link to hand back.
        qr_links: Mutex<std::collections::HashMap<String, String>>,
        /// device_id -> logged_in?
        logged_in: Mutex<std::collections::HashMap<String, bool>>,
        /// Every call made, in order - lets tests assert no send-shaped call
        /// was ever made (this trait has no send method, but this also
        /// documents exactly what *was* called for other assertions).
        pub calls: Mutex<Vec<String>>,
    }

    impl MockWhatsAppClient {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn seed_qr(&self, device_id: &str, qr_link: &str) {
            self.qr_links.lock().unwrap().insert(device_id.to_string(), qr_link.to_string());
        }

        pub fn seed_logged_in(&self, device_id: &str, logged_in: bool) {
            self.logged_in.lock().unwrap().insert(device_id.to_string(), logged_in);
        }

        /// device_ids registered so far - test-only introspection.
        pub fn devices_snapshot(&self) -> Vec<String> {
            self.devices.lock().unwrap().keys().cloned().collect()
        }
    }

    #[async_trait]
    impl WhatsAppClient for MockWhatsAppClient {
        async fn create_device(&self, device_id: &str) -> ApiResult<()> {
            self.calls.lock().unwrap().push("create_device".into());
            self.devices.lock().unwrap().insert(device_id.to_string(), ());
            Ok(())
        }

        async fn login_qr(&self, device_id: &str) -> ApiResult<String> {
            self.calls.lock().unwrap().push("login_qr".into());
            self.qr_links
                .lock()
                .unwrap()
                .get(device_id)
                .cloned()
                .ok_or_else(|| ApiError::Upstream("gowa rejected login request".into()))
        }

        async fn status(&self, device_id: &str) -> ApiResult<bool> {
            self.calls.lock().unwrap().push("status".into());
            Ok(self.logged_in.lock().unwrap().get(device_id).copied().unwrap_or(false))
        }
    }
}
