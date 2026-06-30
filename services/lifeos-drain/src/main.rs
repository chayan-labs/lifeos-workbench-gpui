//! lifeos-drain: the Mac-side job queue consumer. Polls `jobs`, atomically
//! claims one at a time, dispatches by kind, and reaps crashed claims.
//!
//! Config (env): LIFEOS_DB_PATH (default `lifeos.db`),
//! LIFEOS_DRAIN_POLL_SECS (3), LIFEOS_DRAIN_STUCK_TTL_SECS (300),
//! LIFEOS_DRAIN_MAX_ATTEMPTS (3).

use libsql::Builder;
use lifeos_drain::{claim_job, complete_job, dispatch, fail_job, reap_stuck, Dispatch, DrainConfig};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn env_int(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() {
    let db_path = std::env::var("LIFEOS_DB_PATH").unwrap_or_else(|_| "lifeos.db".to_string());
    let poll = Duration::from_secs(env_int("LIFEOS_DRAIN_POLL_SECS", 3).max(1) as u64);
    let cfg = DrainConfig {
        stuck_ttl_secs: env_int("LIFEOS_DRAIN_STUCK_TTL_SECS", 300),
        max_attempts: env_int("LIFEOS_DRAIN_MAX_ATTEMPTS", 3),
    };

    let db = match Builder::new_local(&db_path).build().await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("lifeos-drain: failed to open {db_path}: {e}");
            std::process::exit(1);
        }
    };
    let conn = db.connect().expect("connect");
    // Wait rather than error on a write lock so two drainers cooperate.
    let _ = conn.execute("PRAGMA busy_timeout = 5000", ()).await;

    let worker_id = format!("mac-drain-{}", now_secs());
    println!("lifeos-drain: worker {worker_id} on {db_path} (poll {poll:?}, {cfg:?})");

    loop {
        match claim_job(&conn, &worker_id, now_secs(), cfg).await {
            Ok(Some(job)) => run_job(&conn, &job).await,
            Ok(None) => {}
            Err(e) => eprintln!("lifeos-drain: claim failed: {e}"),
        }
        match reap_stuck(&conn, now_secs(), cfg).await {
            Ok(n) if n > 0 => println!("lifeos-drain: reaped {n} stuck job(s)"),
            Ok(_) => {}
            Err(e) => eprintln!("lifeos-drain: reaper failed: {e}"),
        }
        sleep(poll).await;
    }
}

async fn run_job(conn: &libsql::Connection, job: &lifeos_drain::ClaimedJob) {
    println!("lifeos-drain: claimed {} (kind={})", job.id, job.kind);
    let result = match dispatch(&job.kind) {
        Dispatch::Stub(handler) => {
            println!("lifeos-drain: {} -> {handler} (stub, no-op this phase)", job.id);
            complete_job(conn, &job.id).await
        }
        Dispatch::Unknown => {
            eprintln!("lifeos-drain: unknown kind '{}' for {} - failing", job.kind, job.id);
            fail_job(conn, &job.id).await
        }
    };
    if let Err(e) = result {
        eprintln!("lifeos-drain: status update for {} failed: {e}", job.id);
    }
}
