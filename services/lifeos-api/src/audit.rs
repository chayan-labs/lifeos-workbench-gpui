//! Append-only domain log helper. Every meaningful state change writes one
//! `events` row. This is the only sanctioned way handlers add to the log.

use crate::error::ApiResult;
use crate::ids::{new_id, now_secs};
use libsql::Connection;

/// Append a domain event. `attrs` is any JSON payload (defaults to `{}`).
pub async fn emit(
    conn: &Connection,
    workspace_id: &str,
    event_type: &str,
    entity_id: Option<&str>,
    actor: &str,
    attrs: &serde_json::Value,
) -> ApiResult<String> {
    let id = new_id("evt");
    let attrs_str = serde_json::to_string(attrs).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO events (id, workspace_id, ts, type, entity_id, actor, attrs) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        libsql::params![
            id.clone(),
            workspace_id,
            now_secs(),
            event_type,
            entity_id,
            actor,
            attrs_str
        ],
    )
    .await?;
    Ok(id)
}
