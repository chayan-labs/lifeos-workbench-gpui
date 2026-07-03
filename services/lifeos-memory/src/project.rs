//! Event-sourced projection (issues #112/#111, docs/AI-MEMORY.md §2-3).
//!
//! `events` is the single source of truth; this module folds it - in strict
//! (ts, id) order - into the memory read models. Every derived row id is a
//! BLAKE3 of stable inputs, every mutation of the read models is driven by an
//! event, and consolidation's own outputs are replayed from `memory.*` events
//! rather than recomputed - so `rebuild_workspace` is deterministic: wipe the
//! read models, fold the log again, get identical rows.
//!
//! Events arrive here from EVERY tier (Telegram bot, platform UI/API, Mac
//! harness, terminal hooks, drain jobs) because they all append to the same
//! `events` table; the projector keys on nothing source-specific.

use crate::error::MemoryError;
use crate::redact::{flatten_redacted, is_secret_event_type};
use libsql::{params, Connection};

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectionStats {
    pub nodes: usize,
    pub edges: usize,
    pub summaries: usize,
    pub rules: usize,
    pub superseded: usize,
    pub skipped: usize,
}

/// One upcast event row, in the current (v1) shape.
#[derive(Debug, Clone)]
pub struct EventRecord {
    pub id: String,
    pub ts: i64,
    pub event_type: String,
    pub entity_id: Option<String>,
    pub actor: Option<String>,
    pub attrs: serde_json::Value,
    pub caused_by_event_id: Option<String>,
    pub schema_version: i64,
}

/// Versioned upcasters (§2): old payload shapes are rewritten to the current
/// one before folding, so ancient events stay replayable forever. v0 is the
/// pre-#111 era (no schema_version column -> NULL -> treated as 0): identical
/// payload shape, so its upcast is the identity; future shape changes add
/// arms here instead of migrating the log.
fn upcast(mut ev: EventRecord) -> EventRecord {
    if ev.schema_version < 1 {
        ev.schema_version = 1;
    }
    ev
}

fn short_hash(input: &str) -> String {
    blake3::hash(input.as_bytes()).to_hex()[..24].to_string()
}

/// Deterministic memory-node id for the node derived from one event.
pub fn node_id(workspace_id: &str, event_id: &str) -> String {
    format!("mn_{}", short_hash(&format!("{workspace_id}|{event_id}")))
}

fn edge_id(workspace_id: &str, source_event_id: &str, from: &str, to: &str, rel: &str) -> String {
    format!("me_{}", short_hash(&format!("{workspace_id}|{source_event_id}|{from}|{to}|{rel}")))
}

fn summary_id(workspace_id: &str, event_id: &str) -> String {
    format!("ms_{}", short_hash(&format!("{workspace_id}|{event_id}")))
}

fn rule_id(workspace_id: &str, event_id: &str) -> String {
    format!("mr_{}", short_hash(&format!("{workspace_id}|{event_id}")))
}

/// Write-time salience (§4: "importance scored once at write time"). Cheap,
/// deterministic heuristic; consolidation's `score_importance` refines it
/// later via memory.importance.scored events.
fn write_time_importance(event_type: &str, content: &str) -> f64 {
    const SALIENT_MARKERS: &[&str] =
        &["completed", "closed", "published", "error", "failed", "decision", "installed"];
    let mut importance: f64 = 0.3;
    if SALIENT_MARKERS.iter().any(|m| event_type.contains(m)) {
        importance += 0.2;
    }
    if content.chars().count() > 200 {
        importance += 0.1;
    }
    importance.clamp(0.0, 1.0)
}

/// Incremental projection: fold every event past the workspace's cursor.
pub async fn project_workspace(
    conn: &Connection,
    workspace_id: &str,
) -> Result<ProjectionStats, MemoryError> {
    let mut stats = ProjectionStats::default();
    let (mut cur_ts, mut cur_id) = read_cursor(conn, workspace_id).await?;

    loop {
        let batch = fetch_events_after(conn, workspace_id, cur_ts, &cur_id, 500).await?;
        if batch.is_empty() {
            break;
        }
        for ev in batch {
            cur_ts = ev.ts;
            cur_id = ev.id.clone();
            apply_event(conn, workspace_id, &upcast(ev), &mut stats).await?;
        }
    }

    write_cursor(conn, workspace_id, cur_ts, &cur_id).await?;
    Ok(stats)
}

/// The must-pass invariant path (§11): wipe the read models (and the derived
/// FTS mirror) and fold the whole log again. The consolidation cursor is
/// deliberately NOT reset - consolidation's outputs are already events in the
/// log and are replayed, not re-derived.
pub async fn rebuild_workspace(
    conn: &Connection,
    workspace_id: &str,
) -> Result<ProjectionStats, MemoryError> {
    for table in ["memory_nodes", "memory_edges", "memory_summaries", "memory_rules"] {
        conn.execute(
            &format!("DELETE FROM {table} WHERE workspace_id = ?1"),
            params![workspace_id],
        )
        .await?;
    }
    // Best-effort: the derived DB may not be attached (e.g. drain).
    let _ = conn
        .execute("DELETE FROM d.memory_idx WHERE workspace_id = ?1", params![workspace_id])
        .await;
    conn.execute(
        "UPDATE memory_cursors SET projected_ts = 0, projected_id = '' WHERE workspace_id = ?1",
        params![workspace_id],
    )
    .await?;
    project_workspace(conn, workspace_id).await
}

async fn read_cursor(conn: &Connection, ws: &str) -> Result<(i64, String), MemoryError> {
    let mut rows = conn
        .query(
            "SELECT projected_ts, projected_id FROM memory_cursors WHERE workspace_id = ?1",
            params![ws],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok((row.get(0)?, row.get(1)?)),
        None => Ok((0, String::new())),
    }
}

async fn write_cursor(conn: &Connection, ws: &str, ts: i64, id: &str) -> Result<(), MemoryError> {
    conn.execute(
        "INSERT INTO memory_cursors (workspace_id, projected_ts, projected_id, updated_at) \
         VALUES (?1, ?2, ?3, ?2) \
         ON CONFLICT(workspace_id) DO UPDATE SET \
            projected_ts = excluded.projected_ts, projected_id = excluded.projected_id, \
            updated_at = excluded.updated_at",
        params![ws, ts, id],
    )
    .await?;
    Ok(())
}

/// Strict (ts, id) replay order - the same total order on every rebuild.
pub(crate) async fn fetch_events_after(
    conn: &Connection,
    ws: &str,
    after_ts: i64,
    after_id: &str,
    limit: u32,
) -> Result<Vec<EventRecord>, MemoryError> {
    let mut rows = conn
        .query(
            "SELECT id, ts, type, entity_id, actor, attrs, caused_by_event_id, \
                    COALESCE(schema_version, 0) \
             FROM events WHERE workspace_id = ?1 AND (ts > ?2 OR (ts = ?2 AND id > ?3)) \
             ORDER BY ts ASC, id ASC LIMIT ?4",
            params![ws, after_ts, after_id, limit],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let attrs_raw: Option<String> = row.get(5)?;
        out.push(EventRecord {
            id: row.get(0)?,
            ts: row.get(1)?,
            event_type: row.get(2)?,
            entity_id: row.get(3)?,
            actor: row.get(4)?,
            attrs: attrs_raw
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null),
            caused_by_event_id: row.get(6)?,
            schema_version: row.get(7)?,
        });
    }
    Ok(out)
}

async fn apply_event(
    conn: &Connection,
    ws: &str,
    ev: &EventRecord,
    stats: &mut ProjectionStats,
) -> Result<(), MemoryError> {
    if is_secret_event_type(&ev.event_type) {
        stats.skipped += 1;
        return Ok(());
    }
    match ev.event_type.as_str() {
        "memory.summary.created" => apply_summary(conn, ws, ev, stats).await,
        "memory.rule.added" => apply_rule_added(conn, ws, ev, stats).await,
        "memory.rule.retired" => apply_rule_retired(conn, ws, ev, stats).await,
        "memory.importance.scored" => apply_importance(conn, ws, ev, stats).await,
        "memory.supersede.detected" => {
            let old = ev.attrs.get("old_event_id").and_then(|v| v.as_str()).unwrap_or("");
            // Validity ends when the NEW fact became true (its event ts,
            // carried in attrs), not when the detector happened to run.
            let t_invalid = ev.attrs.get("t_invalid").and_then(|v| v.as_i64()).unwrap_or(ev.ts);
            invalidate_from_event(conn, ws, old, &ev.id, t_invalid, stats).await
        }
        t if t.starts_with("memory.") => {
            // Bookkeeping events (recalled/episode.segmented/decay.swept/
            // tiered/…) are ledger entries, not memories.
            stats.skipped += 1;
            Ok(())
        }
        _ => apply_domain_event(conn, ws, ev, stats).await,
    }
}

async fn apply_domain_event(
    conn: &Connection,
    ws: &str,
    ev: &EventRecord,
    stats: &mut ProjectionStats,
) -> Result<(), MemoryError> {
    let mut flattened = String::new();
    flatten_redacted(&ev.attrs, &mut flattened);
    let entity_title = match &ev.entity_id {
        Some(eid) => entity_title(conn, ws, eid).await?,
        None => None,
    };
    let mut content = format!(
        "[{}] {}",
        ev.actor.as_deref().unwrap_or("unknown"),
        ev.event_type
    );
    if let Some(title) = &entity_title {
        content.push_str(&format!(" ({title})"));
    }
    if !flattened.is_empty() {
        content.push_str(": ");
        content.push_str(&flattened);
    }
    if content.trim().is_empty() {
        stats.skipped += 1;
        return Ok(());
    }

    let id = node_id(ws, &ev.id);
    let importance = write_time_importance(&ev.event_type, &content);
    let sources = serde_json::json!([ev.id]).to_string();
    conn.execute(
        "INSERT OR REPLACE INTO memory_nodes \
           (id, workspace_id, kind, content, importance, access_count, last_accessed, \
            confidence, source_event_ids, ts, t_invalid, superseded_by_event_id) \
         VALUES (?1, ?2, 'episodic', ?3, ?4, 0, NULL, 1.0, ?5, ?6, NULL, NULL)",
        params![id.clone(), ws, content.clone(), importance, sources, ev.ts],
    )
    .await?;
    stats.nodes += 1;
    mirror_to_fts(conn, ws, &id, &content, ev.ts).await;

    if let Some(eid) = &ev.entity_id {
        insert_edge(conn, ws, &ev.id, &id, eid, "about", ev.ts, stats).await?;
    }
    if let Some(cause) = &ev.caused_by_event_id {
        let cause_node = node_id(ws, cause);
        insert_edge(conn, ws, &ev.id, &id, &cause_node, "caused_by", ev.ts, stats).await?;
    }
    // In-band supersede convention: an event that carries
    // attrs.supersedes_event_id invalidates the memory derived from it.
    if let Some(old) = ev.attrs.get("supersedes_event_id").and_then(|v| v.as_str()) {
        let old = old.to_string();
        invalidate_from_event(conn, ws, &old, &ev.id, ev.ts, stats).await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // one column per arg; a struct here is ceremony
async fn insert_edge(
    conn: &Connection,
    ws: &str,
    source_event_id: &str,
    from: &str,
    to: &str,
    rel: &str,
    ts: i64,
    stats: &mut ProjectionStats,
) -> Result<(), MemoryError> {
    let id = edge_id(ws, source_event_id, from, to, rel);
    conn.execute(
        "INSERT OR REPLACE INTO memory_edges \
           (id, workspace_id, from_id, to_id, rel_type, t_valid, t_invalid, \
            superseded_by_event_id, source_event_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, ?7)",
        params![id, ws, from, to, rel, ts, source_event_id],
    )
    .await?;
    stats.edges += 1;
    Ok(())
}

/// Bi-temporal supersede (§7): the node/edges derived from `old_event_id`
/// stop being current truth at `t_invalid` (when the new fact became true),
/// but keep existing for point-in-time queries. Never a DELETE.
async fn invalidate_from_event(
    conn: &Connection,
    ws: &str,
    old_event_id: &str,
    superseding_event_id: &str,
    t_invalid: i64,
    stats: &mut ProjectionStats,
) -> Result<(), MemoryError> {
    if old_event_id.is_empty() {
        stats.skipped += 1;
        return Ok(());
    }
    let old_node = node_id(ws, old_event_id);
    let n = conn
        .execute(
            "UPDATE memory_nodes SET t_invalid = ?1, superseded_by_event_id = ?2 \
             WHERE workspace_id = ?3 AND id = ?4 AND t_invalid IS NULL",
            params![t_invalid, superseding_event_id, ws, old_node.clone()],
        )
        .await?;
    conn.execute(
        "UPDATE memory_edges SET t_invalid = ?1, superseded_by_event_id = ?2 \
         WHERE workspace_id = ?3 AND from_id = ?4 AND t_invalid IS NULL",
        params![t_invalid, superseding_event_id, ws, old_node],
    )
    .await?;
    stats.superseded += n as usize;
    Ok(())
}

async fn apply_summary(
    conn: &Connection,
    ws: &str,
    ev: &EventRecord,
    stats: &mut ProjectionStats,
) -> Result<(), MemoryError> {
    let level = ev.attrs.get("level").and_then(|v| v.as_str()).unwrap_or("episode");
    let content = ev.attrs.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if content.is_empty() {
        stats.skipped += 1;
        return Ok(());
    }
    let confidence = ev.attrs.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let sources = ev
        .attrs
        .get("source_event_ids")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([ev.id]));
    let id = summary_id(ws, &ev.id);
    conn.execute(
        "INSERT OR REPLACE INTO memory_summaries \
           (id, workspace_id, level, content, confidence, source_event_ids, ts) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id.clone(), ws, level, content, confidence, sources.to_string(), ev.ts],
    )
    .await?;
    stats.summaries += 1;

    // A summary is also recallable: project it as a semantic node whose
    // provenance is the summarized events.
    let nid = node_id(ws, &ev.id);
    let node_content = format!("[summary/{level}] {content}");
    conn.execute(
        "INSERT OR REPLACE INTO memory_nodes \
           (id, workspace_id, kind, content, importance, access_count, last_accessed, \
            confidence, source_event_ids, ts, t_invalid, superseded_by_event_id) \
         VALUES (?1, ?2, 'semantic', ?3, 0.6, 0, NULL, ?4, ?5, ?6, NULL, NULL)",
        params![nid.clone(), ws, node_content.clone(), confidence, sources.to_string(), ev.ts],
    )
    .await?;
    stats.nodes += 1;
    mirror_to_fts(conn, ws, &nid, &node_content, ev.ts).await;
    Ok(())
}

async fn apply_rule_added(
    conn: &Connection,
    ws: &str,
    ev: &EventRecord,
    stats: &mut ProjectionStats,
) -> Result<(), MemoryError> {
    let rule = ev.attrs.get("rule").and_then(|v| v.as_str()).unwrap_or("");
    if rule.is_empty() {
        stats.skipped += 1;
        return Ok(());
    }
    let confidence = ev.attrs.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let sources = ev
        .attrs
        .get("source_event_ids")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([ev.id]));
    conn.execute(
        "INSERT OR REPLACE INTO memory_rules \
           (id, workspace_id, rule, confidence, status, source_event_ids, created_ts, retired_ts) \
         VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, NULL)",
        params![rule_id(ws, &ev.id), ws, rule, confidence, sources.to_string(), ev.ts],
    )
    .await?;
    stats.rules += 1;
    Ok(())
}

async fn apply_rule_retired(
    conn: &Connection,
    ws: &str,
    ev: &EventRecord,
    stats: &mut ProjectionStats,
) -> Result<(), MemoryError> {
    let rid = ev.attrs.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");
    let n = conn
        .execute(
            "UPDATE memory_rules SET status = 'retired', retired_ts = ?1 \
             WHERE workspace_id = ?2 AND id = ?3",
            params![ev.ts, ws, rid],
        )
        .await?;
    if n == 0 {
        stats.skipped += 1;
    } else {
        stats.rules += 1;
    }
    Ok(())
}

async fn apply_importance(
    conn: &Connection,
    ws: &str,
    ev: &EventRecord,
    stats: &mut ProjectionStats,
) -> Result<(), MemoryError> {
    let nid = ev.attrs.get("node_id").and_then(|v| v.as_str()).unwrap_or("");
    let importance = ev.attrs.get("importance").and_then(|v| v.as_f64()).unwrap_or(0.3);
    let n = conn
        .execute(
            "UPDATE memory_nodes SET importance = ?1 WHERE workspace_id = ?2 AND id = ?3",
            params![importance.clamp(0.0, 1.0), ws, nid],
        )
        .await?;
    if n == 0 {
        stats.skipped += 1;
    }
    Ok(())
}

async fn entity_title(
    conn: &Connection,
    ws: &str,
    entity_id: &str,
) -> Result<Option<String>, MemoryError> {
    let mut rows = conn
        .query(
            "SELECT title FROM entities WHERE workspace_id = ?1 AND id = ?2",
            params![ws, entity_id],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(row.get(0).ok()),
        None => Ok(None),
    }
}

/// Best-effort mirror into the derived FTS index. The derived DB may not be
/// attached (drain runs without it); the API's boot rebuild reconciles drift.
async fn mirror_to_fts(conn: &Connection, ws: &str, id: &str, content: &str, ts: i64) {
    let _ = conn
        .execute(
            "INSERT INTO d.memory_idx (id, workspace_id, content, ts) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(id) DO UPDATE SET content = excluded.content, ts = excluded.ts",
            params![id, ws, content, ts],
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{seed_entity, seed_event, test_conn};
    use serde_json::json;

    async fn all_rows(conn: &Connection, sql: &str, ws: &str) -> Vec<String> {
        let mut rows = conn.query(sql, params![ws]).await.unwrap();
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            out.push(row.get::<String>(0).unwrap());
        }
        out
    }

    #[tokio::test]
    async fn projects_domain_events_into_nodes_and_edges() {
        let conn = test_conn().await;
        seed_entity(&conn, "ws_1", "ent_task", "tasks", "Ship memory subsystem").await;
        seed_event(
            &conn, "ws_1", "evt_1", 100, "task.completed", Some("ent_task"), "user",
            json!({"note": "finished the projector"}), None,
        )
        .await;
        seed_event(
            &conn, "ws_1", "evt_2", 200, "study.review", None, "bot",
            json!({"topic": "spectral graph theory"}), Some("evt_1"),
        )
        .await;

        let stats = project_workspace(&conn, "ws_1").await.unwrap();
        assert_eq!(stats.nodes, 2);
        assert_eq!(stats.edges, 2, "one 'about' edge + one 'caused_by' edge");

        let contents = all_rows(
            &conn,
            "SELECT content FROM memory_nodes WHERE workspace_id = ?1 ORDER BY ts",
            "ws_1",
        )
        .await;
        assert!(contents[0].contains("task.completed"));
        assert!(contents[0].contains("Ship memory subsystem"), "entity title woven in");
        assert!(contents[0].contains("[user]"), "actor/source recorded");
        assert!(contents[1].contains("[bot]"), "telegram-bot events project the same way");

        // Incremental: re-running with no new events folds nothing twice.
        let again = project_workspace(&conn, "ws_1").await.unwrap();
        assert_eq!(again, ProjectionStats::default());
    }

    #[tokio::test]
    async fn rebuild_is_deterministic() {
        let conn = test_conn().await;
        for i in 0..20 {
            seed_event(
                &conn, "ws_1", &format!("evt_{i:03}"), 100 + i, "note.captured", None, "user",
                json!({"text": format!("note number {i}")}), None,
            )
            .await;
        }
        seed_event(
            &conn, "ws_1", "evt_sup", 500, "memory.supersede.detected", None, "harness",
            json!({"old_event_id": "evt_003"}), None,
        )
        .await;
        project_workspace(&conn, "ws_1").await.unwrap();

        async fn snapshot(conn: &Connection) -> Vec<String> {
            let mut rows = conn
                .query(
                    "SELECT id, content, importance, ts, COALESCE(t_invalid, -1), \
                            COALESCE(superseded_by_event_id, ''), source_event_ids \
                     FROM memory_nodes WHERE workspace_id = 'ws_1' ORDER BY id",
                    (),
                )
                .await
                .unwrap();
            let mut out = Vec::new();
            while let Some(r) = rows.next().await.unwrap() {
                out.push(format!(
                    "{}|{}|{}|{}|{}|{}|{}",
                    r.get::<String>(0).unwrap(),
                    r.get::<String>(1).unwrap(),
                    r.get::<f64>(2).unwrap(),
                    r.get::<i64>(3).unwrap(),
                    r.get::<i64>(4).unwrap(),
                    r.get::<String>(5).unwrap(),
                    r.get::<String>(6).unwrap(),
                ));
            }
            out
        }

        let before = snapshot(&conn).await;
        assert!(!before.is_empty());
        let stats = rebuild_workspace(&conn, "ws_1").await.unwrap();
        assert_eq!(stats.nodes, 20, "20 note events; the supersede event creates no node");
        let after = snapshot(&conn).await;
        assert_eq!(before, after, "wipe + replay must reproduce identical rows");
    }

    #[tokio::test]
    async fn secret_events_and_keys_never_become_memory() {
        let conn = test_conn().await;
        seed_event(
            &conn, "ws_1", "evt_conn", 100, "connection.created", None, "user",
            json!({"provider": "google", "access_token": "ya29.SECRET-TOKEN"}), None,
        )
        .await;
        seed_event(
            &conn, "ws_1", "evt_task", 200, "task.created", None, "user",
            json!({"note": "rotate the keys", "api_key": "sk-HIDDEN"}), None,
        )
        .await;
        project_workspace(&conn, "ws_1").await.unwrap();

        let contents = all_rows(
            &conn,
            "SELECT content FROM memory_nodes WHERE workspace_id = ?1",
            "ws_1",
        )
        .await;
        assert_eq!(contents.len(), 1, "connection.* events skipped entirely");
        assert!(contents[0].contains("rotate the keys"));
        assert!(!contents[0].contains("SECRET"));
        assert!(!contents[0].contains("sk-HIDDEN"));
    }

    #[tokio::test]
    async fn memory_events_materialize_summaries_and_rules() {
        let conn = test_conn().await;
        seed_event(
            &conn, "ws_1", "evt_sum", 100, "memory.summary.created", None, "harness",
            json!({"level": "episode", "content": "studied graphs all morning",
                   "confidence": 0.8, "source_event_ids": ["evt_a", "evt_b"]}),
            None,
        )
        .await;
        seed_event(
            &conn, "ws_1", "evt_rule", 200, "memory.rule.added", None, "harness",
            json!({"rule": "lead with the TLDR", "confidence": 0.7}), None,
        )
        .await;
        let stats = project_workspace(&conn, "ws_1").await.unwrap();
        assert_eq!(stats.summaries, 1);
        assert_eq!(stats.rules, 1);

        let mut rows = conn
            .query(
                "SELECT source_event_ids FROM memory_summaries WHERE workspace_id = 'ws_1'",
                (),
            )
            .await
            .unwrap();
        let sources: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert!(sources.contains("evt_a") && sources.contains("evt_b"), "provenance kept");
    }
}
