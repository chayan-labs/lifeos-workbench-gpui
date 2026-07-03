//! Snapshot/branch/tag, jj-style (issue #84, docs/VERSIONING.md §2.4).
//!
//! A snapshot is a content-addressed Merkle manifest of `{entity_id ->
//! blob_ref}` across a workspace at a point in time - itself just another
//! CAS object, so it dedups and integrity-checks exactly like a blob does.
//! A branch is a named, moving pointer to a snapshot; a tag is a fixed
//! pointer that refuses to move once set.
//!
//! Per docs/AGENT-CONTROL.md §1, VCS internals (branch-force, rewrite, GC)
//! are a hard-denied domain for the agent - these functions are library
//! primitives for a human/CLI caller (issue #86), never wrapped as an
//! agent-callable tool.

use std::collections::BTreeMap;
use std::fmt;

use libsql::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::hash::hash_bytes;
use crate::store::ObjectStore;

#[derive(Debug)]
pub enum SnapshotError {
    Db(libsql::Error),
    Io(std::io::Error),
    Serde(serde_json::Error),
    /// A tag is a fixed pointer - refuses to move once set to a different snapshot.
    TagImmutable { name: String },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnapshotError::Db(e) => write!(f, "db error: {e}"),
            SnapshotError::Io(e) => write!(f, "io error: {e}"),
            SnapshotError::Serde(e) => write!(f, "serde error: {e}"),
            SnapshotError::TagImmutable { name } => {
                write!(f, "tag \"{name}\" already points elsewhere and cannot be moved")
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

/// `entity_id -> blob_ref` for every file-bearing entity captured in the
/// snapshot. `BTreeMap` gives deterministic key order so identical workspace
/// state always hashes to the same `snapshot_ref`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SnapshotManifest {
    pub entries: BTreeMap<String, String>,
}

/// Captures every entity's current `blob_ref` in `workspace_id` into a
/// snapshot manifest, stores it as a CAS object, and returns its content
/// address.
pub async fn create_snapshot(
    conn: &Connection,
    store: &ObjectStore,
    workspace_id: &str,
) -> Result<String, SnapshotError> {
    let mut rows = conn
        .query(
            "SELECT id, blob_ref FROM entities WHERE workspace_id=?1 AND blob_ref IS NOT NULL",
            params![workspace_id],
        )
        .await
        .map_err(SnapshotError::Db)?;

    let mut entries = BTreeMap::new();
    while let Some(row) = rows.next().await.map_err(SnapshotError::Db)? {
        let id: String = row.get(0).map_err(SnapshotError::Db)?;
        let blob_ref: String = row.get(1).map_err(SnapshotError::Db)?;
        entries.insert(id, blob_ref);
    }

    let manifest = SnapshotManifest { entries };
    let manifest_json = serde_json::to_vec(&manifest).map_err(SnapshotError::Serde)?;
    let snapshot_ref = hash_bytes(&manifest_json);
    store.write_object(&snapshot_ref, &manifest_json).map_err(SnapshotError::Io)?;

    Ok(snapshot_ref)
}

pub fn read_snapshot(store: &ObjectStore, snapshot_ref: &str) -> Result<SnapshotManifest, SnapshotError> {
    let bytes = store.read_object(snapshot_ref).map_err(SnapshotError::Io)?;
    serde_json::from_slice(&bytes).map_err(SnapshotError::Serde)
}

/// Points a branch (a moving pointer) at `snapshot_ref`, creating it if new.
pub async fn set_branch(
    conn: &Connection,
    workspace_id: &str,
    name: &str,
    snapshot_ref: &str,
    now: i64,
) -> Result<(), SnapshotError> {
    conn.execute(
        "INSERT INTO vcs_refs (workspace_id, kind, name, snapshot_ref, updated_at) \
         VALUES (?1, 'branch', ?2, ?3, ?4) \
         ON CONFLICT(workspace_id, kind, name) DO UPDATE SET snapshot_ref=excluded.snapshot_ref, updated_at=excluded.updated_at",
        params![workspace_id, name, snapshot_ref, now],
    )
    .await
    .map_err(SnapshotError::Db)?;
    Ok(())
}

/// Points a tag (a fixed pointer) at `snapshot_ref`. Errors if the tag
/// already exists pointing at a *different* snapshot - setting it again to
/// the same snapshot is an idempotent no-op.
pub async fn set_tag(
    conn: &Connection,
    workspace_id: &str,
    name: &str,
    snapshot_ref: &str,
    now: i64,
) -> Result<(), SnapshotError> {
    if let Some(existing) = get_ref(conn, workspace_id, "tag", name).await? {
        if existing != snapshot_ref {
            return Err(SnapshotError::TagImmutable { name: name.to_string() });
        }
        return Ok(());
    }
    conn.execute(
        "INSERT INTO vcs_refs (workspace_id, kind, name, snapshot_ref, updated_at) VALUES (?1, 'tag', ?2, ?3, ?4)",
        params![workspace_id, name, snapshot_ref, now],
    )
    .await
    .map_err(SnapshotError::Db)?;
    Ok(())
}

/// Resolves a branch or tag name to its current `snapshot_ref`.
pub async fn get_ref(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    name: &str,
) -> Result<Option<String>, SnapshotError> {
    let mut rows = conn
        .query(
            "SELECT snapshot_ref FROM vcs_refs WHERE workspace_id=?1 AND kind=?2 AND name=?3",
            params![workspace_id, kind, name],
        )
        .await
        .map_err(SnapshotError::Db)?;
    match rows.next().await.map_err(SnapshotError::Db)? {
        Some(row) => Ok(Some(row.get(0).map_err(SnapshotError::Db)?)),
        None => Ok(None),
    }
}

/// A single branch/tag pointer, as returned to a listing caller (issue #87's
/// read-only branch/tag UI - forward-only, matches docs/AGENT-CONTROL.md §1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefEntry {
    pub name: String,
    pub snapshot_ref: String,
    pub updated_at: i64,
}

/// Lists every branch or tag in a workspace, newest first.
pub async fn list_refs(conn: &Connection, workspace_id: &str, kind: &str) -> Result<Vec<RefEntry>, SnapshotError> {
    let mut rows = conn
        .query(
            "SELECT name, snapshot_ref, updated_at FROM vcs_refs \
             WHERE workspace_id=?1 AND kind=?2 ORDER BY updated_at DESC",
            params![workspace_id, kind],
        )
        .await
        .map_err(SnapshotError::Db)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(SnapshotError::Db)? {
        out.push(RefEntry {
            name: row.get(0).map_err(SnapshotError::Db)?,
            snapshot_ref: row.get(1).map_err(SnapshotError::Db)?,
            updated_at: row.get(2).map_err(SnapshotError::Db)?,
        });
    }
    Ok(out)
}

/// All `snapshot_ref`s currently pointed at by any branch or tag, across
/// every workspace. Used by `gc::mark_and_sweep` to compute the live set.
pub async fn all_ref_snapshots(conn: &Connection) -> Result<Vec<String>, SnapshotError> {
    let mut rows = conn
        .query("SELECT snapshot_ref FROM vcs_refs", ())
        .await
        .map_err(SnapshotError::Db)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(SnapshotError::Db)? {
        out.push(row.get(0).map_err(SnapshotError::Db)?);
    }
    Ok(out)
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
                id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, blob_ref TEXT)",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "CREATE TABLE vcs_refs (\
                workspace_id TEXT NOT NULL, kind TEXT NOT NULL, name TEXT NOT NULL, \
                snapshot_ref TEXT NOT NULL, updated_at INTEGER NOT NULL, \
                PRIMARY KEY (workspace_id, kind, name))",
            (),
        )
        .await
        .unwrap();
        conn
    }

    async fn insert_entity(conn: &Connection, id: &str, workspace_id: &str, blob_ref: Option<&str>) {
        conn.execute(
            "INSERT INTO entities (id, workspace_id, blob_ref) VALUES (?1, ?2, ?3)",
            params![id, workspace_id, blob_ref],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn snapshot_captures_every_entity_with_a_blob_ref() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_snap_capture.db").await;
        insert_entity(&conn, "ent_a", "ws_1", Some("blob_a")).await;
        insert_entity(&conn, "ent_b", "ws_1", Some("blob_b")).await;
        insert_entity(&conn, "ent_c", "ws_1", None).await; // no blob yet, excluded

        let snapshot_ref = create_snapshot(&conn, &store, "ws_1").await.unwrap();
        let manifest = read_snapshot(&store, &snapshot_ref).unwrap();

        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.entries.get("ent_a"), Some(&"blob_a".to_string()));
        assert_eq!(manifest.entries.get("ent_b"), Some(&"blob_b".to_string()));
    }

    #[tokio::test]
    async fn identical_workspace_state_produces_the_same_snapshot_ref() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_snap_dedup.db").await;
        insert_entity(&conn, "ent_a", "ws_1", Some("blob_a")).await;

        let first = create_snapshot(&conn, &store, "ws_1").await.unwrap();
        let second = create_snapshot(&conn, &store, "ws_1").await.unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn branch_moves_freely() {
        let conn = fresh_conn("test_branch_move.db").await;

        set_branch(&conn, "ws_1", "main", "snap_1", 100).await.unwrap();
        assert_eq!(get_ref(&conn, "ws_1", "branch", "main").await.unwrap(), Some("snap_1".to_string()));

        set_branch(&conn, "ws_1", "main", "snap_2", 200).await.unwrap();
        assert_eq!(get_ref(&conn, "ws_1", "branch", "main").await.unwrap(), Some("snap_2".to_string()));
    }

    #[tokio::test]
    async fn tag_refuses_to_move() {
        let conn = fresh_conn("test_tag_immutable.db").await;

        set_tag(&conn, "ws_1", "v1", "snap_1", 100).await.unwrap();
        let result = set_tag(&conn, "ws_1", "v1", "snap_2", 200).await;

        assert!(matches!(result, Err(SnapshotError::TagImmutable { .. })));
        assert_eq!(get_ref(&conn, "ws_1", "tag", "v1").await.unwrap(), Some("snap_1".to_string()));
    }

    #[tokio::test]
    async fn setting_a_tag_to_the_same_snapshot_again_is_a_no_op() {
        let conn = fresh_conn("test_tag_idempotent.db").await;

        set_tag(&conn, "ws_1", "v1", "snap_1", 100).await.unwrap();
        let result = set_tag(&conn, "ws_1", "v1", "snap_1", 200).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn list_refs_returns_only_the_requested_kind_newest_first() {
        let conn = fresh_conn("test_list_refs.db").await;

        set_branch(&conn, "ws_1", "main", "snap_1", 100).await.unwrap();
        set_branch(&conn, "ws_1", "experiment", "snap_2", 200).await.unwrap();
        set_tag(&conn, "ws_1", "v1", "snap_3", 150).await.unwrap();

        let branches = list_refs(&conn, "ws_1", "branch").await.unwrap();
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].name, "experiment");
        assert_eq!(branches[1].name, "main");

        let tags = list_refs(&conn, "ws_1", "tag").await.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v1");
    }

    #[tokio::test]
    async fn branches_and_tags_share_a_name_in_separate_namespaces() {
        let conn = fresh_conn("test_ref_namespaces.db").await;

        set_branch(&conn, "ws_1", "release", "snap_branch", 100).await.unwrap();
        set_tag(&conn, "ws_1", "release", "snap_tag", 100).await.unwrap();

        assert_eq!(get_ref(&conn, "ws_1", "branch", "release").await.unwrap(), Some("snap_branch".to_string()));
        assert_eq!(get_ref(&conn, "ws_1", "tag", "release").await.unwrap(), Some("snap_tag".to_string()));
    }
}
