//! Storage-backend factory (issue #107, docs/STORAGE-BACKENDS.md §3-§4).
//!
//! Turns a workspace's `storage_backend` config entity into a live
//! `lifeos_vcs::StorageBackend`:
//! - `local-fs` - the default; today's `objects/<hh>/<hash>` under
//!   `vcs_blob_root`. With no config, behavior is identical to before.
//! - `s3` / `r2` / `gcs` / `azure` - user's own keys, envelope-encrypted in
//!   `connections.secret_enc`, one `object_store` impl.
//! - `google-drive` / `dropbox` / `onedrive` - through the Nango proxy; the
//!   backend holds only a `connectionId`, never a token.
//! - `webdav` - self-hosted-friendly, keys in `connections.secret_enc`.
//!
//! Credentials are decrypted here, injected into the transport, and never
//! serialized into entities, events, logs, or agent context
//! (docs/SECURITY.md §1).

use crate::crypto;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use async_trait::async_trait;
use lifeos_vcs::{
    BackendError, BackendProxy, ExternalObjectStoreBackend, LocalFsBackend, ProxiedFileBackend,
    ProxyProvider, ProxyRequest, ProxyResponse, StorageBackend,
};
use serde_json::Value;
use std::sync::Arc;

/// Every backend kind a `storage_backend` config entity may declare.
pub const STORAGE_KINDS: &[&str] =
    &["local-fs", "s3", "r2", "gcs", "azure", "google-drive", "dropbox", "onedrive", "webdav"];

/// [`BackendProxy`] over Nango: token injected server-side by Nango, this
/// struct holds only the connection handle.
struct NangoBackendProxy {
    nango: Arc<dyn crate::nango::NangoClient>,
    connection_id: String,
    provider_config_key: String,
}

#[async_trait]
impl BackendProxy for NangoBackendProxy {
    async fn execute(&self, req: ProxyRequest) -> Result<ProxyResponse, BackendError> {
        self.nango
            .proxy_raw(&self.connection_id, &self.provider_config_key, &req)
            .await
            .map_err(|e| BackendError::Other(format!("nango proxy failed: {e:?}")))
    }
}

/// [`BackendProxy`] speaking plain WebDAV with basic auth from decrypted
/// `secret_enc` keys. The credentials live only inside this struct.
struct WebDavProxy {
    http: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

#[async_trait]
impl BackendProxy for WebDavProxy {
    async fn execute(&self, preq: ProxyRequest) -> Result<ProxyResponse, BackendError> {
        let method = reqwest::Method::from_bytes(preq.method.as_bytes())
            .map_err(|_| BackendError::Other(format!("invalid webdav method '{}'", preq.method)))?;
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), preq.endpoint.trim_start_matches('/'));
        let mut req = self
            .http
            .request(method, url)
            .basic_auth(&self.username, Some(&self.password))
            .query(&preq.query);
        for (name, value) in &preq.headers {
            req = req.header(name, value);
        }
        if let Some(body) = &preq.body {
            req = req.body(body.clone());
        }
        let resp = req
            .send()
            .await
            .map_err(|e| BackendError::Other(format!("webdav unreachable: {e}")))?;
        let status = resp.status().as_u16();
        let body = resp
            .bytes()
            .await
            .map_err(|e| BackendError::Other(format!("webdav body read failed: {e}")))?
            .to_vec();
        Ok(ProxyResponse { status, body })
    }
}

/// The connection row a backend config points at: Nango handle or decrypted
/// non-Nango keys, resolved fresh per construction and dropped after.
struct ConnectionAuth {
    nango_connection_id: Option<String>,
    secrets: Option<Value>,
}

async fn connection_auth(state: &AppState, workspace_id: &str, connection_id: &str) -> ApiResult<ConnectionAuth> {
    let mut rows = state
        .conn
        .query(
            "SELECT nango_connection_id, secret_enc FROM connections \
             WHERE id = ?1 AND workspace_id = ?2 AND status = 'active'",
            libsql::params![connection_id, workspace_id],
        )
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("no active connection '{connection_id}'")))?;
    let nango_connection_id: Option<String> = row.get(0)?;
    let secret_enc: Option<String> = row.get(1)?;

    let secrets = match secret_enc {
        Some(blob) => {
            let key = state
                .config
                .secret_encryption_key
                .as_ref()
                .ok_or_else(|| ApiError::NotImplemented("LIFEOS_SECRET_ENCRYPTION_KEY is not configured".into()))?;
            let plaintext = crypto::decrypt(&blob, key)?;
            Some(serde_json::from_str(&plaintext).map_err(|_| ApiError::Internal("secret_enc is not valid JSON".into()))?)
        }
        None => None,
    };
    Ok(ConnectionAuth { nango_connection_id, secrets })
}

fn required<'a>(secrets: &'a Value, field: &str, kind: &str) -> ApiResult<&'a str> {
    secrets[field]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::BadRequest(format!("'{kind}' backend keys are missing '{field}'")))
}

/// One `object_store` impl covers all four bucket providers; only the
/// builder differs (docs/STORAGE-BACKENDS.md §2).
fn bucket_store(kind: &str, secrets: &Value) -> ApiResult<Arc<dyn object_store::ObjectStore>> {
    let store: Arc<dyn object_store::ObjectStore> = match kind {
        "s3" | "r2" => {
            let mut builder = object_store::aws::AmazonS3Builder::new()
                .with_bucket_name(required(secrets, "bucket", kind)?)
                .with_access_key_id(required(secrets, "access_key_id", kind)?)
                .with_secret_access_key(required(secrets, "secret_access_key", kind)?)
                .with_region(secrets["region"].as_str().unwrap_or("auto"))
                .with_virtual_hosted_style_request(false);
            if let Some(endpoint) = secrets["endpoint"].as_str() {
                builder = builder.with_endpoint(endpoint);
            }
            Arc::new(builder.build().map_err(bad_keys(kind))?)
        }
        "gcs" => Arc::new(
            object_store::gcp::GoogleCloudStorageBuilder::new()
                .with_bucket_name(required(secrets, "bucket", kind)?)
                .with_service_account_key(required(secrets, "service_account_key", kind)?)
                .build()
                .map_err(bad_keys(kind))?,
        ),
        "azure" => Arc::new(
            object_store::azure::MicrosoftAzureBuilder::new()
                .with_account(required(secrets, "account", kind)?)
                .with_access_key(required(secrets, "access_key", kind)?)
                .with_container_name(required(secrets, "container", kind)?)
                .build()
                .map_err(bad_keys(kind))?,
        ),
        other => return Err(ApiError::BadRequest(format!("'{other}' is not a bucket backend"))),
    };
    Ok(store)
}

fn bad_keys(kind: &str) -> impl Fn(object_store::Error) -> ApiError + '_ {
    move |e| ApiError::BadRequest(format!("invalid '{kind}' backend keys: {e}"))
}

fn nango_provider(kind: &str) -> Option<ProxyProvider> {
    match kind {
        "google-drive" => Some(ProxyProvider::GoogleDrive),
        "dropbox" => Some(ProxyProvider::Dropbox),
        "onedrive" => Some(ProxyProvider::OneDrive),
        _ => None,
    }
}

/// Wraps `backend` in client-side envelope encryption (issue #110) when the
/// config asks for it: the provider stores ciphertext only, under the
/// per-workspace envelope key. Hashes stay computed over plaintext, so the
/// `blob_ref` is unchanged by the wrap.
async fn maybe_encrypt(
    state: &AppState,
    workspace_id: &str,
    attrs: &Value,
    backend: Arc<dyn StorageBackend>,
) -> ApiResult<Arc<dyn StorageBackend>> {
    if attrs["encryption"].as_bool() != Some(true) {
        return Ok(backend);
    }
    let master_key = state
        .config
        .secret_encryption_key
        .as_ref()
        .ok_or_else(|| ApiError::NotImplemented("LIFEOS_SECRET_ENCRYPTION_KEY is not configured".into()))?;
    let envelope_key = crypto::ensure_envelope_key(&state.conn, master_key, workspace_id).await?;
    Ok(Arc::new(lifeos_vcs::EncryptedBackend::new(backend, envelope_key)))
}

/// Builds the live backend for one `storage_backend` config entity.
pub async fn backend_from_config(
    state: &AppState,
    workspace_id: &str,
    backend_id: &str,
    attrs: &Value,
) -> ApiResult<Arc<dyn StorageBackend>> {
    let kind = attrs["kind"].as_str().unwrap_or("local-fs");
    if kind == "local-fs" {
        // `folder` optionally points local-fs at a user-chosen directory
        // (e.g. an external disk); default stays the API's blob root.
        let root = attrs["folder"]
            .as_str()
            .filter(|f| !f.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| state.config.vcs_blob_root.clone());
        let backend: Arc<dyn StorageBackend> = Arc::new(LocalFsBackend::new(root));
        return maybe_encrypt(state, workspace_id, attrs, backend).await;
    }
    let connection_id = attrs["connection_id"]
        .as_str()
        .ok_or_else(|| ApiError::BadRequest(format!("'{kind}' backend config has no connection_id")))?;
    let auth = connection_auth(state, workspace_id, connection_id).await?;
    let folder = attrs["folder"].as_str().unwrap_or("lifeos").to_string();

    if let Some(provider) = nango_provider(kind) {
        let nango = state
            .nango
            .clone()
            .ok_or_else(|| ApiError::NotImplemented("Nango is not configured - see docs/MANUAL-SETUP.md".into()))?;
        let nango_connection_id = auth
            .nango_connection_id
            .ok_or_else(|| ApiError::BadRequest(format!("connection '{connection_id}' has no Nango handle")))?;
        let proxy = Arc::new(NangoBackendProxy {
            nango,
            connection_id: nango_connection_id,
            provider_config_key: kind.to_string(),
        });
        let backend: Arc<dyn StorageBackend> = Arc::new(ProxiedFileBackend::new(
            provider,
            proxy,
            (*state.conn).clone(),
            workspace_id,
            backend_id,
            folder,
        ));
        return maybe_encrypt(state, workspace_id, attrs, backend).await;
    }

    let secrets = auth
        .secrets
        .ok_or_else(|| ApiError::BadRequest(format!("connection '{connection_id}' holds no keys for '{kind}'")))?;
    if kind == "webdav" {
        let proxy = Arc::new(WebDavProxy {
            http: reqwest::Client::new(),
            base_url: required(&secrets, "base_url", kind)?.to_string(),
            username: required(&secrets, "username", kind)?.to_string(),
            password: secrets["password"].as_str().unwrap_or_default().to_string(),
        });
        let backend: Arc<dyn StorageBackend> = Arc::new(ProxiedFileBackend::new(
            ProxyProvider::WebDav,
            proxy,
            (*state.conn).clone(),
            workspace_id,
            backend_id,
            folder,
        ));
        return maybe_encrypt(state, workspace_id, attrs, backend).await;
    }
    let backend: Arc<dyn StorageBackend> = Arc::new(ExternalObjectStoreBackend::new(bucket_store(kind, &secrets)?));
    maybe_encrypt(state, workspace_id, attrs, backend).await
}

/// The workspace's active storage backends: the primary (writes) first,
/// then mirrors - each as `(entity_id, attrs)`. Empty when nothing is
/// configured (callers fall back to local-fs, so nothing regresses).
pub async fn active_backend_configs(state: &AppState, workspace_id: &str) -> ApiResult<Vec<(String, Value)>> {
    let mut rows = state
        .conn
        .query(
            "SELECT id, attrs FROM entities \
             WHERE workspace_id = ?1 AND module = 'storage' AND type = 'storage_backend' AND status = 'active' \
             ORDER BY json_extract(attrs, '$.default') DESC, updated_at DESC",
            libsql::params![workspace_id],
        )
        .await?;
    let mut configs = Vec::new();
    while let Some(row) = rows.next().await? {
        let id: String = row.get(0)?;
        let attrs_str: String = row.get(1)?;
        let attrs: Value = serde_json::from_str(&attrs_str).unwrap_or_default();
        configs.push((id, attrs));
    }
    Ok(configs)
}

/// The backend blob reads/writes should use: the workspace's active default
/// config, or local-fs when none is configured.
pub async fn primary_backend(state: &AppState, workspace_id: &str) -> ApiResult<Arc<dyn StorageBackend>> {
    match active_backend_configs(state, workspace_id).await?.into_iter().next() {
        Some((id, attrs)) => backend_from_config(state, workspace_id, &id, &attrs).await,
        None => Ok(Arc::new(LocalFsBackend::new(state.config.vcs_blob_root.clone()))),
    }
}

/// Every backend a read may consult, in fallback order: the local CAS first
/// (cache/default), then the primary, then mirrors (issue #108).
pub async fn read_backends(state: &AppState, workspace_id: &str) -> ApiResult<Vec<Arc<dyn StorageBackend>>> {
    let mut backends: Vec<Arc<dyn StorageBackend>> =
        vec![Arc::new(LocalFsBackend::new(state.config.vcs_blob_root.clone()))];
    for (id, attrs) in active_backend_configs(state, workspace_id).await? {
        backends.push(backend_from_config(state, workspace_id, &id, &attrs).await?);
    }
    Ok(backends)
}

/// Fetches a whole blob by `blob_ref` with cross-backend fallback - the
/// frontend's blob fetch (issue #109) and any other by-hash content read.
pub async fn read_blob(state: &AppState, workspace_id: &str, blob_ref: &str) -> ApiResult<Vec<u8>> {
    let backends = read_backends(state, workspace_id).await?;
    lifeos_vcs::read_blob_with_fallback(&backends, blob_ref)
        .await
        .map_err(|e| match e {
            lifeos_vcs::BackendError::NotFound { hash } => {
                ApiError::NotFound(format!("blob {hash} not found on any backend"))
            }
            other => ApiError::Internal(format!("blob read failed: {other}")),
        })
}
