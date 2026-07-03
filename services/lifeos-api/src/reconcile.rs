//! Events-based reconciliation (docs/DATA-MODEL.md §4.2-4.3).
//!
//! Sync between the Worker and the Mac embedded replica is last-push-wins at
//! row granularity over the whole `entities.attrs` blob - NOT last-writer-wins
//! on `updated_at`. If two writers (the bot lane and the Mac lane) push
//! divergent `attrs` for the same entity, whichever sync round trips last
//! wins, irrespective of which write actually happened later in wall-clock
//! time. `events` is append-only and conflict-free, so it is the source of
//! truth to repair from: replay every `entity.created`/`entity.updated` event
//! for the entity in causal order (ULID-prefixed `id`, which is
//! time-sortable, with `ts` as a secondary key) and the last attrs snapshot
//! in that order is the intended final state.
//!
//! Single-writer-per-row discipline (defense in depth, not enforced here):
//! the bot lane (light/medium tier) and the Mac lane (heavy tier) should not
//! both mutate `attrs` on the same entity in the same window - see
//! docs/DATA-MODEL.md §4.2. Reconciliation is the backstop for when that
//! invariant is violated or raced.

use crate::error::ApiResult;
use crate::ids::now_secs;
use libsql::Connection;
use serde_json::Value;

/// Replay `entity.created`/`entity.updated` events for one entity, in causal
/// order, and return the attrs blob implied by the last event that actually
/// carried an attrs snapshot (events from updates that didn't touch attrs
/// record `null` and are skipped). Returns `None` if no event recorded attrs.
pub async fn replay_entity_attrs(
    conn: &Connection,
    workspace_id: &str,
    entity_id: &str,
) -> ApiResult<Option<Value>> {
    let mut rows = conn
        .query(
            // Order by the ULID `id` (the causal key): it embeds millisecond
            // creation time and is monotonic per writer, so it is finer and more
            // skew-resistant than `ts`, which is only second-granularity. Using
            // `ts` first would let coarse, cross-machine-skewed timestamps reorder
            // same-second events that the ULID already orders correctly.
            "SELECT attrs FROM events \
             WHERE workspace_id = ?1 AND entity_id = ?2 \
               AND type IN ('entity.created', 'entity.updated') \
             ORDER BY id ASC",
            libsql::params![workspace_id, entity_id],
        )
        .await?;

    let mut last_attrs: Option<Value> = None;
    while let Some(row) = rows.next().await? {
        let payload: Option<String> = row.get(0)?;
        let Some(payload) = payload else { continue };
        let Ok(parsed) = serde_json::from_str::<Value>(&payload) else { continue };
        if let Some(attrs) = parsed.get("attrs") {
            if !attrs.is_null() {
                last_attrs = Some(attrs.clone());
            }
        }
    }
    Ok(last_attrs)
}

/// Replay and write the reconciled attrs back onto the entity row. No-op
/// (returns `None`) if the event log has no attrs snapshot to replay.
pub async fn reconcile_entity(
    conn: &Connection,
    workspace_id: &str,
    entity_id: &str,
) -> ApiResult<Option<Value>> {
    let Some(attrs) = replay_entity_attrs(conn, workspace_id, entity_id).await? else {
        return Ok(None);
    };
    let attrs_str = serde_json::to_string(&attrs).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "UPDATE entities SET attrs = ?1, updated_at = ?2 WHERE id = ?3 AND workspace_id = ?4",
        libsql::params![attrs_str, now_secs(), entity_id, workspace_id],
    )
    .await?;
    Ok(Some(attrs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::emit;
    use serde_json::json;

    async fn test_conn() -> Connection {
        let path = std::env::temp_dir().join(format!("lifeos_reconcile_{}.db", ulid::Ulid::new()));
        let db = libsql::Builder::new_local(path.to_str().unwrap())
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        conn.execute_batch(
            "CREATE TABLE entities (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, \
             attrs TEXT NOT NULL DEFAULT '{}', updated_at INTEGER NOT NULL); \
             CREATE TABLE events (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, ts INTEGER NOT NULL, \
             type TEXT NOT NULL, entity_id TEXT, actor TEXT, attrs TEXT DEFAULT '{}');",
        )
        .await
        .unwrap();
        conn
    }

    #[tokio::test]
    async fn replays_last_attrs_snapshot_in_causal_order() {
        let conn = test_conn().await;
        let ws = "ws_test";
        let ent = "ent_test";
        conn.execute(
            "INSERT INTO entities (id, workspace_id, attrs, updated_at) VALUES (?1, ?2, '{}', 0)",
            libsql::params![ent, ws],
        )
        .await
        .unwrap();

        // Simulate two divergent writers racing on the same row: the bot
        // lane writes first (causally), the Mac lane writes second - both
        // append a conflict-free event. But last-push-wins cares about sync
        // arrival order, not causal order: the bot's (older, causally-first)
        // push happens to reach the primary LAST, so the synced row ends up
        // holding the bot's stale attrs even though the Mac's write is the
        // true intended final state.
        emit(&conn, ws, "entity.created", Some(ent), "api", &json!({ "attrs": {} })).await.unwrap();
        emit(&conn, ws, "entity.updated", Some(ent), "bot", &json!({ "attrs": { "status": "todo" } }))
            .await
            .unwrap();
        emit(&conn, ws, "entity.updated", Some(ent), "mac", &json!({ "attrs": { "status": "done" } }))
            .await
            .unwrap();
        // Forced conflict: the row reflects the bot's write, which lost the sync race
        // despite being causally older - the opposite of what the event log implies.
        conn.execute(
            "UPDATE entities SET attrs = '{\"status\":\"todo\"}' WHERE id = ?1",
            libsql::params![ent],
        )
        .await
        .unwrap();

        let reconciled = reconcile_entity(&conn, ws, ent).await.unwrap();
        assert_eq!(reconciled, Some(json!({ "status": "done" })));

        let mut rows = conn
            .query("SELECT attrs FROM entities WHERE id = ?1", libsql::params![ent])
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let attrs: String = row.get(0).unwrap();
        assert_eq!(attrs, "{\"status\":\"done\"}");
    }

    #[tokio::test]
    async fn returns_none_when_no_attrs_events_exist() {
        let conn = test_conn().await;
        let result = replay_entity_attrs(&conn, "ws_test", "ent_missing").await.unwrap();
        assert_eq!(result, None);
    }
}
