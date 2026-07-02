//! Benchmark fixtures (issue #120, docs/AI-MEMORY.md §11) - scaled-down,
//! CI-runnable analogues of the public sets:
//! - LongMemEval axes: temporal reasoning, knowledge updates, abstention;
//! - LoCoMo axis: multi-hop (relational) recall;
//! - BEAM axis: ranking quality as history grows (a scaling smoke test - the
//!   real 1M/10M runs are an offline exercise, not a unit test).
//!
//! Each fixture compares the activation re-ranker against a FLAT TOP-K
//! baseline (relevance only - no recency, importance, frequency, graph, or
//! bi-temporal filtering) and must beat or match it. That's the issue-#113
//! acceptance bar made executable.

use lifeos_memory::project::project_workspace;
use lifeos_memory::{recall, NoopVectorSearcher, RecallOutcome, RecallParams};
use libsql::{params, Builder, Connection};
use serde_json::json;

const MIGRATION_MEMORY: &str = include_str!("../../../migrations/0017_memory.sql");
const HOUR: i64 = 3600;

async fn fresh_db() -> Connection {
    let dir = std::env::temp_dir().join(format!("lifeos-mem-bench-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Builder::new_local(dir.join("bench.db").to_str().unwrap()).build().await.unwrap();
    let conn = db.connect().unwrap();
    conn.execute_batch(
        "CREATE TABLE workspaces (id TEXT PRIMARY KEY, name TEXT, created_at INTEGER, updated_at INTEGER);
         CREATE TABLE entities (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, module TEXT NOT NULL,
            type TEXT NOT NULL, title TEXT, status TEXT, attrs TEXT DEFAULT '{}',
            created_at INTEGER, updated_at INTEGER);
         CREATE TABLE events (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, ts INTEGER NOT NULL,
            type TEXT NOT NULL, entity_id TEXT, actor TEXT, attrs TEXT DEFAULT '{}',
            caused_by_event_id TEXT, schema_version INTEGER NOT NULL DEFAULT 1);
         INSERT INTO workspaces VALUES ('ws_1', 'bench', 1, 1);",
    )
    .await
    .unwrap();
    conn.execute_batch(MIGRATION_MEMORY).await.unwrap();
    conn
}

async fn event(conn: &Connection, id: &str, ts: i64, event_type: &str, entity: Option<&str>, attrs: serde_json::Value) {
    conn.execute(
        "INSERT INTO events (id, workspace_id, ts, type, entity_id, actor, attrs) \
         VALUES (?1, 'ws_1', ?2, ?3, ?4, 'user', ?5)",
        params![id, ts, event_type, entity, attrs.to_string()],
    )
    .await
    .unwrap();
}

/// The flat top-k baseline: pure lexical token-match count, no activation.
async fn flat_topk(conn: &Connection, query: &str, k: usize) -> Vec<String> {
    let tokens: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 3)
        .map(|t| t.to_lowercase())
        .collect();
    let mut scores: std::collections::HashMap<String, usize> = Default::default();
    for t in tokens {
        let mut rows = conn
            .query(
                "SELECT id FROM memory_nodes WHERE workspace_id = 'ws_1' \
                 AND content LIKE ?1 COLLATE NOCASE",
                params![format!("%{t}%")],
            )
            .await
            .unwrap();
        while let Some(r) = rows.next().await.unwrap() {
            *scores.entry(r.get::<String>(0).unwrap()).or_insert(0) += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.into_iter().take(k).map(|(id, _)| id).collect()
}

fn recalled_ids(out: &RecallOutcome) -> Vec<String> {
    match out {
        RecallOutcome::Recalled { memories, .. } => memories.iter().map(|m| m.id.clone()).collect(),
        _ => Vec::new(),
    }
}

/// LongMemEval "knowledge update" axis: after an update, only the new value
/// is current truth. Flat top-k returns both (or the wrong one); activation
/// recall with bi-temporal filtering returns only Bangalore.
#[tokio::test]
async fn knowledge_update_beats_flat_topk() {
    let conn = fresh_db().await;
    event(&conn, "evt_delhi", 0, "profile.updated", None, json!({"home city": "Delhi"})).await;
    event(
        &conn, "evt_blr", 100 * HOUR, "profile.updated", None,
        json!({"home city": "Bangalore", "supersedes_event_id": "evt_delhi"}),
    )
    .await;
    project_workspace(&conn, "ws_1").await.unwrap();

    let now = 101 * HOUR;
    let query = "what is my home city?";
    let flat = flat_topk(&conn, query, 8).await;
    assert_eq!(flat.len(), 2, "the naive baseline can't tell old from current");

    let out = recall(&conn, "ws_1", query, now, &RecallParams::default(), &NoopVectorSearcher)
        .await
        .unwrap();
    let RecallOutcome::Recalled { memories, .. } = &out else { panic!("{out:?}") };
    assert_eq!(memories.len(), 1, "superseded fact must not be current: {memories:#?}");
    assert!(memories[0].content.contains("Bangalore"));

    // Temporal reasoning: point-in-time recall still recovers Delhi.
    let then = recall(
        &conn, "ws_1", query, now,
        &RecallParams { as_of: Some(50 * HOUR), ..Default::default() },
        &NoopVectorSearcher,
    )
    .await
    .unwrap();
    let RecallOutcome::Recalled { memories, .. } = &then else { panic!("{then:?}") };
    assert_eq!(memories.len(), 1);
    assert!(memories[0].content.contains("Delhi"), "what was true in March stays answerable");
}

/// LongMemEval "temporal reasoning" axis: many equally-relevant memories; the
/// re-ranker must order by recency where the baseline is arbitrary.
#[tokio::test]
async fn temporal_ranking_orders_recent_first() {
    let conn = fresh_db().await;
    for i in 0..10 {
        event(
            &conn, &format!("evt_standup_{i:02}"), i * 24 * HOUR, "note.captured", None,
            json!({"text": "daily standup notes about the ingest pipeline"}),
        )
        .await;
    }
    project_workspace(&conn, "ws_1").await.unwrap();

    let now = 10 * 24 * HOUR;
    let out = recall(
        &conn, "ws_1", "standup notes ingest pipeline", now,
        &RecallParams { abstention_threshold: 0.0, ..Default::default() },
        &NoopVectorSearcher,
    )
    .await
    .unwrap();
    let RecallOutcome::Recalled { memories, .. } = out else { panic!() };
    let ts: Vec<i64> = memories.iter().map(|m| m.ts).collect();
    let mut sorted = ts.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(ts, sorted, "identical relevance => strictly recency-ordered");
    assert_eq!(memories[0].ts, 9 * 24 * HOUR);
}

/// LoCoMo multi-hop axis: the answer shares no token with the query and is
/// only reachable through the entity graph. Flat top-k scores 0; spreading
/// activation finds it.
#[tokio::test]
async fn multi_hop_beats_flat_topk() {
    let conn = fresh_db().await;
    conn.execute(
        "INSERT INTO entities (id, workspace_id, module, type, title, created_at, updated_at) \
         VALUES ('ent_topic', 'ws_1', 'learning', 'topic', 'Market microstructure', 1, 1)",
        (),
    )
    .await
    .unwrap();
    event(
        &conn, "evt_dd", 1000, "trade.closed", Some("ent_topic"),
        json!({"note": "drawdown on the banknifty position"}),
    )
    .await;
    event(
        &conn, "evt_study", 2000, "learning.review", Some("ent_topic"),
        json!({"summary": "reviewed liquidity sweeps and stop hunts"}),
    )
    .await;
    project_workspace(&conn, "ws_1").await.unwrap();

    let query = "this drawdown relates to which subject?";
    let study_node = lifeos_memory::project::node_id("ws_1", "evt_study");

    let flat = flat_topk(&conn, query, 8).await;
    assert!(!flat.contains(&study_node), "baseline can't hop the graph");

    let out = recall(
        &conn, "ws_1", query, 3000,
        &RecallParams { abstention_threshold: 0.0, ..Default::default() },
        &NoopVectorSearcher,
    )
    .await
    .unwrap();
    assert!(recalled_ids(&out).contains(&study_node), "graph expansion recovers the link");
}

/// LongMemEval abstention axis + BEAM-style scaling smoke: even with a large
/// noisy history, an unknown fact abstains instead of confabulating, and a
/// known needle still surfaces at rank 1.
#[tokio::test]
async fn abstention_and_needle_survive_a_large_history() {
    let conn = fresh_db().await;
    for i in 0..500 {
        event(
            &conn, &format!("evt_noise_{i:04}"), i * HOUR, "note.captured", None,
            json!({"text": format!("routine journal entry number {i} about lunch and errands")}),
        )
        .await;
    }
    event(
        &conn, "evt_needle", 250 * HOUR, "decision.recorded", None,
        json!({"text": "chose libsql embedded replica over postgres for the canonical store"}),
    )
    .await;
    project_workspace(&conn, "ws_1").await.unwrap();

    let now = 501 * HOUR;
    let found = recall(
        &conn, "ws_1", "why did we choose libsql over postgres?", now,
        &RecallParams::default(), &NoopVectorSearcher,
    )
    .await
    .unwrap();
    let RecallOutcome::Recalled { memories, .. } = &found else { panic!("{found:?}") };
    assert!(memories[0].content.contains("embedded replica"), "needle at rank 1");

    let unknown = recall(
        &conn, "ws_1", "what is my kubernetes cluster admin passphrase hint?", now,
        &RecallParams::default(), &NoopVectorSearcher,
    )
    .await
    .unwrap();
    assert!(
        !matches!(unknown, RecallOutcome::Recalled { .. })
            || recalled_ids(&unknown).is_empty(),
        "genuinely-unknown fact must abstain: {unknown:?}"
    );
}
