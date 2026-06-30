//! Acceptance tests for the drain queue: no double-claim under concurrency,
//! and crashed claims get reaped and retried (then failed past the cap).

use libsql::{params, Builder, Connection, Database};
use lifeos_drain::{claim_job, reap_stuck, DrainConfig};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

async fn temp_db(tag: &str) -> Database {
    let path =
        std::env::temp_dir().join(format!("lifeos-drain-{tag}-{}-{}.db", std::process::id(), now()));
    let _ = std::fs::remove_file(&path);
    let db = Builder::new_local(path).build().await.unwrap();
    let conn = db.connect().unwrap();
    conn.execute_batch(
        "CREATE TABLE jobs (\
            id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, kind TEXT NOT NULL, \
            payload TEXT NOT NULL DEFAULT '{}', status TEXT NOT NULL DEFAULT 'queued', \
            priority INTEGER NOT NULL DEFAULT 0, run_after INTEGER, claimed_by TEXT, \
            claimed_at INTEGER, attempts INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL);",
    )
    .await
    .unwrap();
    db
}

async fn seed_queued(conn: &Connection, n: usize) {
    for i in 0..n {
        conn.execute(
            "INSERT INTO jobs (id, workspace_id, kind, created_at) VALUES (?1, 'ws', 'ingest', ?2)",
            params![format!("job_{i}"), now()],
        )
        .await
        .unwrap();
    }
}

async fn drain_all(conn: &Connection, worker: &str, cfg: DrainConfig) -> Vec<String> {
    let mut claimed = Vec::new();
    while let Some(job) = claim_job(conn, worker, now(), cfg).await.unwrap() {
        claimed.push(job.id);
    }
    claimed
}

#[tokio::test]
async fn two_drainers_never_double_claim() {
    let cfg = DrainConfig::default();
    let db = temp_db("concurrency").await;
    let setup = db.connect().unwrap();
    let _ = setup.execute("PRAGMA busy_timeout = 5000", ()).await;
    seed_queued(&setup, 50).await;

    let a = db.connect().unwrap();
    let b = db.connect().unwrap();
    let _ = a.execute("PRAGMA busy_timeout = 5000", ()).await;
    let _ = b.execute("PRAGMA busy_timeout = 5000", ()).await;

    let (ra, rb) = tokio::join!(
        async move { drain_all(&a, "worker-a", cfg).await },
        async move { drain_all(&b, "worker-b", cfg).await },
    );

    let mut all = ra.clone();
    all.extend(rb.clone());
    let unique: HashSet<_> = all.iter().cloned().collect();
    assert_eq!(all.len(), 50, "every queued job claimed exactly once");
    assert_eq!(unique.len(), 50, "no job claimed by both drainers");
}

#[tokio::test]
async fn reaper_requeues_then_fails_past_cap() {
    let cfg = DrainConfig { stuck_ttl_secs: 300, max_attempts: 3 };
    let db = temp_db("reaper").await;
    let conn = db.connect().unwrap();
    let stale = now() - 1000;

    // Stuck running job with retries left -> requeued.
    conn.execute(
        "INSERT INTO jobs (id, workspace_id, kind, status, claimed_by, claimed_at, attempts, created_at) \
         VALUES ('retry', 'ws', 'ingest', 'running', 'dead-worker', ?1, 1, ?1)",
        params![stale],
    ).await.unwrap();
    // Stuck running job that exhausted retries -> failed.
    conn.execute(
        "INSERT INTO jobs (id, workspace_id, kind, status, claimed_by, claimed_at, attempts, created_at) \
         VALUES ('dead', 'ws', 'ingest', 'running', 'dead-worker', ?1, 3, ?1)",
        params![stale],
    ).await.unwrap();
    // Fresh running job -> untouched.
    conn.execute(
        "INSERT INTO jobs (id, workspace_id, kind, status, claimed_by, claimed_at, attempts, created_at) \
         VALUES ('fresh', 'ws', 'ingest', 'running', 'live-worker', ?1, 1, ?1)",
        params![now()],
    ).await.unwrap();

    let reaped = reap_stuck(&conn, now(), cfg).await.unwrap();
    assert_eq!(reaped, 2, "two stale jobs reaped, fresh one left alone");

    assert_eq!(status(&conn, "retry").await, "queued");
    assert_eq!(status(&conn, "dead").await, "failed");
    assert_eq!(status(&conn, "fresh").await, "running");

    // The requeued job is claimable again (a killed mid-run job is retried).
    let claimed = claim_job(&conn, "worker-b", now(), cfg).await.unwrap();
    assert_eq!(claimed.unwrap().id, "retry");
}

async fn status(conn: &Connection, id: &str) -> String {
    let mut rows = conn
        .query("SELECT status FROM jobs WHERE id=?1", params![id])
        .await
        .unwrap();
    rows.next().await.unwrap().unwrap().get(0).unwrap()
}
