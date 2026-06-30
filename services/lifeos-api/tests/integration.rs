//! HTTP-level integration tests. Each test builds the real router against a
//! throwaway libSQL file and drives it with `tower::ServiceExt::oneshot` - no
//! network, no port binding.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use lifeos_api::{build_state, config::Config, ids::new_id, routes};
use serde_json::{json, Value};
use tower::ServiceExt;

struct TestApp {
    router: Router,
    db_path: String,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
        let _ = std::fs::remove_file(format!("{}.derived", self.db_path));
    }
}

async fn test_app() -> TestApp {
    let db_path = std::env::temp_dir()
        .join(format!("lifeos_{}.db", new_id("t")))
        .to_string_lossy()
        .to_string();
    let _ = std::fs::remove_file(&db_path);
    let config = Config {
        db_path: db_path.clone(),
        turso_url: None,
        turso_token: None,
        sync_interval_secs: 60,
        derived_db_path: format!("{db_path}.derived"),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        jwt_secret: "test-secret".into(),
        agent_cwd: None,
        agent_timeout_secs: 30,
    };
    let state = build_state(config).await.expect("build state");
    TestApp {
        router: routes::router(state),
        db_path,
    }
}

/// Send a request, return `(status, parsed-json-or-Null)`.
async fn send(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(b) => builder
            .header("content-type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(request).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn health_is_healthy_and_touches_db() {
    let app = test_app().await;
    let (status, body) = send(&app.router, "GET", "/api/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
    assert!(body["workspace_id"].is_string());
}

#[tokio::test]
async fn register_persists_and_is_idempotent() {
    let app = test_app().await;
    let payload = json!({"email": "x@y.z", "name": "X", "workspace_name": "WS"});

    let (status, body) = send(&app.router, "POST", "/api/register", Some(payload.clone())).await;
    assert_eq!(status, StatusCode::OK);
    let ws = body["workspace_id"].as_str().unwrap().to_string();
    assert!(body["key_token"].as_str().unwrap().len() > 10);
    assert_eq!(body["status"], "registered");

    // The workspace must really exist: creating an entity scoped to it succeeds.
    let (st, _) = send(
        &app.router,
        "POST",
        "/api/entity",
        Some(json!({"workspace_id": ws, "module": "tasks", "type": "task", "title": "t"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    // Re-registering the same email is idempotent (same workspace, no 500).
    let (status2, body2) = send(&app.router, "POST", "/api/register", Some(payload)).await;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(body2["status"], "existing");
    assert_eq!(body2["workspace_id"].as_str().unwrap(), ws);
}

#[tokio::test]
async fn entity_create_list_and_workspace_scoping() {
    let app = test_app().await;
    // Create in the default workspace.
    let (st, ent) = send(
        &app.router,
        "POST",
        "/api/entity",
        Some(json!({"module": "tasks", "type": "task", "title": "Buy milk", "attrs": {"priority": "high"}})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(ent["module"], "tasks");
    assert_eq!(ent["attrs"]["priority"], "high");
    assert!(ent["id"].as_str().unwrap().starts_with("ent_"));

    // Listing the default workspace returns it.
    let (st, list) = send(&app.router, "GET", "/api/entity?module=tasks", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // A different (non-existent) workspace sees nothing - tenant isolation.
    let (st, other) = send(
        &app.router,
        "GET",
        "/api/entity?workspace_id=ws_someone_else",
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(other.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn entity_get_and_update_emits_event() {
    let app = test_app().await;
    let (_, ent) = send(
        &app.router,
        "POST",
        "/api/entity",
        Some(json!({"module": "tasks", "type": "task", "title": "T", "status": "todo"})),
    )
    .await;
    let id = ent["id"].as_str().unwrap();

    // PATCH the status.
    let (st, updated) = send(
        &app.router,
        "PATCH",
        &format!("/api/entity/{id}"),
        Some(json!({"status": "completed"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(updated["status"], "completed");
    assert_eq!(updated["title"], "T"); // untouched field preserved

    // GET reflects the update.
    let (st, fetched) = send(&app.router, "GET", &format!("/api/entity/{id}"), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(fetched["status"], "completed");

    // An entity.updated event was logged.
    let (_, events) = send(&app.router, "GET", "/api/event?type=entity.updated", None).await;
    assert_eq!(events.as_array().unwrap().len(), 1);

    // Missing entity -> 404.
    let (st, _) = send(&app.router, "GET", "/api/entity/ent_missing", None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn entity_list_paginates_with_limit_and_offset() {
    let app = test_app().await;
    for i in 0..3 {
        send(
            &app.router,
            "POST",
            "/api/entity",
            Some(json!({"module": "tasks", "type": "task", "title": format!("t{i}")})),
        )
        .await;
    }
    // First page (limit 2) and second page (offset 2) partition the 3 rows.
    let (_, page1) = send(&app.router, "GET", "/api/entity?module=tasks&limit=2", None).await;
    let (_, page2) = send(&app.router, "GET", "/api/entity?module=tasks&limit=2&offset=2", None).await;
    assert_eq!(page1.as_array().unwrap().len(), 2);
    assert_eq!(page2.as_array().unwrap().len(), 1);
    // No overlap between pages.
    let id_a = page1[0]["id"].as_str().unwrap();
    let id_b = page2[0]["id"].as_str().unwrap();
    assert_ne!(id_a, id_b);
}

#[tokio::test]
async fn edge_state_lifecycle_filter_and_transition() {
    let app = test_app().await;
    let (_, a) = send(&app.router, "POST", "/api/entity", Some(json!({"module": "tasks", "type": "task"}))).await;
    let (_, b) = send(&app.router, "POST", "/api/entity", Some(json!({"module": "tasks", "type": "task"}))).await;
    let (src, dst) = (a["id"].as_str().unwrap(), b["id"].as_str().unwrap());

    // Create a pending edge.
    let (st, edge) = send(
        &app.router,
        "POST",
        "/api/edge",
        Some(json!({"src_id": src, "dst_id": dst, "rel": "depends_on", "state": "pending"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(edge["state"], "pending");
    let edge_id = edge["id"].as_str().unwrap();

    // Filter by state surfaces it under pending, not accepted.
    let (_, pending) = send(&app.router, "GET", "/api/edge?state=pending", None).await;
    assert_eq!(pending.as_array().unwrap().len(), 1);
    let (_, accepted) = send(&app.router, "GET", "/api/edge?state=accepted", None).await;
    assert_eq!(accepted.as_array().unwrap().len(), 0);

    // Transition pending -> accepted.
    let (st, moved) = send(
        &app.router,
        "PATCH",
        &format!("/api/edge/{edge_id}"),
        Some(json!({"state": "accepted"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(moved["state"], "accepted");

    // Now it lists under accepted.
    let (_, accepted2) = send(&app.router, "GET", "/api/edge?state=accepted", None).await;
    assert_eq!(accepted2.as_array().unwrap().len(), 1);

    // Patching a missing edge -> 404.
    let (st, _) = send(&app.router, "PATCH", "/api/edge/edg_missing", Some(json!({"state": "accepted"}))).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn events_are_append_only_no_mutation_routes() {
    let app = test_app().await;
    let (st, _) = send(
        &app.router,
        "POST",
        "/api/event",
        Some(json!({"type": "study.review", "actor": "user"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    // No update/delete route exists -> 405.
    for method in ["PUT", "PATCH", "DELETE"] {
        let (st, _) = send(&app.router, method, "/api/event", None).await;
        assert_eq!(st, StatusCode::METHOD_NOT_ALLOWED, "{method} must be 405");
    }
}

#[tokio::test]
async fn module_request_queues_a_build_job() {
    let app = test_app().await;
    let (st, body) = send(
        &app.router,
        "POST",
        "/api/module-request",
        Some(json!({"prompt": "add a reading module"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["status"], "queued");

    let (_, jobs) = send(&app.router, "GET", "/api/jobs?kind=module_build", None).await;
    assert_eq!(jobs.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn agents_endpoint_lists_detected_agents() {
    let app = test_app().await;
    let (st, body) = send(&app.router, "GET", "/api/agents", None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(body["agents"].is_array());
    // `default` is null only if no agent CLI is installed; either way the key exists.
    assert!(body.get("default").is_some());
}

#[tokio::test]
async fn metrics_aggregates_over_the_workspace() {
    let app = test_app().await;
    send(
        &app.router,
        "POST",
        "/api/entity",
        Some(json!({"module": "tasks", "type": "task", "title": "a"})),
    )
    .await;
    let (st, body) = send(&app.router, "GET", "/api/metrics", None).await;
    assert_eq!(st, StatusCode::OK);
    // One entity created + its entity.created event.
    assert_eq!(body["entities"], 1);
    assert!(body["events"].as_i64().unwrap() >= 1);
    assert_eq!(body["entities_by_module"]["tasks"], 1);
    // Completeness (#6): grouped by type, and events/jobs rollups present.
    assert_eq!(body["entities_by_type"]["task"], 1);
    assert_eq!(body["events_by_type"]["entity.created"], 1);
    assert!(body["jobs_by_status"].is_object());
    // Harness rollup keys exist and are numeric (COALESCE'd, never null).
    assert!(body["tokens_in"].is_number());
    assert!(body["cost"].is_number());
    assert!(body["gated_actions"].is_number());
}

#[tokio::test]
async fn planned_routes_are_honest() {
    let app = test_app().await;
    // ingest enqueues a job -> 202.
    let (st, body) = send(
        &app.router,
        "POST",
        "/api/ingest",
        Some(json!({"uri": "s3://x/a.mp4", "kind": "video"})),
    )
    .await;
    assert_eq!(st, StatusCode::ACCEPTED);
    assert_eq!(body["status"], "queued");

    // vcs history is honestly not implemented -> 501 (not a silent mock).
    let (st, _) = send(&app.router, "GET", "/api/vcs/history", None).await;
    assert_eq!(st, StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn unknown_workspace_is_rejected() {
    let app = test_app().await;
    let (st, body) = send(
        &app.router,
        "POST",
        "/api/entity",
        Some(json!({"workspace_id": "ws_nope", "module": "tasks", "type": "task"})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("workspace"));
}

/// docs/DATA-MODEL.md §4.2: sync is last-push-wins over the whole `attrs`
/// blob, not LWW on `updated_at`. Force a row-level conflict (two divergent
/// writers patching the same entity) and confirm POST .../reconcile repairs
/// the row from the append-only `events` log.
#[tokio::test]
async fn reconcile_replays_events_after_forced_conflict() {
    let app = test_app().await;
    let (_, ent) = send(
        &app.router,
        "POST",
        "/api/entity",
        Some(json!({"module": "tasks", "type": "task", "attrs": {}})),
    )
    .await;
    let id = ent["id"].as_str().unwrap();

    // Two divergent writers (bot lane, Mac lane) each PATCH attrs - both
    // events land in the log, conflict-free.
    let (_, after_bot) = send(
        &app.router,
        "PATCH",
        &format!("/api/entity/{id}"),
        Some(json!({"attrs": {"status": "todo"}})),
    )
    .await;
    assert_eq!(after_bot["attrs"]["status"], "todo");

    let (_, after_mac) = send(
        &app.router,
        "PATCH",
        &format!("/api/entity/{id}"),
        Some(json!({"attrs": {"status": "done"}})),
    )
    .await;
    assert_eq!(after_mac["attrs"]["status"], "done");

    // Force the conflict the way it actually happens: an out-of-band Turso
    // sync pull overwrites the row directly (no API call, no new event) with
    // a stale push - the bot's older write - landing after the Mac's,
    // because last-push-wins cares about sync arrival order, not causal
    // order. Simulate that with a raw connection to the same file, bypassing
    // the handler/event-emit path entirely.
    let raw = libsql::Builder::new_local(&app.db_path).build().await.unwrap();
    let raw_conn = raw.connect().unwrap();
    raw_conn
        .execute(
            "UPDATE entities SET attrs = '{\"status\":\"todo\"}' WHERE id = ?1",
            libsql::params![id],
        )
        .await
        .unwrap();
    let (_, forced) = send(&app.router, "GET", &format!("/api/entity/{id}"), None).await;
    assert_eq!(forced["attrs"]["status"], "todo");

    // Reconcile replays events in causal order; the last attrs-bearing event
    // wins, restoring "done" - the intended final state.
    let (st, reconciled) = send(&app.router, "POST", &format!("/api/entity/{id}/reconcile"), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(reconciled["attrs"]["status"], "done");

    let (_, fetched) = send(&app.router, "GET", &format!("/api/entity/{id}"), None).await;
    assert_eq!(fetched["attrs"]["status"], "done");
}
