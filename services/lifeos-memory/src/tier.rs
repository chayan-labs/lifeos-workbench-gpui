//! Cold-tier migration + promote-on-access (issue #117, docs/AI-MEMORY.md §7).
//!
//! Forgetting is never deletion: cold, low-activation memory CONTENT moves to
//! the workspace's storage backend as a content-addressed bundle; the node
//! row stays (id, provenance, scores, a short stub) so ranking and provenance
//! are intact, and the full text is restored on access. Nodes are derived, so
//! a rebuild from `events` naturally un-tiers them - tiering is a storage
//! optimization, not a semantic state.

use crate::error::MemoryError;
use libsql::{params, Connection};
use lifeos_vcs::StorageBackend;
use std::collections::BTreeMap;

const STUB_CHARS: usize = 64;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TierReport {
    pub tiered: usize,
    pub blob_ref: Option<String>,
}

/// Cold = old, unimportant, and rarely accessed - all thresholds explicit so
/// the sweep is auditable.
pub async fn find_cold_nodes(
    conn: &Connection,
    workspace_id: &str,
    now: i64,
    min_age_secs: i64,
    max_importance: f64,
    max_access_count: i64,
) -> Result<Vec<String>, MemoryError> {
    let mut rows = conn
        .query(
            "SELECT id FROM memory_nodes \
             WHERE workspace_id = ?1 AND tiered_ref IS NULL \
               AND ts < ?2 AND importance <= ?3 AND access_count <= ?4 \
             ORDER BY id",
            params![workspace_id, now - min_age_secs, max_importance, max_access_count],
        )
        .await?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next().await? {
        ids.push(row.get::<String>(0)?);
    }
    Ok(ids)
}

/// Bundle the given nodes' full content into one content-addressed blob on
/// the backend, then replace each row's content with a stub + `tiered_ref`.
pub async fn tier_out_cold(
    conn: &Connection,
    workspace_id: &str,
    backend: &dyn StorageBackend,
    node_ids: &[String],
) -> Result<TierReport, MemoryError> {
    if node_ids.is_empty() {
        return Ok(TierReport { tiered: 0, blob_ref: None });
    }
    // BTreeMap => deterministic bundle bytes => stable content address.
    let mut bundle: BTreeMap<String, String> = BTreeMap::new();
    for id in node_ids {
        let mut rows = conn
            .query(
                "SELECT content FROM memory_nodes \
                 WHERE workspace_id = ?1 AND id = ?2 AND tiered_ref IS NULL",
                params![workspace_id, id.clone()],
            )
            .await?;
        if let Some(row) = rows.next().await? {
            bundle.insert(id.clone(), row.get(0)?);
        }
    }
    if bundle.is_empty() {
        return Ok(TierReport { tiered: 0, blob_ref: None });
    }

    let bytes = serde_json::to_vec(&bundle).map_err(|e| MemoryError::Other(e.to_string()))?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    backend.put(&hash, &bytes).await?;

    let mut tiered = 0;
    for (id, content) in &bundle {
        let stub: String = content.chars().take(STUB_CHARS).collect();
        let n = conn
            .execute(
                "UPDATE memory_nodes SET content = ?1, tiered_ref = ?2 \
                 WHERE workspace_id = ?3 AND id = ?4",
                params![format!("{stub}… [tiered]"), hash.clone(), workspace_id, id.clone()],
            )
            .await?;
        tiered += n as usize;
    }
    Ok(TierReport { tiered, blob_ref: Some(hash) })
}

/// Promote-on-access: restore full content for any of the given nodes that
/// are tiered. Returns how many were restored.
pub async fn promote_nodes(
    conn: &Connection,
    workspace_id: &str,
    backend: &dyn StorageBackend,
    node_ids: &[String],
) -> Result<usize, MemoryError> {
    let mut promoted = 0;
    for id in node_ids {
        let mut rows = conn
            .query(
                "SELECT tiered_ref FROM memory_nodes \
                 WHERE workspace_id = ?1 AND id = ?2 AND tiered_ref IS NOT NULL",
                params![workspace_id, id.clone()],
            )
            .await?;
        let Some(row) = rows.next().await? else { continue };
        let blob_ref: String = row.get(0)?;
        let bytes = backend.get(&blob_ref).await?;
        let bundle: BTreeMap<String, String> =
            serde_json::from_slice(&bytes).map_err(|e| MemoryError::Other(e.to_string()))?;
        let Some(content) = bundle.get(id) else { continue };
        promoted += conn
            .execute(
                "UPDATE memory_nodes SET content = ?1, tiered_ref = NULL \
                 WHERE workspace_id = ?2 AND id = ?3",
                params![content.clone(), workspace_id, id.clone()],
            )
            .await? as usize;
    }
    Ok(promoted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::project_workspace;
    use crate::testutil::{seed_event, test_conn};
    use lifeos_vcs::LocalFsBackend;
    use serde_json::json;

    #[tokio::test]
    async fn cold_nodes_tier_out_and_promote_back() {
        let conn = test_conn().await;
        seed_event(
            &conn, "ws_1", "evt_cold", 1000, "note.captured", None, "user",
            json!({"text": "an ancient rarely-used observation about ferns"}), None,
        )
        .await;
        project_workspace(&conn, "ws_1").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsBackend::new(dir.path());

        let now = 1000 + 100 * 86400;
        let cold = find_cold_nodes(&conn, "ws_1", now, 30 * 86400, 0.5, 2).await.unwrap();
        assert_eq!(cold.len(), 1);

        let report = tier_out_cold(&conn, "ws_1", &backend, &cold).await.unwrap();
        assert_eq!(report.tiered, 1);
        let blob_ref = report.blob_ref.unwrap();

        // The row survived (provenance intact), content is a stub.
        let mut rows = conn
            .query(
                "SELECT content, tiered_ref, source_event_ids FROM memory_nodes WHERE id = ?1",
                params![cold[0].clone()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let content: String = row.get(0).unwrap();
        assert!(content.contains("[tiered]"));
        assert_eq!(row.get::<String>(1).unwrap(), blob_ref);
        assert!(row.get::<String>(2).unwrap().contains("evt_cold"), "provenance never leaves");

        // Promote-on-access restores the full text.
        let promoted = promote_nodes(&conn, "ws_1", &backend, &cold).await.unwrap();
        assert_eq!(promoted, 1);
        let mut rows = conn
            .query("SELECT content, tiered_ref FROM memory_nodes WHERE id = ?1", params![cold[0].clone()])
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert!(row.get::<String>(0).unwrap().contains("ancient rarely-used observation"));
        assert!(row.get::<Option<String>>(1).unwrap().is_none());
    }
}
