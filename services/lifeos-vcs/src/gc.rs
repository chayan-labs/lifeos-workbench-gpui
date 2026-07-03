//! GC: mark-and-sweep against live snapshots (issue #84).
//!
//! Per docs/AGENT-CONTROL.md §1, GC on `lifeos-vcs` is a hard-denied domain
//! for the agent - `mark_and_sweep` is a library primitive meant to be
//! invoked by a human/CLI caller (issue #86), never wrapped as an
//! agent-callable tool and never run automatically on a commit path.
//!
//! "Live" is deliberately broader than just branch/tag snapshots: an
//! entity's *current* `blob_ref` (its live checked-out version, whether or
//! not anyone has taken a snapshot of it) must also survive, or committing a
//! blob and running GC before ever snapshotting would silently destroy it.

use std::collections::HashSet;
use std::fmt;

use libsql::Connection;

use crate::blob::BlobManifest;
use crate::snapshot::{all_ref_snapshots, read_snapshot, SnapshotError};
use crate::store::ObjectStore;

#[derive(Debug)]
pub enum GcError {
    Db(libsql::Error),
    Io(std::io::Error),
}

impl fmt::Display for GcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GcError::Db(e) => write!(f, "db error: {e}"),
            GcError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for GcError {}

/// `all_ref_snapshots` only ever produces `SnapshotError::Db` in practice
/// (it does no manifest parsing), but map the other variants defensively
/// instead of assuming that invariant holds forever.
fn snapshot_err_to_gc_err(e: SnapshotError) -> GcError {
    match e {
        SnapshotError::Db(e) => GcError::Db(e),
        SnapshotError::Io(e) => GcError::Io(e),
        SnapshotError::Serde(e) => GcError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        SnapshotError::TagImmutable { name } => {
            GcError::Io(std::io::Error::other(format!("unexpected tag error for {name}")))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcReport {
    pub scanned: usize,
    pub reclaimed: usize,
}

fn mark_blob_live(store: &ObjectStore, blob_ref: &str, live: &mut HashSet<String>) {
    if !live.insert(blob_ref.to_string()) {
        return; // already processed via another reference
    }
    if let Ok(manifest_bytes) = store.read_object(blob_ref) {
        if let Ok(manifest) = serde_json::from_slice::<BlobManifest>(&manifest_bytes) {
            for chunk in &manifest.chunks {
                live.insert(chunk.hash.clone());
            }
        }
    }
}

/// Computes every object hash still reachable from live state: every
/// entity's current `blob_ref` across every workspace, plus every
/// branch/tag's snapshot and everything each snapshot's manifest references.
pub async fn live_object_hashes(conn: &Connection, store: &ObjectStore) -> Result<HashSet<String>, GcError> {
    let mut live = HashSet::new();

    let mut rows = conn
        .query("SELECT blob_ref FROM entities WHERE blob_ref IS NOT NULL", ())
        .await
        .map_err(GcError::Db)?;
    while let Some(row) = rows.next().await.map_err(GcError::Db)? {
        let blob_ref: String = row.get(0).map_err(GcError::Db)?;
        mark_blob_live(store, &blob_ref, &mut live);
    }

    for snapshot_ref in all_ref_snapshots(conn).await.map_err(snapshot_err_to_gc_err)? {
        if !live.insert(snapshot_ref.clone()) {
            continue;
        }
        if let Ok(manifest) = read_snapshot(store, &snapshot_ref) {
            for blob_ref in manifest.entries.values() {
                mark_blob_live(store, blob_ref, &mut live);
            }
        }
    }

    Ok(live)
}

/// Sweeps every object under `store`'s root not reachable from live state.
/// Mark phase is `live_object_hashes`; sweep deletes anything else found on
/// disk. Never called automatically - a human/CLI-triggered maintenance op.
pub async fn mark_and_sweep(conn: &Connection, store: &ObjectStore) -> Result<GcReport, GcError> {
    let live = live_object_hashes(conn, store).await?;

    let mut scanned = 0;
    let mut reclaimed = 0;
    for entry in walkdir::WalkDir::new(store.root().join("objects"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        scanned += 1;
        let hash = entry.file_name().to_string_lossy().to_string();
        if !live.contains(&hash) {
            std::fs::remove_file(entry.path()).map_err(GcError::Io)?;
            reclaimed += 1;
        }
    }

    Ok(GcReport { scanned, reclaimed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::store_blob;
    use crate::commit::commit_version;
    use crate::snapshot::{create_snapshot, set_branch};
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

    async fn insert_entity(conn: &Connection, id: &str, workspace_id: &str, now: i64) {
        conn.execute(
            "INSERT INTO entities (id, workspace_id, blob_ref, updated_at) VALUES (?1, ?2, NULL, ?3)",
            libsql::params![id, workspace_id, now],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn gc_reclaims_only_unreferenced_objects() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_gc_basic.db").await;
        insert_entity(&conn, "ent_1", "ws_1", 100).await;

        let live_ref = store_blob(&store, b"this stays - it's the entity's current version").unwrap();
        commit_version(&conn, "ws_1", "ent_1", &live_ref, None, "chayan", "keep", 100)
            .await
            .unwrap();

        let orphan_ref = store_blob(&store, b"this is orphaned - no entity/snapshot points at it").unwrap();
        assert!(store.has_object(&orphan_ref));

        let report = mark_and_sweep(&conn, &store).await.unwrap();

        // Each small blob is stored as 2 objects (its manifest + its one
        // chunk) - the orphan's manifest and chunk are both unreferenced.
        assert_eq!(report.reclaimed, 2);
        assert!(store.has_object(&live_ref));
        assert!(!store.has_object(&orphan_ref));
    }

    #[tokio::test]
    async fn gc_preserves_objects_referenced_only_by_a_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_gc_snapshot.db").await;
        insert_entity(&conn, "ent_1", "ws_1", 100).await;

        let old_ref = store_blob(&store, b"v1 content").unwrap();
        commit_version(&conn, "ws_1", "ent_1", &old_ref, None, "chayan", "v1", 100)
            .await
            .unwrap();
        let snapshot_ref = create_snapshot(&conn, &store, "ws_1").await.unwrap();
        set_branch(&conn, "ws_1", "main", &snapshot_ref, 100).await.unwrap();

        // Entity moves on to v2; v1 is no longer the entity's current blob_ref
        // but IS still referenced by the "main" branch's snapshot.
        let new_ref = store_blob(&store, b"v2 content").unwrap();
        commit_version(&conn, "ws_1", "ent_1", &new_ref, Some(&old_ref), "chayan", "v2", 200)
            .await
            .unwrap();

        let report = mark_and_sweep(&conn, &store).await.unwrap();

        assert_eq!(report.reclaimed, 0);
        assert!(store.has_object(&old_ref));
        assert!(store.has_object(&new_ref));
        assert!(store.has_object(&snapshot_ref));
    }

    #[tokio::test]
    async fn gc_reports_zero_reclaimed_when_nothing_is_orphaned() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_gc_clean.db").await;
        insert_entity(&conn, "ent_1", "ws_1", 100).await;

        let blob_ref = store_blob(&store, b"only version, still live").unwrap();
        commit_version(&conn, "ws_1", "ent_1", &blob_ref, None, "chayan", "v1", 100)
            .await
            .unwrap();

        let report = mark_and_sweep(&conn, &store).await.unwrap();

        assert_eq!(report.reclaimed, 0);
        assert!(report.scanned > 0);
    }
}
