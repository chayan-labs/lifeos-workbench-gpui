//! `/api/storage/backends` - per-workspace storage-backend configuration
//! (issue #107, docs/STORAGE-BACKENDS.md §4).
//!
//! Adding/switching a backend MOVES the user's data, so `create` is gated:
//! it only ever writes a `pending_approval` config entity (the same
//! draft -> approve -> active flow as every outward action,
//! docs/SECURITY.md §2). Keys never enter `attrs` - they are
//! envelope-encrypted into `connections.secret_enc` and the entity carries
//! only the `connection_id` handle. Storage selection is part of the
//! connections protected domain: the in-app agent can read/render content
//! but cannot call this route to reconfigure storage
//! (docs/AGENT-CONTROL.md §1, docs/STORAGE-BACKENDS.md §6).

use crate::audit::emit;
use crate::auth::resolve_workspace;
use crate::db::{index_entity, workspace_exists};
use crate::error::{ApiError, ApiResult};
use crate::ids::{new_id, now_secs};
use crate::models::{read_entity, Entity, COLS_ENTITY};
use crate::state::AppState;
use crate::storage::STORAGE_KINDS;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct ListParams {
    workspace_id: Option<String>,
}

/// `GET /api/storage/backends` - free read: every backend config in the
/// workspace, any status, so the UI can show pending drafts too.
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
) -> ApiResult<Json<Vec<Entity>>> {
    let workspace_id = resolve_workspace(&headers, &state.config.jwt_secret, params.workspace_id.as_deref());
    let mut rows = state
        .conn
        .query(
            &format!(
                "SELECT {COLS_ENTITY} FROM entities \
                 WHERE workspace_id = ?1 AND module = 'storage' AND type = 'storage_backend' \
                 ORDER BY created_at DESC"
            ),
            libsql::params![workspace_id],
        )
        .await?;
    let mut entities = Vec::new();
    while let Some(row) = rows.next().await? {
        entities.push(read_entity(&row)?);
    }
    Ok(Json(entities))
}

#[derive(Deserialize)]
pub struct CreateBackend {
    kind: String,
    /// Life OS app folder / prefix on the provider.
    folder: Option<String>,
    /// Existing connection to authenticate with (e.g. a Nango OAuth
    /// connection completed via /api/connections). Mutually exclusive with
    /// `keys`.
    connection_id: Option<String>,
    /// Non-Nango credentials (S3/R2/GCS/Azure/WebDAV). Envelope-encrypted
    /// into `connections.secret_enc` - never stored in entity attrs.
    keys: Option<Value>,
    /// Client-side envelope encryption at rest (issue #110).
    #[serde(default)]
    encryption: bool,
    /// Make this the primary (write) backend once approved.
    #[serde(default)]
    default: bool,
    workspace_id: Option<String>,
}

/// `POST /api/storage/backends` - GATED (docs/STORAGE-BACKENDS.md §4): only
/// drafts a `pending_approval` config entity; nothing changes where bytes
/// live until a human approves it.
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateBackend>,
) -> ApiResult<Json<Entity>> {
    if !STORAGE_KINDS.contains(&req.kind.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "unknown backend kind '{}' (expected one of {})",
            req.kind,
            STORAGE_KINDS.join(", ")
        )));
    }
    let workspace_id = resolve_workspace(&headers, &state.config.jwt_secret, req.workspace_id.as_deref());
    if !workspace_exists(&state.conn, &workspace_id).await? {
        return Err(ApiError::BadRequest(format!("unknown workspace '{workspace_id}'")));
    }
    if req.connection_id.is_some() && req.keys.is_some() {
        return Err(ApiError::BadRequest("pass either connection_id or keys, not both".into()));
    }

    let connection_id = match (&req.connection_id, &req.keys) {
        (Some(existing), None) => Some(existing.clone()),
        (None, Some(keys)) => Some(store_keys(&state, &workspace_id, &req.kind, keys).await?),
        (None, None) if req.kind == "local-fs" => None,
        (None, None) => {
            return Err(ApiError::BadRequest(format!("'{}' backend needs connection_id or keys", req.kind)))
        }
        (Some(_), Some(_)) => unreachable!("guarded above"),
    };

    // Only the handle and non-secret settings - key material never lands in
    // attrs, events, or logs.
    let attrs = json!({
        "kind": req.kind,
        "folder": req.folder,
        "connection_id": connection_id,
        "encryption": req.encryption,
        "default": req.default,
    });

    let id = new_id("ent");
    let now = now_secs();
    let attrs_str = serde_json::to_string(&attrs).unwrap_or_else(|_| "{}".into());
    let title = format!("{} backend", req.kind);
    state
        .conn
        .execute(
            "INSERT INTO entities \
             (id, workspace_id, module, type, parent_id, title, status, tier, attrs, source, blob_ref, created_at, updated_at) \
             VALUES (?1, ?2, 'storage', 'storage_backend', NULL, ?3, 'pending_approval', NULL, ?4, 'api', NULL, ?5, ?5)",
            libsql::params![id.clone(), workspace_id.clone(), title, attrs_str, now],
        )
        .await?;
    emit(&state.conn, &workspace_id, "storage.backend.drafted", Some(&id), "api", &attrs).await?;
    if let Err(e) = index_entity(&state.conn, &id).await {
        tracing::warn!("derived index upsert failed for {id}: {e}");
    }

    let mut rows = state
        .conn
        .query(&format!("SELECT {COLS_ENTITY} FROM entities WHERE id = ?1"), libsql::params![id])
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Json(read_entity(&row)?)),
        None => Err(ApiError::Internal("backend draft vanished after insert".into())),
    }
}

/// Envelope-encrypts non-Nango backend keys into a `connections` row and
/// returns its id - the only artifact the config entity references.
async fn store_keys(state: &AppState, workspace_id: &str, kind: &str, keys: &Value) -> ApiResult<String> {
    let enc_key = state
        .config
        .secret_encryption_key
        .as_ref()
        .ok_or_else(|| ApiError::NotImplemented("LIFEOS_SECRET_ENCRYPTION_KEY is not configured".into()))?;
    let plaintext = serde_json::to_string(keys).map_err(|_| ApiError::BadRequest("keys must be a JSON object".into()))?;
    let secret_enc = crate::crypto::encrypt(&plaintext, enc_key)?;

    let connection_id = new_id("conn");
    state
        .conn
        .execute(
            "INSERT INTO connections (id, workspace_id, provider, account_handle, nango_connection_id, secret_enc, scopes, expires_at, status, created_at) \
             VALUES (?1, ?2, ?3, NULL, NULL, ?4, NULL, NULL, 'active', ?5)",
            libsql::params![connection_id.clone(), workspace_id, format!("storage-{kind}"), secret_enc, now_secs()],
        )
        .await?;
    Ok(connection_id)
}
