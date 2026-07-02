//! Backend index (issue #106, docs/STORAGE-BACKENDS.md §2/§7): maps
//! `blob_ref`/chunk hash <-> the backend-native locator for each configured
//! backend. Path-shaped backends (local-fs, S3-compatible) can derive the
//! locator from the hash, but id-shaped backends (Google Drive file ids,
//! Dropbox ids) cannot - the index is what lets a `get` on those backends
//! resolve a hash at all, and what lets migration enumerate exactly which
//! objects a backend holds.
//!
//! Lives in the canonical DB (metadata only - never bytes), following the
//! same standalone-`Connection` convention as `commit.rs`: this crate has no
//! dependency on `lifeos-api`; the table is created by migration
//! `0014_blob_backends.sql`.

use libsql::{params, Connection};

/// Records (or refreshes) where `hash` lives on `backend_id`. Idempotent:
/// re-recording overwrites the locator, matching CAS re-put semantics.
pub async fn record_location(
    conn: &Connection,
    workspace_id: &str,
    backend_id: &str,
    hash: &str,
    locator: &str,
    now: i64,
) -> libsql::Result<()> {
    conn.execute(
        "INSERT INTO blob_backends (workspace_id, backend_id, hash, locator, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(workspace_id, backend_id, hash) DO UPDATE SET locator=excluded.locator",
        params![workspace_id, backend_id, hash, locator, now],
    )
    .await?;
    Ok(())
}

/// The backend-native locator for `hash` on `backend_id`, if indexed.
pub async fn location_for(
    conn: &Connection,
    workspace_id: &str,
    backend_id: &str,
    hash: &str,
) -> libsql::Result<Option<String>> {
    let mut rows = conn
        .query(
            "SELECT locator FROM blob_backends \
             WHERE workspace_id=?1 AND backend_id=?2 AND hash=?3",
            params![workspace_id, backend_id, hash],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(row.get::<String>(0)?)),
        None => Ok(None),
    }
}

/// Every hash the index says `backend_id` holds - what a migration job
/// diffs against the live set to find what still needs re-putting.
pub async fn hashes_on_backend(
    conn: &Connection,
    workspace_id: &str,
    backend_id: &str,
) -> libsql::Result<Vec<String>> {
    let mut rows = conn
        .query(
            "SELECT hash FROM blob_backends WHERE workspace_id=?1 AND backend_id=?2 ORDER BY hash",
            params![workspace_id, backend_id],
        )
        .await?;
    let mut hashes = Vec::new();
    while let Some(row) = rows.next().await? {
        hashes.push(row.get::<String>(0)?);
    }
    Ok(hashes)
}

/// Drops the index rows for `backend_id` objects that were GC'd or migrated
/// away. Index maintenance only - the objects themselves are deleted through
/// [`crate::StorageBackend::delete`], which is GC-only and never
/// agent-callable.
pub async fn forget_location(
    conn: &Connection,
    workspace_id: &str,
    backend_id: &str,
    hash: &str,
) -> libsql::Result<()> {
    conn.execute(
        "DELETE FROM blob_backends WHERE workspace_id=?1 AND backend_id=?2 AND hash=?3",
        params![workspace_id, backend_id, hash],
    )
    .await?;
    Ok(())
}

#[cfg(test)]
pub(crate) async fn create_table_for_tests(conn: &Connection) {
    // The real schema's FK target; this crate's tests run against a bare DB
    // (same convention as commit.rs) so stub the referenced table first.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workspaces (id TEXT PRIMARY KEY, name TEXT)",
        (),
    )
    .await
    .unwrap();
    conn.execute_batch(include_str!("../../../migrations/0014_blob_backends.sql"))
        .await
        .unwrap();
    for ws in ["ws_1", "ws_2"] {
        conn.execute("INSERT OR IGNORE INTO workspaces (id, name) VALUES (?1, ?1)", params![ws])
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libsql::Builder;

    async fn fresh_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db = Builder::new_local(dir.path().join("test.db").to_str().unwrap())
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        create_table_for_tests(&conn).await;
        (dir, conn)
    }

    #[tokio::test]
    async fn records_and_resolves_a_backend_native_locator() {
        let (_dir, conn) = fresh_conn().await;

        record_location(&conn, "ws_1", "drive_main", "b3hash", "drive-file-id-123", 100)
            .await
            .unwrap();

        let locator = location_for(&conn, "ws_1", "drive_main", "b3hash").await.unwrap();
        assert_eq!(locator.as_deref(), Some("drive-file-id-123"));
    }

    #[tokio::test]
    async fn re_recording_updates_the_locator_idempotently() {
        let (_dir, conn) = fresh_conn().await;

        record_location(&conn, "ws_1", "drive_main", "b3hash", "old-id", 100).await.unwrap();
        record_location(&conn, "ws_1", "drive_main", "b3hash", "new-id", 200).await.unwrap();

        let locator = location_for(&conn, "ws_1", "drive_main", "b3hash").await.unwrap();
        assert_eq!(locator.as_deref(), Some("new-id"));
    }

    #[tokio::test]
    async fn index_is_scoped_by_workspace_and_backend() {
        let (_dir, conn) = fresh_conn().await;
        record_location(&conn, "ws_1", "drive_main", "h1", "id-1", 100).await.unwrap();
        record_location(&conn, "ws_2", "drive_main", "h2", "id-2", 100).await.unwrap();
        record_location(&conn, "ws_1", "s3_backup", "h3", "id-3", 100).await.unwrap();

        assert_eq!(location_for(&conn, "ws_2", "drive_main", "h1").await.unwrap(), None);
        assert_eq!(location_for(&conn, "ws_1", "s3_backup", "h1").await.unwrap(), None);
        assert_eq!(
            hashes_on_backend(&conn, "ws_1", "drive_main").await.unwrap(),
            vec!["h1".to_string()]
        );
    }

    #[tokio::test]
    async fn forget_location_removes_only_the_named_row() {
        let (_dir, conn) = fresh_conn().await;
        record_location(&conn, "ws_1", "drive_main", "h1", "id-1", 100).await.unwrap();
        record_location(&conn, "ws_1", "drive_main", "h2", "id-2", 100).await.unwrap();

        forget_location(&conn, "ws_1", "drive_main", "h1").await.unwrap();

        assert_eq!(location_for(&conn, "ws_1", "drive_main", "h1").await.unwrap(), None);
        assert_eq!(location_for(&conn, "ws_1", "drive_main", "h2").await.unwrap().as_deref(), Some("id-2"));
    }
}
