//! lifeos-drain queue core: atomic claim, reaper, and dispatch-by-kind.
//!
//! The Mac drains heavy work enqueued to `jobs`. Claims must be atomic so two
//! drainers never run the same job; crashed claims must be reaped and retried.
//! These functions are split out of `main` so the concurrency and reaper
//! guarantees can be tested directly against a libSQL connection.

use libsql::{params, Connection};

/// A job a drainer has exclusively claimed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedJob {
    pub id: String,
    pub kind: String,
    pub payload: String,
    pub workspace_id: String,
}

/// Tunables, all overridable via env in `main`.
#[derive(Debug, Clone, Copy)]
pub struct DrainConfig {
    /// Seconds a `running` job may go untouched before it is reaped.
    pub stuck_ttl_secs: i64,
    /// Max claim attempts before a job is marked `failed` for good.
    pub max_attempts: i64,
}

impl Default for DrainConfig {
    fn default() -> Self {
        Self {
            stuck_ttl_secs: 300,
            max_attempts: 3,
        }
    }
}

/// Atomically claim the highest-priority eligible job, if any.
///
/// SQLite serializes writers, so the `UPDATE ... WHERE id = (SELECT ...
/// status='queued' ...)` re-checks `status` under the write lock - two drainers
/// racing this statement can never select-and-claim the same row. We bump
/// `attempts` here so the count survives a crash (the reaper requeues without
/// re-incrementing).
pub async fn claim_job(
    conn: &Connection,
    worker_id: &str,
    now: i64,
    cfg: DrainConfig,
) -> libsql::Result<Option<ClaimedJob>> {
    let sql = "UPDATE jobs \
         SET status='running', claimed_by=?1, claimed_at=?2, attempts=attempts+1 \
         WHERE id = ( \
            SELECT id FROM jobs \
            WHERE status='queued' \
              AND (run_after IS NULL OR run_after <= ?2) \
              AND attempts < ?3 \
            ORDER BY priority DESC, created_at ASC LIMIT 1 \
         ) \
         RETURNING id, kind, payload, workspace_id";
    let mut rows = conn
        .query(sql, params![worker_id, now, cfg.max_attempts])
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(ClaimedJob {
            id: row.get(0)?,
            kind: row.get(1)?,
            payload: row.get(2)?,
            workspace_id: row.get(3)?,
        })),
        None => Ok(None),
    }
}

/// Mark a claimed job done.
pub async fn complete_job(conn: &Connection, id: &str) -> libsql::Result<()> {
    conn.execute("UPDATE jobs SET status='done' WHERE id=?1", params![id])
        .await?;
    Ok(())
}

/// Mark a claimed job failed (no further retries).
pub async fn fail_job(conn: &Connection, id: &str) -> libsql::Result<()> {
    conn.execute("UPDATE jobs SET status='failed' WHERE id=?1", params![id])
        .await?;
    Ok(())
}

/// Reap jobs stuck in `running` past the TTL. Those under the attempt cap go
/// back to `queued`; those that have exhausted their retries become `failed`.
/// Returns the number of rows reaped.
pub async fn reap_stuck(conn: &Connection, now: i64, cfg: DrainConfig) -> libsql::Result<u64> {
    let threshold = now - cfg.stuck_ttl_secs;
    let n = conn
        .execute(
            "UPDATE jobs \
             SET status = CASE WHEN attempts >= ?1 THEN 'failed' ELSE 'queued' END, \
                 claimed_by = NULL, claimed_at = NULL \
             WHERE status='running' AND claimed_at IS NOT NULL AND claimed_at < ?2",
            params![cfg.max_attempts, threshold],
        )
        .await?;
    Ok(n)
}

/// Result of dispatching a claimed job to its (eventual) handler.
#[derive(Debug, PartialEq, Eq)]
pub enum Dispatch {
    /// A known kind whose real handler lands in a later phase (no-op stub).
    Stub(&'static str),
    /// Unknown kind - cannot be handled, will be failed.
    Unknown,
}

/// Route a job to its handler by kind. Real handlers (ingest/pipeline/
/// module_build/eval) land in later phases; until then known kinds are
/// acknowledged as no-op stubs and unknown kinds are rejected.
///
/// `reconcile` (docs/DATA-MODEL.md §4.2) already has a real handler -
/// `lifeos_api::reconcile::reconcile_entity`, reachable today via
/// `POST /api/entity/:id/reconcile`. It is dispatched here as a stub too so a
/// queued `jobs` row of this kind is acknowledged rather than rejected as
/// Unknown; wiring drain to actually call the API is a later phase, same as
/// the other stub kinds.
pub fn dispatch(kind: &str) -> Dispatch {
    match kind {
        "ingest" => Dispatch::Stub("lifeos-ingest"),
        "pipeline" => Dispatch::Stub("lifeos-pipelines"),
        "module_build" => Dispatch::Stub("scaffold.js"),
        "eval" => Dispatch::Stub("harness eval"),
        "reconcile" => Dispatch::Stub("lifeos-api reconcile"),
        _ => Dispatch::Unknown,
    }
}
