//! Thin client for the self-hosted Nango OAuth vault (infra/nango/,
//! docs/INTEGRATIONS.md). This is the only place lifeos-api holds Nango's
//! secret key; every route talks to Nango through this module and only ever
//! passes a `connectionId` back to the caller - never a token
//! (docs/SECURITY.md §1).

use crate::error::{ApiError, ApiResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
pub struct EndUser {
    pub id: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectSession {
    pub token: String,
}

/// Connection metadata only. Deliberately has no field for the underlying
/// provider token - Nango's proxy injects it server-side, it never reaches
/// this struct.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NangoConnection {
    pub connection_id: String,
    pub provider_config_key: String,
}

#[async_trait]
pub trait NangoClient: Send + Sync {
    async fn create_connect_session(
        &self,
        end_user: EndUser,
        allowed_integrations: Vec<String>,
    ) -> ApiResult<ConnectSession>;

    async fn get_connection(&self, connection_id: &str, provider_config_key: &str) -> ApiResult<NangoConnection>;

    async fn delete_connection(&self, connection_id: &str, provider_config_key: &str) -> ApiResult<()>;

    /// Call a provider endpoint through Nango's proxy - the token is injected
    /// server-side by Nango and never reaches this process's caller. Used by
    /// the per-provider thin tools (issue #53, docs/INTEGRATIONS.md) for both
    /// free reads and any write we choose not to gate at the entity layer.
    async fn proxy(
        &self,
        connection_id: &str,
        provider_config_key: &str,
        method: &str,
        endpoint: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> ApiResult<Value>;

    /// Raw-byte proxy for storage backends (issue #107,
    /// docs/STORAGE-BACKENDS.md §3): same server-side token injection as
    /// `proxy`, but carries opaque request/response bytes plus provider
    /// headers so blob content can move through Nango without JSON coercion.
    /// Returns the upstream status untranslated - the storage layer decides
    /// what a 404 means.
    async fn proxy_raw(
        &self,
        connection_id: &str,
        provider_config_key: &str,
        req: &lifeos_vcs::ProxyRequest,
    ) -> ApiResult<lifeos_vcs::ProxyResponse>;
}

/// Real implementation, calling the self-hosted Nango REST API.
pub struct HttpNangoClient {
    base_url: String,
    secret_key: String,
    http: reqwest::Client,
}

impl HttpNangoClient {
    pub fn new(base_url: String, secret_key: String) -> Self {
        Self { base_url, secret_key, http: reqwest::Client::new() }
    }
}

#[async_trait]
impl NangoClient for HttpNangoClient {
    async fn create_connect_session(
        &self,
        end_user: EndUser,
        allowed_integrations: Vec<String>,
    ) -> ApiResult<ConnectSession> {
        let resp = self
            .http
            .post(format!("{}/connect/sessions", self.base_url))
            .bearer_auth(&self.secret_key)
            .json(&json!({ "end_user": end_user, "allowed_integrations": allowed_integrations }))
            .send()
            .await
            .map_err(nango_unreachable)?;
        decode_ok(resp, "create_connect_session").await
    }

    async fn get_connection(&self, connection_id: &str, provider_config_key: &str) -> ApiResult<NangoConnection> {
        let resp = self
            .http
            .get(format!("{}/connections/{connection_id}", self.base_url))
            .query(&[("provider_config_key", provider_config_key)])
            .bearer_auth(&self.secret_key)
            .send()
            .await
            .map_err(nango_unreachable)?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ApiError::NotFound("nango connection not found".into()));
        }
        decode_ok(resp, "get_connection").await
    }

    async fn delete_connection(&self, connection_id: &str, provider_config_key: &str) -> ApiResult<()> {
        let resp = self
            .http
            .delete(format!("{}/connections/{connection_id}", self.base_url))
            .query(&[("provider_config_key", provider_config_key)])
            .bearer_auth(&self.secret_key)
            .send()
            .await
            .map_err(nango_unreachable)?;
        if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(())
        } else {
            tracing::error!("nango delete_connection returned {}", resp.status());
            Err(ApiError::Upstream("nango rejected delete_connection".into()))
        }
    }

    async fn proxy(
        &self,
        connection_id: &str,
        provider_config_key: &str,
        method: &str,
        endpoint: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> ApiResult<Value> {
        let http_method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| ApiError::BadRequest(format!("invalid proxy method '{method}'")))?;
        let mut req = self
            .http
            .request(http_method, format!("{}/proxy/{}", self.base_url, endpoint.trim_start_matches('/')))
            .header("Connection-Id", connection_id)
            .header("Provider-Config-Key", provider_config_key)
            .bearer_auth(&self.secret_key)
            .query(query);
        if let Some(b) = &body {
            req = req.json(b);
        }
        let resp = req.send().await.map_err(nango_unreachable)?;
        decode_ok(resp, "proxy").await
    }

    async fn proxy_raw(
        &self,
        connection_id: &str,
        provider_config_key: &str,
        preq: &lifeos_vcs::ProxyRequest,
    ) -> ApiResult<lifeos_vcs::ProxyResponse> {
        let http_method = reqwest::Method::from_bytes(preq.method.as_bytes())
            .map_err(|_| ApiError::BadRequest(format!("invalid proxy method '{}'", preq.method)))?;
        let mut req = self
            .http
            .request(
                http_method,
                format!("{}/proxy/{}", self.base_url, preq.endpoint.trim_start_matches('/')),
            )
            .header("Connection-Id", connection_id)
            .header("Provider-Config-Key", provider_config_key)
            .bearer_auth(&self.secret_key)
            .query(&preq.query);
        for (name, value) in &preq.headers {
            req = req.header(name, value);
        }
        if let Some(body) = &preq.body {
            req = req.body(body.clone());
        }
        let resp = req.send().await.map_err(nango_unreachable)?;
        let status = resp.status().as_u16();
        let body = resp.bytes().await.map_err(nango_unreachable)?.to_vec();
        Ok(lifeos_vcs::ProxyResponse { status, body })
    }
}

fn nango_unreachable(e: reqwest::Error) -> ApiError {
    tracing::error!("nango request failed: {e}");
    ApiError::Upstream("nango unreachable".into())
}

async fn decode_ok<T: serde::de::DeserializeOwned>(resp: reqwest::Response, op: &str) -> ApiResult<T> {
    if !resp.status().is_success() {
        tracing::error!("nango {op} returned {}", resp.status());
        return Err(ApiError::Upstream(format!("nango rejected {op}")));
    }
    resp.json().await.map_err(|e| {
        tracing::error!("nango {op} response decode failed: {e}");
        ApiError::Upstream("malformed nango response".into())
    })
}

/// In-memory fake used by tests so the HTTP surface can be exercised without
/// a real Nango deployment. Exposed unconditionally (not `#[cfg(test)]`) so
/// the `tests/` integration crate can construct one too.
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockNangoClient {
        connections: Mutex<std::collections::HashMap<String, NangoConnection>>,
        /// "{provider_config_key}:{method}:{endpoint}" -> canned response.
        proxy_responses: Mutex<std::collections::HashMap<String, Value>>,
        /// Every proxy call made, in order - lets tests assert a gated draft
        /// path never actually reached the provider.
        pub calls: Mutex<Vec<String>>,
    }

    impl MockNangoClient {
        pub fn new() -> Self {
            Self {
                connections: Mutex::new(std::collections::HashMap::new()),
                proxy_responses: Mutex::new(std::collections::HashMap::new()),
                calls: Mutex::new(Vec::new()),
            }
        }

        /// Seed a connection as if a real OAuth flow had already completed.
        pub fn seed(&self, connection_id: &str, provider_config_key: &str) {
            self.connections.lock().unwrap().insert(
                connection_id.to_string(),
                NangoConnection {
                    connection_id: connection_id.to_string(),
                    provider_config_key: provider_config_key.to_string(),
                },
            );
        }

        /// Seed the response a `proxy()` call should return for this exact
        /// (provider, method, endpoint) triple.
        pub fn seed_proxy(&self, provider_config_key: &str, method: &str, endpoint: &str, response: Value) {
            self.proxy_responses
                .lock()
                .unwrap()
                .insert(format!("{provider_config_key}:{method}:{endpoint}"), response);
        }
    }

    impl Default for MockNangoClient {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl NangoClient for MockNangoClient {
        async fn create_connect_session(
            &self,
            _end_user: EndUser,
            _allowed_integrations: Vec<String>,
        ) -> ApiResult<ConnectSession> {
            Ok(ConnectSession { token: "mock-session-token".into() })
        }

        async fn get_connection(&self, connection_id: &str, provider_config_key: &str) -> ApiResult<NangoConnection> {
            self.connections
                .lock()
                .unwrap()
                .get(connection_id)
                .filter(|c| c.provider_config_key == provider_config_key)
                .cloned()
                .ok_or_else(|| ApiError::NotFound("nango connection not found".into()))
        }

        async fn delete_connection(&self, connection_id: &str, _provider_config_key: &str) -> ApiResult<()> {
            self.connections.lock().unwrap().remove(connection_id);
            Ok(())
        }

        async fn proxy(
            &self,
            _connection_id: &str,
            provider_config_key: &str,
            method: &str,
            endpoint: &str,
            _query: &[(&str, &str)],
            _body: Option<Value>,
        ) -> ApiResult<Value> {
            let key = format!("{provider_config_key}:{method}:{endpoint}");
            self.calls.lock().unwrap().push(key.clone());
            self.proxy_responses
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| ApiError::Upstream(format!("no mock proxy response seeded for '{key}'")))
        }

        async fn proxy_raw(
            &self,
            _connection_id: &str,
            provider_config_key: &str,
            req: &lifeos_vcs::ProxyRequest,
        ) -> ApiResult<lifeos_vcs::ProxyResponse> {
            let key = format!("{provider_config_key}:{}:{}", req.method, req.endpoint);
            self.calls.lock().unwrap().push(key.clone());
            match self.proxy_responses.lock().unwrap().get(&key) {
                Some(value) => Ok(lifeos_vcs::ProxyResponse {
                    status: 200,
                    body: value.to_string().into_bytes(),
                }),
                None => Ok(lifeos_vcs::ProxyResponse { status: 404, body: vec![] }),
            }
        }
    }
}
