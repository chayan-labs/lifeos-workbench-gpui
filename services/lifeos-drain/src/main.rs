//! lifeos-drain: the Mac-side job queue consumer. Polls `jobs`, atomically
//! claims one at a time, dispatches by kind, and reaps crashed claims. Also
//! polls `module_requests` directly for bot-queued self-extension builds
//! (issue #78) - that path doesn't go through `jobs` at all (see
//! `lifeos_drain::claim_next_module_request`'s doc comment for why).
//!
//! Config (env): LIFEOS_DB_PATH (default `lifeos.db`),
//! LIFEOS_DRAIN_POLL_SECS (3), LIFEOS_DRAIN_STUCK_TTL_SECS (300),
//! LIFEOS_DRAIN_MAX_ATTEMPTS (3), LIFEOS_SERVER_DIR (default `server`, the
//! directory `scaffold.js` lives in - set explicitly for a compiled binary
//! whose cwd isn't the repo root, e.g. in the launchd plist),
//! TELEGRAM_BOT_TOKEN (optional - without it, module-build notifications are
//! logged locally instead of sent to Telegram), LIFEOS_VCS_BLOB_ROOT (default
//! `lifeos-blobs`, same env var `lifeos-api` uses for its blob store),
//! LIFEOS_MEMVEC (optional path to `server/memvec.py` - without it, ingest
//! still creates segments, just without semantic embedding),
//! LIFEOS_DERIVED_DB_PATH (default `lifeos-derived.db`, only used when
//! LIFEOS_MEMVEC is set), LIFEOS_WHISPER_MODEL (optional path to a GGML
//! whisper.cpp model, e.g. ggml-tiny.en.bin - without it, audio ingest jobs
//! fail loudly rather than silently producing zero segments, see #89).

use libsql::Builder;
use lifeos_drain::{
    claim_job, claim_next_module_request, complete_job, dispatch, fail_job, reap_stuck,
    run_module_build, Dispatch, DrainConfig, NoopNotifier, Notifier, ScaffoldJsBuilder,
    TelegramNotifier,
};
use lifeos_ingest::{Embedder, IngestJobPayload, NoopEmbedder, NoopTranscriber, SubprocessEmbedder, Transcriber, WhisperTranscriber};
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

    let server_dir = std::env::var("LIFEOS_SERVER_DIR").unwrap_or_else(|_| "server".to_string());
    let builder = ScaffoldJsBuilder { server_dir };
    let notifier: Box<dyn Notifier> = match std::env::var("TELEGRAM_BOT_TOKEN") {
        Ok(token) if !token.is_empty() => Box::new(TelegramNotifier::new(token)),
        _ => {
            println!("lifeos-drain: TELEGRAM_BOT_TOKEN not set, module-build notifications will only be logged");
            Box::new(NoopNotifier)
        }
    };

    let vcs_blob_root = std::env::var("LIFEOS_VCS_BLOB_ROOT").unwrap_or_else(|_| "lifeos-blobs".to_string());
    let vcs_store = lifeos_vcs::ObjectStore::new(vcs_blob_root);
    let embedder: Box<dyn Embedder> = match std::env::var("LIFEOS_MEMVEC") {
        Ok(memvec_path) if !memvec_path.is_empty() => {
            let derived_db_path =
                std::env::var("LIFEOS_DERIVED_DB_PATH").unwrap_or_else(|_| "lifeos-derived.db".to_string());
            Box::new(SubprocessEmbedder { memvec_path, derived_db_path })
        }
        _ => {
            println!("lifeos-drain: LIFEOS_MEMVEC not set, ingested segments won't be semantically embedded");
            Box::new(NoopEmbedder)
        }
    };

    let transcriber: Box<dyn Transcriber> = match std::env::var("LIFEOS_WHISPER_MODEL") {
        Ok(model_path) if !model_path.is_empty() => Box::new(WhisperTranscriber { model_path }),
        _ => {
            println!("lifeos-drain: LIFEOS_WHISPER_MODEL not set, audio ingest jobs will fail loudly");
            Box::new(NoopTranscriber)
        }
    };

    loop {
        match claim_job(&conn, &worker_id, now_secs(), cfg).await {
            Ok(Some(job)) => run_job(&conn, &job, &worker_id, &vcs_store, embedder.as_ref(), transcriber.as_ref()).await,
            Ok(None) => {}
            Err(e) => eprintln!("lifeos-drain: claim failed: {e}"),
        }
        match claim_next_module_request(&conn, now_secs()).await {
            Ok(Some(req)) => {
                println!("lifeos-drain: building module request {} ({})", req.id, req.prompt);
                run_module_build(&conn, &builder, notifier.as_ref(), req, now_secs()).await;
            }
            Ok(None) => {}
            Err(e) => eprintln!("lifeos-drain: module_request claim failed: {e}"),
        }
        match reap_stuck(&conn, now_secs(), cfg).await {
            Ok(n) if n > 0 => println!("lifeos-drain: reaped {n} stuck job(s)"),
            Ok(_) => {}
            Err(e) => eprintln!("lifeos-drain: reaper failed: {e}"),
        }
        sleep(poll).await;
    }
}

async fn run_job(
    conn: &libsql::Connection,
    job: &lifeos_drain::ClaimedJob,
    worker_id: &str,
    vcs_store: &lifeos_vcs::ObjectStore,
    embedder: &dyn Embedder,
    transcriber: &dyn Transcriber,
) {
    println!("lifeos-drain: claimed {} (kind={})", job.id, job.kind);
    let result = match dispatch(&job.kind) {
        Dispatch::Stub(handler) => {
            println!("lifeos-drain: {} -> {handler} (stub, no-op this phase)", job.id);
            complete_job(conn, &job.id, worker_id).await
        }
        Dispatch::Ingest => {
            let payload: IngestJobPayload = serde_json::from_str(&job.payload).unwrap_or_default();
            match lifeos_ingest::process_ingest_job(
                conn,
                vcs_store,
                embedder,
                transcriber,
                &job.workspace_id,
                payload,
                now_secs(),
            )
            .await
            {
                Ok(outcome) => {
                    println!("lifeos-drain: {} ingest -> {outcome:?}", job.id);
                    complete_job(conn, &job.id, worker_id).await
                }
                Err(e) => {
                    eprintln!("lifeos-drain: {} ingest failed: {e} - failing", job.id);
                    fail_job(conn, &job.id, worker_id).await
                }
            }
        }
        Dispatch::Unknown => {
            eprintln!("lifeos-drain: unknown kind '{}' for {} - failing", job.kind, job.id);
            fail_job(conn, &job.id, worker_id).await
        }
    };
    match result {
        // 0 rows = this worker no longer holds the lease (reaped + re-claimed
        // while it was working). Don't overwrite the new owner's claim.
        Ok(0) => eprintln!(
            "lifeos-drain: lease lost for {} (reaped + re-claimed); skipping status write",
            job.id
        ),
        Ok(_) => {}
        Err(e) => eprintln!("lifeos-drain: status update for {} failed: {e}", job.id),
    }
}
