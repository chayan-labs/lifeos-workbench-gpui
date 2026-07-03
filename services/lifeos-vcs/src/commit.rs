//! Commit model (issue #82, docs/VERSIONING.md §2.3): a commit is a
//! `version.created` event, not a separate history table. Version history of
//! any entity is a plain query over `events`, so it survives sync without
//! merge hazards the same way the rest of the append-only log does.
//!
//! This crate has no dependency on `lifeos-api` (mirrors `lifeos-drain`'s
//! standalone-process convention against the same DB file), so `emit_event`
//! is a small self-contained mirror of `lifeos_api::audit::emit`.

use std::sync::Mutex;

use libsql::{params, Connection};
use serde::{Deserialize, Serialize};
use ulid::{Generator, Ulid};

static EVENT_ID_GENERATOR: Mutex<Generator> = Mutex::new(Generator::new());

fn new_event_id() -> String {
    let ulid = EVENT_ID_GENERATOR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .generate()
        .unwrap_or_else(|_| Ulid::new());
    format!("evt_{ulid}")
}

async fn emit_event(
    conn: &Connection,
    workspace_id: &str,
    event_type: &str,
    entity_id: &str,
    actor: &str,
    attrs: &serde_json::Value,
    now: i64,
) -> libsql::Result<()> {
    let attrs_str = serde_json::to_string(attrs).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO events (id, workspace_id, ts, type, entity_id, actor, attrs) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![new_event_id(), workspace_id, now, event_type, entity_id, actor, attrs_str],
    )
    .await?;
    Ok(())
}

/// A version-history entry, reconstructed from a `version.created` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionEntry {
    pub blob_ref: String,
    pub parent_blob_ref: Option<String>,
    pub author: String,
    pub message: String,
    pub ts: i64,
}

/// Records a new version of `entity_id`: emits `version.created` (the
/// commit) and updates the entity's `blob_ref` to point at it. `parent_ref`
/// should be the entity's current `blob_ref` before this call, if any -
/// callers read it via `history` (the latest entry) or their own entity
/// fetch; this function doesn't look it up itself, keeping it a single
/// write path with no read-then-write race.
#[allow(clippy::too_many_arguments)] // mirrors the version.created event columns
pub async fn commit_version(
    conn: &Connection,
    workspace_id: &str,
    entity_id: &str,
    blob_ref: &str,
    parent_ref: Option<&str>,
    author: &str,
    message: &str,
    now: i64,
) -> libsql::Result<()> {
    conn.execute(
        "UPDATE entities SET blob_ref=?2, updated_at=?3 WHERE id=?1 AND workspace_id=?4",
        params![entity_id, blob_ref, now, workspace_id],
    )
    .await?;

    let attrs = serde_json::json!({
        "entity_id": entity_id,
        "blob_ref": blob_ref,
        "parent_blob_ref": parent_ref,
        "author": author,
        "message": message,
    });
    emit_event(conn, workspace_id, "version.created", entity_id, author, &attrs, now).await
}

/// Reconstructs an entity's full version history from `events` - oldest
/// first - by querying `version.created` rows for that entity. No separate
/// history table (docs/VERSIONING.md §2.3).
pub async fn history(
    conn: &Connection,
    workspace_id: &str,
    entity_id: &str,
) -> libsql::Result<Vec<VersionEntry>> {
    let mut rows = conn
        .query(
            "SELECT attrs, ts FROM events \
             WHERE workspace_id=?1 AND entity_id=?2 AND type='version.created' \
             ORDER BY ts ASC",
            params![workspace_id, entity_id],
        )
        .await?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        let attrs_str: String = row.get(0)?;
        let ts: i64 = row.get(1)?;
        let attrs: serde_json::Value = serde_json::from_str(&attrs_str).unwrap_or_default();
        entries.push(VersionEntry {
            blob_ref: attrs["blob_ref"].as_str().unwrap_or_default().to_string(),
            parent_blob_ref: attrs["parent_blob_ref"].as_str().map(str::to_string),
            author: attrs["author"].as_str().unwrap_or_default().to_string(),
            message: attrs["message"].as_str().unwrap_or_default().to_string(),
            ts,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use libsql::Builder;

    async fn fresh_conn(path: &str) -> Connection {
        let _ = std::fs::remove_file(path);
        let db = Builder::new_local(path).build().await.unwrap();
        let conn = db.connect().unwrap();
        conn.execute(
            "CREATE TABLE entities (\
                id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, blob_ref TEXT, \
                updated_at INTEGER NOT NULL)",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "CREATE TABLE events (\
                id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, ts INTEGER NOT NULL, \
                type TEXT NOT NULL, entity_id TEXT, actor TEXT NOT NULL, attrs TEXT NOT NULL)",
            (),
        )
        .await
        .unwrap();
        conn
    }

    async fn insert_entity(conn: &Connection, id: &str, workspace_id: &str, now: i64) {
        conn.execute(
            "INSERT INTO entities (id, workspace_id, blob_ref, updated_at) VALUES (?1, ?2, NULL, ?3)",
            params![id, workspace_id, now],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn commit_version_updates_entity_and_emits_event() {
        let conn = fresh_conn("test_commit_basic.db").await;
        insert_entity(&conn, "ent_1", "ws_1", 100).await;

        commit_version(&conn, "ws_1", "ent_1", "blob_aaa", None, "chayan", "initial import", 100)
            .await
            .unwrap();

        let mut rows = conn
            .query("SELECT blob_ref FROM entities WHERE id='ent_1'", ())
            .await
            .unwrap();
        let blob_ref: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(blob_ref, "blob_aaa");

        let mut rows = conn
            .query("SELECT type FROM events WHERE entity_id='ent_1'", ())
            .await
            .unwrap();
        let event_type: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(event_type, "version.created");
    }

    #[tokio::test]
    async fn history_reconstructs_full_chain_oldest_first() {
        let conn = fresh_conn("test_commit_history.db").await;
        insert_entity(&conn, "ent_2", "ws_1", 100).await;

        commit_version(&conn, "ws_1", "ent_2", "blob_v1", None, "chayan", "v1", 100)
            .await
            .unwrap();
        commit_version(&conn, "ws_1", "ent_2", "blob_v2", Some("blob_v1"), "chayan", "v2", 200)
            .await
            .unwrap();
        commit_version(&conn, "ws_1", "ent_2", "blob_v3", Some("blob_v2"), "chayan", "v3", 300)
            .await
            .unwrap();

        let entries = history(&conn, "ws_1", "ent_2").await.unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].blob_ref, "blob_v1");
        assert_eq!(entries[1].blob_ref, "blob_v2");
        assert_eq!(entries[2].blob_ref, "blob_v3");
        assert_eq!(entries[2].parent_blob_ref.as_deref(), Some("blob_v2"));
    }

    #[tokio::test]
    async fn checkout_an_old_version_retrieves_its_content_via_blob_ref() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::ObjectStore::new(dir.path());
        let conn = fresh_conn("test_commit_checkout.db").await;
        insert_entity(&conn, "ent_3", "ws_1", 100).await;

        let v1_ref = crate::store_blob(&store, b"first draft").unwrap();
        commit_version(&conn, "ws_1", "ent_3", &v1_ref, None, "chayan", "v1", 100)
            .await
            .unwrap();
        let v2_ref = crate::store_blob(&store, b"second draft, much longer").unwrap();
        commit_version(&conn, "ws_1", "ent_3", &v2_ref, Some(&v1_ref), "chayan", "v2", 200)
            .await
            .unwrap();

        let entries = history(&conn, "ws_1", "ent_3").await.unwrap();
        let old_version = &entries[0];

        let checked_out = crate::read_blob(&store, &old_version.blob_ref).unwrap();
        assert_eq!(checked_out, b"first draft");
    }

    #[tokio::test]
    async fn history_is_scoped_to_workspace_and_entity() {
        let conn = fresh_conn("test_commit_scoping.db").await;
        insert_entity(&conn, "ent_4", "ws_1", 100).await;
        insert_entity(&conn, "ent_5", "ws_1", 100).await;

        commit_version(&conn, "ws_1", "ent_4", "blob_x", None, "chayan", "x", 100)
            .await
            .unwrap();
        commit_version(&conn, "ws_1", "ent_5", "blob_y", None, "chayan", "y", 100)
            .await
            .unwrap();

        let entries = history(&conn, "ws_1", "ent_4").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].blob_ref, "blob_x");
    }
}
