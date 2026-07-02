//! Provider-file storage backends (issue #107, docs/STORAGE-BACKENDS.md §3):
//! Google Drive / Dropbox / OneDrive (via the Nango proxy) and WebDAV (via
//! user keys) as [`StorageBackend`]s. Blob bytes are stored as provider
//! files under a Life OS app folder; the backend index (issue #106) maps
//! hash <-> provider-native file id for id-shaped providers.
//!
//! This crate never sees a token: all HTTP goes through the [`BackendProxy`]
//! trait, implemented by `lifeos-api` (Nango proxy with server-side token
//! injection, or a WebDAV client holding decrypted `secret_enc` keys). The
//! proxy carries opaque request/response bytes only - credentials are
//! injected on the other side of the trait boundary (docs/SECURITY.md §1).

use std::sync::Arc;

use async_trait::async_trait;
use libsql::Connection;

use crate::backend::{BackendError, StorageBackend};
use crate::backend_index::{forget_location, location_for, record_location};

/// One opaque HTTP exchange. The implementor injects auth (Nango
/// `connectionId` headers or WebDAV basic auth) - never this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyRequest {
    pub method: String,
    pub endpoint: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct ProxyResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[async_trait]
pub trait BackendProxy: Send + Sync {
    async fn execute(&self, req: ProxyRequest) -> Result<ProxyResponse, BackendError>;
}

/// Which provider dialect the backend speaks. Drive is id-shaped (uploads
/// return a file id the index must remember); the rest are path-shaped
/// (locator derivable from the hash alone).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyProvider {
    GoogleDrive,
    Dropbox,
    OneDrive,
    WebDav,
}

impl ProxyProvider {
    pub fn id(&self) -> &'static str {
        match self {
            ProxyProvider::GoogleDrive => "google-drive",
            ProxyProvider::Dropbox => "dropbox",
            ProxyProvider::OneDrive => "onedrive",
            ProxyProvider::WebDav => "webdav",
        }
    }
}

/// A storage backend whose bytes live as files on a consumer/self-hosted
/// provider, reached through a credential-injecting [`BackendProxy`].
pub struct ProxiedFileBackend {
    provider: ProxyProvider,
    proxy: Arc<dyn BackendProxy>,
    /// Canonical-DB connection for the backend index (metadata only).
    conn: Connection,
    workspace_id: String,
    /// The `storage_backend` config entity id this backend was built from.
    backend_id: String,
    /// Life OS app folder on the provider (Drive folder id / path prefix).
    folder: String,
}

impl ProxiedFileBackend {
    pub fn new(
        provider: ProxyProvider,
        proxy: Arc<dyn BackendProxy>,
        conn: Connection,
        workspace_id: impl Into<String>,
        backend_id: impl Into<String>,
        folder: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            proxy,
            conn,
            workspace_id: workspace_id.into(),
            backend_id: backend_id.into(),
            folder: folder.into(),
        }
    }

    /// Path-shaped locator (Dropbox/OneDrive/WebDAV). Drive's true locator
    /// is the file id returned at upload time and lives in the index.
    fn object_path(&self, hash: &str) -> String {
        format!("{}/objects/{}/{}", self.folder.trim_end_matches('/'), &hash[..2], hash)
    }

    async fn indexed_locator(&self, hash: &str) -> Result<Option<String>, BackendError> {
        location_for(&self.conn, &self.workspace_id, &self.backend_id, hash)
            .await
            .map_err(|e| BackendError::Other(format!("backend index read failed: {e}")))
    }

    async fn remember(&self, hash: &str, locator: &str, now: i64) -> Result<(), BackendError> {
        record_location(&self.conn, &self.workspace_id, &self.backend_id, hash, locator, now)
            .await
            .map_err(|e| BackendError::Other(format!("backend index write failed: {e}")))
    }

    async fn run(&self, req: ProxyRequest, op: &str) -> Result<ProxyResponse, BackendError> {
        let resp = self.proxy.execute(req).await?;
        if resp.status == 404 {
            return Err(BackendError::Other(format!("{op}: provider returned 404")));
        }
        if resp.status >= 300 {
            return Err(BackendError::Other(format!("{op}: provider returned {}", resp.status)));
        }
        Ok(resp)
    }

    /// Uploads `bytes` and returns the provider-native locator.
    async fn upload(&self, hash: &str, bytes: &[u8]) -> Result<String, BackendError> {
        match self.provider {
            ProxyProvider::GoogleDrive => self.upload_drive(hash, bytes).await,
            ProxyProvider::Dropbox => {
                let path = self.object_path(hash);
                let arg = format!(r#"{{"path":"{path}","mode":"overwrite"}}"#);
                self.run(
                    ProxyRequest {
                        method: "POST".into(),
                        endpoint: "2/files/upload".into(),
                        query: vec![],
                        headers: vec![
                            ("Dropbox-API-Arg".into(), arg),
                            ("Content-Type".into(), "application/octet-stream".into()),
                        ],
                        body: Some(bytes.to_vec()),
                    },
                    "dropbox upload",
                )
                .await?;
                Ok(path)
            }
            ProxyProvider::OneDrive => {
                let path = self.object_path(hash);
                self.run(
                    ProxyRequest {
                        method: "PUT".into(),
                        endpoint: format!("me/drive/special/approot:/{path}:/content"),
                        query: vec![],
                        headers: vec![("Content-Type".into(), "application/octet-stream".into())],
                        body: Some(bytes.to_vec()),
                    },
                    "onedrive upload",
                )
                .await?;
                Ok(path)
            }
            ProxyProvider::WebDav => {
                let path = self.object_path(hash);
                self.run(
                    ProxyRequest {
                        method: "PUT".into(),
                        endpoint: path.clone(),
                        query: vec![],
                        headers: vec![("Content-Type".into(), "application/octet-stream".into())],
                        body: Some(bytes.to_vec()),
                    },
                    "webdav upload",
                )
                .await?;
                Ok(path)
            }
        }
    }

    /// Drive is id-shaped: create file metadata (named by hash, under the
    /// app folder), then upload the media into the returned file id.
    async fn upload_drive(&self, hash: &str, bytes: &[u8]) -> Result<String, BackendError> {
        let metadata = serde_json::json!({ "name": hash, "parents": [self.folder] });
        let created = self
            .run(
                ProxyRequest {
                    method: "POST".into(),
                    endpoint: "drive/v3/files".into(),
                    query: vec![],
                    headers: vec![("Content-Type".into(), "application/json".into())],
                    body: Some(metadata.to_string().into_bytes()),
                },
                "drive create",
            )
            .await?;
        let parsed: serde_json::Value = serde_json::from_slice(&created.body)
            .map_err(|e| BackendError::Other(format!("drive create: unparseable response: {e}")))?;
        let file_id = parsed["id"]
            .as_str()
            .ok_or_else(|| BackendError::Other("drive create: response has no file id".into()))?
            .to_string();

        self.run(
            ProxyRequest {
                method: "PATCH".into(),
                endpoint: format!("upload/drive/v3/files/{file_id}"),
                query: vec![("uploadType".into(), "media".into())],
                headers: vec![("Content-Type".into(), "application/octet-stream".into())],
                body: Some(bytes.to_vec()),
            },
            "drive upload",
        )
        .await?;
        Ok(file_id)
    }

    fn download_request(&self, locator: &str) -> ProxyRequest {
        match self.provider {
            ProxyProvider::GoogleDrive => ProxyRequest {
                method: "GET".into(),
                endpoint: format!("drive/v3/files/{locator}"),
                query: vec![("alt".into(), "media".into())],
                headers: vec![],
                body: None,
            },
            ProxyProvider::Dropbox => ProxyRequest {
                method: "POST".into(),
                endpoint: "2/files/download".into(),
                query: vec![],
                headers: vec![("Dropbox-API-Arg".into(), format!(r#"{{"path":"{locator}"}}"#))],
                body: None,
            },
            ProxyProvider::OneDrive => ProxyRequest {
                method: "GET".into(),
                endpoint: format!("me/drive/special/approot:/{locator}:/content"),
                query: vec![],
                headers: vec![],
                body: None,
            },
            ProxyProvider::WebDav => ProxyRequest {
                method: "GET".into(),
                endpoint: locator.to_string(),
                query: vec![],
                headers: vec![],
                body: None,
            },
        }
    }

    fn delete_request(&self, locator: &str) -> ProxyRequest {
        match self.provider {
            ProxyProvider::GoogleDrive => ProxyRequest {
                method: "DELETE".into(),
                endpoint: format!("drive/v3/files/{locator}"),
                query: vec![],
                headers: vec![],
                body: None,
            },
            ProxyProvider::Dropbox => ProxyRequest {
                method: "POST".into(),
                endpoint: "2/files/delete_v2".into(),
                query: vec![],
                headers: vec![("Content-Type".into(), "application/json".into())],
                body: Some(format!(r#"{{"path":"{locator}"}}"#).into_bytes()),
            },
            ProxyProvider::OneDrive => ProxyRequest {
                method: "DELETE".into(),
                endpoint: format!("me/drive/special/approot:/{locator}"),
                query: vec![],
                headers: vec![],
                body: None,
            },
            ProxyProvider::WebDav => ProxyRequest {
                method: "DELETE".into(),
                endpoint: locator.to_string(),
                query: vec![],
                headers: vec![],
                body: None,
            },
        }
    }
}

#[async_trait]
impl StorageBackend for ProxiedFileBackend {
    async fn put(&self, hash: &str, bytes: &[u8]) -> Result<(), BackendError> {
        if self.indexed_locator(hash).await?.is_some() {
            return Ok(()); // CAS idempotency: already on this backend
        }
        let locator = self.upload(hash, bytes).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.remember(hash, &locator, now).await
    }

    async fn fetch_unverified(&self, hash: &str) -> Result<Vec<u8>, BackendError> {
        let locator = match self.indexed_locator(hash).await? {
            Some(l) => l,
            // Path-shaped providers can derive the locator; Drive cannot.
            None if self.provider != ProxyProvider::GoogleDrive => self.object_path(hash),
            None => return Err(BackendError::NotFound { hash: hash.to_string() }),
        };
        let resp = self.proxy.execute(self.download_request(&locator)).await?;
        if resp.status == 404 {
            return Err(BackendError::NotFound { hash: hash.to_string() });
        }
        if resp.status >= 300 {
            return Err(BackendError::Other(format!("download: provider returned {}", resp.status)));
        }
        Ok(resp.body)
    }

    async fn has(&self, hash: &str) -> Result<bool, BackendError> {
        Ok(self.indexed_locator(hash).await?.is_some())
    }

    async fn delete(&self, hash: &str) -> Result<(), BackendError> {
        let Some(locator) = self.indexed_locator(hash).await? else {
            return Ok(()); // GC semantics: deleting a missing object is a no-op
        };
        let resp = self.proxy.execute(self.delete_request(&locator)).await?;
        if resp.status >= 300 && resp.status != 404 {
            return Err(BackendError::Other(format!("delete: provider returned {}", resp.status)));
        }
        forget_location(&self.conn, &self.workspace_id, &self.backend_id, hash)
            .await
            .map_err(|e| BackendError::Other(format!("backend index delete failed: {e}")))
    }

    /// Logical location; the provider-native id (when id-shaped) lives in
    /// the backend index, which is async and therefore not reachable here.
    fn location(&self, hash: &str) -> String {
        format!("{}:{}", self.provider.id(), self.object_path(hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_bytes;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory provider: understands just enough of each dialect to serve
    /// uploads back. Keyed by locator.
    struct FakeProvider {
        provider: ProxyProvider,
        files: Mutex<HashMap<String, Vec<u8>>>,
        calls: Mutex<Vec<String>>,
        next_id: Mutex<u32>,
        /// When set, downloads return these bytes instead (tamper testing).
        tamper: Mutex<Option<Vec<u8>>>,
    }

    impl FakeProvider {
        fn new(provider: ProxyProvider) -> Self {
            Self {
                provider,
                files: Mutex::new(HashMap::new()),
                calls: Mutex::new(Vec::new()),
                next_id: Mutex::new(0),
                tamper: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl BackendProxy for FakeProvider {
        async fn execute(&self, req: ProxyRequest) -> Result<ProxyResponse, BackendError> {
            self.calls.lock().unwrap().push(format!("{} {}", req.method, req.endpoint));
            let ok = |body: Vec<u8>| Ok(ProxyResponse { status: 200, body });

            match self.provider {
                ProxyProvider::GoogleDrive => {
                    if req.endpoint == "drive/v3/files" && req.method == "POST" {
                        let mut next = self.next_id.lock().unwrap();
                        *next += 1;
                        let id = format!("drive-file-{next}");
                        return ok(format!(r#"{{"id":"{id}"}}"#).into_bytes());
                    }
                    if let Some(id) = req.endpoint.strip_prefix("upload/drive/v3/files/") {
                        self.files.lock().unwrap().insert(id.to_string(), req.body.unwrap_or_default());
                        return ok(b"{}".to_vec());
                    }
                    if let Some(id) = req.endpoint.strip_prefix("drive/v3/files/") {
                        if req.method == "DELETE" {
                            self.files.lock().unwrap().remove(id);
                            return ok(vec![]);
                        }
                        if let Some(t) = self.tamper.lock().unwrap().clone() {
                            return ok(t);
                        }
                        return match self.files.lock().unwrap().get(id) {
                            Some(bytes) => ok(bytes.clone()),
                            None => Ok(ProxyResponse { status: 404, body: vec![] }),
                        };
                    }
                    Ok(ProxyResponse { status: 400, body: vec![] })
                }
                // The path-shaped dialects all store/serve by a path locator.
                _ => {
                    let path = match self.provider {
                        ProxyProvider::Dropbox => req
                            .headers
                            .iter()
                            .find(|(k, _)| k == "Dropbox-API-Arg")
                            .and_then(|(_, v)| serde_json::from_str::<serde_json::Value>(v).ok())
                            .and_then(|v| v["path"].as_str().map(str::to_string))
                            .or_else(|| {
                                req.body
                                    .as_deref()
                                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok())
                                    .and_then(|v| v["path"].as_str().map(str::to_string))
                            })
                            .unwrap_or_default(),
                        ProxyProvider::OneDrive => req
                            .endpoint
                            .trim_start_matches("me/drive/special/approot:/")
                            .trim_end_matches(":/content")
                            .to_string(),
                        _ => req.endpoint.clone(),
                    };
                    let is_upload = matches!(
                        (self.provider, req.method.as_str()),
                        (ProxyProvider::Dropbox, "POST") if req.endpoint == "2/files/upload"
                    ) || (self.provider != ProxyProvider::Dropbox && req.method == "PUT");
                    let is_delete = req.method == "DELETE" || req.endpoint == "2/files/delete_v2";

                    if is_upload {
                        self.files.lock().unwrap().insert(path, req.body.unwrap_or_default());
                        return ok(b"{}".to_vec());
                    }
                    if is_delete {
                        self.files.lock().unwrap().remove(&path);
                        return ok(b"{}".to_vec());
                    }
                    if let Some(t) = self.tamper.lock().unwrap().clone() {
                        return ok(t);
                    }
                    match self.files.lock().unwrap().get(&path) {
                        Some(bytes) => ok(bytes.clone()),
                        None => Ok(ProxyResponse { status: 404, body: vec![] }),
                    }
                }
            }
        }
    }

    async fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db = libsql::Builder::new_local(dir.path().join("t.db").to_str().unwrap())
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        crate::backend_index::create_table_for_tests(&conn).await;
        (dir, conn)
    }

    fn backend_over(provider: ProxyProvider, fake: Arc<FakeProvider>, conn: Connection) -> ProxiedFileBackend {
        ProxiedFileBackend::new(provider, fake, conn, "ws_1", "backend_1", "lifeos")
    }

    #[tokio::test]
    async fn every_provider_dialect_round_trips_a_blob() {
        for provider in [
            ProxyProvider::GoogleDrive,
            ProxyProvider::Dropbox,
            ProxyProvider::OneDrive,
            ProxyProvider::WebDav,
        ] {
            let (_dir, conn) = test_conn().await;
            let fake = Arc::new(FakeProvider::new(provider));
            let backend = backend_over(provider, fake, conn);
            let data = format!("bytes for {provider:?}").into_bytes();
            let hash = hash_bytes(&data);

            backend.put(&hash, &data).await.unwrap();

            assert!(backend.has(&hash).await.unwrap(), "{provider:?}");
            assert_eq!(backend.get(&hash).await.unwrap(), data, "{provider:?}");
        }
    }

    #[tokio::test]
    async fn drive_uploads_record_the_file_id_in_the_backend_index() {
        let (_dir, conn) = test_conn().await;
        let fake = Arc::new(FakeProvider::new(ProxyProvider::GoogleDrive));
        let backend = backend_over(ProxyProvider::GoogleDrive, fake, conn.clone());
        let data = b"drive-bound".to_vec();
        let hash = hash_bytes(&data);

        backend.put(&hash, &data).await.unwrap();

        let locator = location_for(&conn, "ws_1", "backend_1", &hash).await.unwrap();
        assert_eq!(locator.as_deref(), Some("drive-file-1"));
    }

    #[tokio::test]
    async fn put_is_idempotent_and_skips_the_provider_on_repeat() {
        let (_dir, conn) = test_conn().await;
        let fake = Arc::new(FakeProvider::new(ProxyProvider::Dropbox));
        let backend = backend_over(ProxyProvider::Dropbox, fake.clone(), conn);
        let data = b"dedup me".to_vec();
        let hash = hash_bytes(&data);

        backend.put(&hash, &data).await.unwrap();
        let calls_after_first = fake.calls.lock().unwrap().len();
        backend.put(&hash, &data).await.unwrap();

        assert_eq!(fake.calls.lock().unwrap().len(), calls_after_first);
    }

    #[tokio::test]
    async fn a_tampering_provider_fails_the_blake3_check_on_get() {
        let (_dir, conn) = test_conn().await;
        let fake = Arc::new(FakeProvider::new(ProxyProvider::GoogleDrive));
        let backend = backend_over(ProxyProvider::GoogleDrive, fake.clone(), conn);
        let data = b"authentic content".to_vec();
        let hash = hash_bytes(&data);
        backend.put(&hash, &data).await.unwrap();
        *fake.tamper.lock().unwrap() = Some(b"swapped by the provider".to_vec());

        let result = backend.get(&hash).await;

        assert!(matches!(result, Err(BackendError::IntegrityMismatch { .. })));
    }

    #[tokio::test]
    async fn delete_removes_the_provider_file_and_the_index_row() {
        let (_dir, conn) = test_conn().await;
        let fake = Arc::new(FakeProvider::new(ProxyProvider::OneDrive));
        let backend = backend_over(ProxyProvider::OneDrive, fake, conn.clone());
        let data = b"short-lived".to_vec();
        let hash = hash_bytes(&data);
        backend.put(&hash, &data).await.unwrap();

        backend.delete(&hash).await.unwrap();

        assert!(!backend.has(&hash).await.unwrap());
        assert_eq!(location_for(&conn, "ws_1", "backend_1", &hash).await.unwrap(), None);
    }

    #[tokio::test]
    async fn chunked_blob_store_works_over_a_proxied_backend() {
        // The issue-#107 acceptance end-to-end: chunk-level store/fetch of a
        // real multi-chunk blob through a provider-file backend.
        let (_dir, conn) = test_conn().await;
        let fake = Arc::new(FakeProvider::new(ProxyProvider::WebDav));
        let backend = backend_over(ProxyProvider::WebDav, fake, conn);
        let mut data = vec![0u8; 400_000];
        let mut state = 5u32;
        for byte in data.iter_mut() {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            *byte = (state >> 16) as u8;
        }

        let blob_ref = crate::store_blob_on_backend(&backend, &data).await.unwrap();
        let restored = crate::read_blob_from_backend(&backend, &blob_ref).await.unwrap();

        assert_eq!(restored, data);
    }
}
