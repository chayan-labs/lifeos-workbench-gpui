//! `/api/vcs/*` - the generic lifeos-vcs CLI surface (issue #86): commit,
//! history, checkout. Unlike `/api/files/commit` (issue #58, Drive-file
//! materialization, content passed as a plain string with no real byte
//! storage), these routes are the first callers to actually persist bytes
//! through lifeos-vcs's real CAS + commit model (#81/#82): `store_blob`
//! chunk-hashes and stores the content, `commit_version` updates the
//! entity's `blob_ref` and appends the `version.created` event in one call.
//!
//! No update/delete/rewrite route exists here, matching docs/AGENT-CONTROL.md
//! §1: VCS internals only ever grow forward through these routes.

use crate::auth::resolve_workspace;
use crate::db::index_entity;
use crate::error::{ApiError, ApiResult};
use crate::ids::{new_id, now_secs};
use crate::models::{read_entity, Entity, COLS_ENTITY};
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine as _;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct CommitFile {
    /// Existing entity id to commit a new version onto; omit to create a new file.
    entity_id: Option<String>,
    name: String,
    mime: Option<String>,
    /// Base64-encoded file bytes - the CLI reads the local file and encodes
    /// it client-side rather than the API trusting a client-supplied
    /// filesystem path (no path-traversal-shaped trust boundary).
    content_base64: String,
    #[serde(default)]
    message: Option<String>,
    workspace_id: Option<String>,
}

pub async fn commit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CommitFile>,
) -> ApiResult<Json<Entity>> {
    if req.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let content = base64::engine::general_purpose::STANDARD
        .decode(&req.content_base64)
        .map_err(|e| ApiError::BadRequest(format!("content_base64 is not valid base64: {e}")))?;

    let workspace_id = resolve_workspace(&headers, &state.config.jwt_secret, req.workspace_id.as_deref());
    let blob_ref =
        lifeos_vcs::store_blob(&state.vcs_store, &content).map_err(|e| ApiError::Internal(format!("blob store failed: {e}")))?;
    let size = content.len() as u64;

    let (id, parent_blob_ref, is_new) = match &req.entity_id {
        Some(existing_id) => {
            let parent = existing_blob_ref(&state, &workspace_id, existing_id).await?;
            (existing_id.clone(), parent, false)
        }
        None => (new_id("file"), None, true),
    };

    if parent_blob_ref.as_deref() == Some(blob_ref.as_str()) {
        return Err(ApiError::BadRequest("content unchanged since the last version".into()));
    }

    let now = now_secs();
    if is_new {
        let attrs = json!({ "name": req.name, "mime": req.mime, "size": size });
        let attrs_str = serde_json::to_string(&attrs).unwrap_or_else(|_| "{}".into());
        state
            .conn
            .execute(
                "INSERT INTO entities (id, workspace_id, module, type, title, attrs, source, blob_ref, created_at, updated_at) \
                 VALUES (?1, ?2, 'files', 'file', ?3, ?4, 'cli', ?5, ?6, ?6)",
                libsql::params![id.clone(), workspace_id.clone(), req.name.clone(), attrs_str, blob_ref.clone(), now],
            )
            .await?;
    }

    lifeos_vcs::commit_version(
        &state.conn,
        &workspace_id,
        &id,
        &blob_ref,
        parent_blob_ref.as_deref(),
        "cli",
        req.message.as_deref().unwrap_or(""),
        now,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("commit_version failed: {e}")))?;

    if let Err(e) = index_entity(&state.conn, &id).await {
        tracing::warn!("derived index upsert failed for {id}: {e}");
    }

    fetch_one(&state, &workspace_id, &id).await
}

#[derive(Deserialize)]
pub struct CheckoutQuery {
    entity_id: String,
    /// Specific historical version to retrieve; omits to the entity's
    /// current (latest) `blob_ref`.
    blob_ref: Option<String>,
}

/// Retrieval by hash "checks out" a version - no separate mutating verb is
/// needed to look at old content (docs/VERSIONING.md §2.3 note under #82).
pub async fn checkout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<CheckoutQuery>,
) -> ApiResult<Response> {
    let workspace_id = resolve_workspace(&headers, &state.config.jwt_secret, None);
    let blob_ref = match q.blob_ref {
        Some(r) => r,
        None => existing_blob_ref(&state, &workspace_id, &q.entity_id)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("entity '{}' has no committed version", q.entity_id)))?,
    };

    let bytes = lifeos_vcs::read_blob(&state.vcs_store, &blob_ref)
        .map_err(|e| ApiError::NotFound(format!("blob '{blob_ref}' not found: {e}")))?;

    Ok(([("content-type", "application/octet-stream")], bytes).into_response())
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    entity_id: String,
}

/// Version history reconstructed from `events` - no separate history table
/// (docs/VERSIONING.md §2.3).
pub async fn history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<Vec<lifeos_vcs::VersionEntry>>> {
    let workspace_id = resolve_workspace(&headers, &state.config.jwt_secret, None);
    let entries = lifeos_vcs::history(&state.conn, &workspace_id, &q.entity_id)
        .await
        .map_err(|e| ApiError::Internal(format!("history query failed: {e}")))?;
    Ok(Json(entries))
}

async fn existing_blob_ref(state: &AppState, workspace_id: &str, id: &str) -> ApiResult<Option<String>> {
    let mut rows = state
        .conn
        .query(
            "SELECT blob_ref FROM entities WHERE id = ?1 AND workspace_id = ?2",
            libsql::params![id, workspace_id],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(row.get::<Option<String>>(0)?),
        None => Err(ApiError::NotFound(format!("entity '{id}' not found"))),
    }
}

async fn fetch_one(state: &AppState, workspace_id: &str, id: &str) -> ApiResult<Json<Entity>> {
    let mut rows = state
        .conn
        .query(
            &format!("SELECT {COLS_ENTITY} FROM entities WHERE id = ?1 AND workspace_id = ?2"),
            libsql::params![id, workspace_id],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Json(read_entity(&row)?)),
        None => Err(ApiError::NotFound(format!("entity '{id}' not found"))),
    }
}
