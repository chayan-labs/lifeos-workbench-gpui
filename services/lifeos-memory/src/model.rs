//! MemoryModel - the one seam through which consolidation talks to an LLM
//! (docs/AI-MEMORY.md §2/§5), plus the content-addressed replay cache that
//! makes event replay deterministic despite LLM non-determinism (issue #112).
//!
//! Every consolidation call MUST go through `ReplayCachedModel`: the first
//! run hits the inner model and stores the response under BLAKE3(task||prompt)
//! in `llm_replay_cache` (canonical DB - surviving a derived-store wipe is the
//! whole point); every replay serves the cached bytes, so rebuilding memory
//! from `events` reproduces byte-identical summaries.

use crate::error::MemoryError;
use async_trait::async_trait;
use libsql::{params, Connection};

#[async_trait]
pub trait MemoryModel: Send + Sync {
    /// `task` names the call site ('summarize', 'reflect', 'reformulate', …);
    /// it is part of the cache key and stored for debuggability.
    async fn complete(&self, task: &str, prompt: &str) -> Result<String, MemoryError>;
}

/// Deterministic, LLM-free fallback: extractive summarization (first clause
/// of each line, bounded). Consolidation stays functional - and testable in
/// CI - with zero API keys; a Haiku-backed impl slots in via the same trait.
pub struct HeuristicModel;

const HEURISTIC_LINE_CHARS: usize = 120;
const HEURISTIC_MAX_LINES: usize = 8;

#[async_trait]
impl MemoryModel for HeuristicModel {
    async fn complete(&self, _task: &str, prompt: &str) -> Result<String, MemoryError> {
        let lines: Vec<String> = prompt
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .take(HEURISTIC_MAX_LINES)
            .map(|l| {
                let truncated: String = l.chars().take(HEURISTIC_LINE_CHARS).collect();
                format!("- {truncated}")
            })
            .collect();
        Ok(lines.join("\n"))
    }
}

/// BLAKE3 content-addressed wrapper. Cache key = blake3(task || 0x00 || prompt).
pub struct ReplayCachedModel<'a> {
    inner: &'a dyn MemoryModel,
    conn: &'a Connection,
    now: i64,
}

impl<'a> ReplayCachedModel<'a> {
    pub fn new(inner: &'a dyn MemoryModel, conn: &'a Connection, now: i64) -> Self {
        Self { inner, conn, now }
    }
}

pub fn request_hash(task: &str, prompt: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(task.as_bytes());
    hasher.update(&[0]);
    hasher.update(prompt.as_bytes());
    hasher.finalize().to_hex().to_string()
}

#[async_trait]
impl MemoryModel for ReplayCachedModel<'_> {
    async fn complete(&self, task: &str, prompt: &str) -> Result<String, MemoryError> {
        let hash = request_hash(task, prompt);
        let mut rows = self
            .conn
            .query(
                "SELECT response FROM llm_replay_cache WHERE request_hash = ?1",
                params![hash.clone()],
            )
            .await?;
        if let Some(row) = rows.next().await? {
            return Ok(row.get(0)?);
        }
        let response = self.inner.complete(task, prompt).await?;
        // INSERT OR IGNORE: a concurrent caller winning the race is fine -
        // both computed from the same request, and first-write-wins keeps
        // replay stable from then on.
        self.conn
            .execute(
                "INSERT OR IGNORE INTO llm_replay_cache (request_hash, task, response, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![hash, task, response.clone(), self.now],
            )
            .await?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_conn;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A "non-deterministic" model: returns a different response every call.
    struct FlakyModel {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl MemoryModel for FlakyModel {
        async fn complete(&self, _t: &str, _p: &str) -> Result<String, MemoryError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(format!("response-{n}"))
        }
    }

    #[tokio::test]
    async fn replay_cache_makes_a_flaky_model_deterministic() {
        let conn = test_conn().await;
        let flaky = FlakyModel { calls: AtomicUsize::new(0) };
        let cached = ReplayCachedModel::new(&flaky, &conn, 1000);

        let first = cached.complete("summarize", "same prompt").await.unwrap();
        let second = cached.complete("summarize", "same prompt").await.unwrap();
        assert_eq!(first, second, "replay must serve the cached response");
        assert_eq!(flaky.calls.load(Ordering::SeqCst), 1, "inner model called once");

        // Different task => different cache key even with the same prompt.
        let other = cached.complete("reflect", "same prompt").await.unwrap();
        assert_ne!(first, other);
    }

    #[tokio::test]
    async fn heuristic_model_is_deterministic_and_bounded() {
        let m = HeuristicModel;
        let long_line = "x".repeat(500);
        let prompt = format!("first fact\n\n{long_line}\nthird");
        let a = m.complete("summarize", &prompt).await.unwrap();
        let b = m.complete("summarize", &prompt).await.unwrap();
        assert_eq!(a, b);
        assert!(a.lines().count() <= HEURISTIC_MAX_LINES);
        for line in a.lines() {
            assert!(line.chars().count() <= HEURISTIC_LINE_CHARS + 2);
        }
    }
}
