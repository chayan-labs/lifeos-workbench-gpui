//! Shared helpers for the per-provider Nango proxy tools (issue #53,
//! docs/INTEGRATIONS.md). Each provider route file (`routes/gmail.rs`,
//! `routes/calendar.rs`, ...) is a thin wrapper around `proxy_call`/
//! `draft_action` here: reads go straight through Nango's proxy (the token
//! is injected server-side and never reaches this process's caller); writes
//! never call the provider at all - they only create a `pending_approval`
//! draft entity for the draft -> Telegram-approve -> execute queue
//! (docs/SECURITY.md §2). `draft_action` has no path to `state.nango`, so
//! that guarantee holds structurally, not just by convention.

use crate::audit::emit;
use crate::db::index_entity;
use crate::error::{ApiError, ApiResult};
use crate::ids::{new_id, now_secs};
use crate::models::{read_entity, Entity, COLS_ENTITY};
use crate::state::AppState;
use serde_json::Value;

pub fn nango_or_501(state: &AppState) -> ApiResult<&dyn crate::nango::NangoClient> {
    state
        .nango
        .as_deref()
        .ok_or_else(|| ApiError::NotImplemented("Nango is not configured - see docs/MANUAL-SETUP.md".into()))
}

/// Resolve the active Nango `connectionId` for `provider` in this workspace.
/// Never returns a token - only the handle Nango's proxy uses server-side.
async fn connection_id_for(state: &AppState, workspace_id: &str, provider: &str) -> ApiResult<String> {
    let mut rows = state
        .conn
        .query(
            "SELECT nango_connection_id FROM connections \
             WHERE workspace_id = ?1 AND provider = ?2 AND status = 'active' \
             ORDER BY created_at DESC LIMIT 1",
            libsql::params![workspace_id, provider],
        )
        .await?;
    let connection_id = match rows.next().await? {
        Some(row) => row.get::<Option<String>>(0)?,
        None => None,
    };
    connection_id.ok_or_else(|| ApiError::NotFound(format!("no active '{provider}' connection")))
}

/// A free read (or any call we've chosen not to gate): proxies straight
/// through to the provider via Nango, token injected server-side.
pub async fn proxy_call(
    state: &AppState,
    workspace_id: &str,
    provider: &str,
    method: &str,
    endpoint: &str,
    query: &[(&str, &str)],
    body: Option<Value>,
) -> ApiResult<Value> {
    let nango = nango_or_501(state)?;
    let connection_id = connection_id_for(state, workspace_id, provider).await?;
    nango.proxy(&connection_id, provider, method, endpoint, query, body).await
}

/// A gated write: never touches the provider. Creates a `pending_approval`
/// draft entity for the approve -> execute queue (docs/SECURITY.md §2).
pub async fn draft_action(
    state: &AppState,
    workspace_id: &str,
    provider: &str,
    action: &str,
    attrs: Value,
) -> ApiResult<Entity> {
    let id = new_id("ent");
    let now = now_secs();
    let entity_type = format!("{provider}_{action}");
    let attrs_str = serde_json::to_string(&attrs).unwrap_or_else(|_| "{}".into());
    state
        .conn
        .execute(
            "INSERT INTO entities \
             (id, workspace_id, module, type, parent_id, title, status, tier, attrs, source, blob_ref, created_at, updated_at) \
             VALUES (?1, ?2, 'integrations', ?3, NULL, NULL, 'pending_approval', NULL, ?4, 'api', NULL, ?5, ?6)",
            libsql::params![id.clone(), workspace_id, entity_type.clone(), attrs_str, now, now],
        )
        .await?;
    emit(&state.conn, workspace_id, &format!("{provider}.{action}.drafted"), Some(&id), "api", &attrs).await?;
    if let Err(e) = index_entity(&state.conn, &id).await {
        tracing::warn!("derived index upsert failed for {id}: {e}");
    }

    let mut rows = state
        .conn
        .query(&format!("SELECT {COLS_ENTITY} FROM entities WHERE id = ?1"), libsql::params![id.clone()])
        .await?;
    match rows.next().await? {
        Some(row) => Ok(read_entity(&row)?),
        None => Err(ApiError::Internal("draft entity vanished after insert".into())),
    }
}
