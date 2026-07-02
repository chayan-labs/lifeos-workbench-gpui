//! `/api/storage/backends` HTTP-level tests (issue #107,
//! docs/STORAGE-BACKENDS.md §3-§4): configs draft as `pending_approval`
//! (gated - adding/switching a backend moves user data), key material is
//! envelope-encrypted into `connections.secret_enc` and never appears in
//! entity attrs, and unknown kinds are rejected.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use lifeos_api::{build_state_with_nango, config::Config, crypto, ids::new_id, routes};
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

fn base_config(db_path: &str) -> Config {
    Config {
        db_path: db_path.to_string(),
        turso_url: None,
        turso_token: None,
        sync_interval_secs: 60,
        derived_db_path: format!("{db_path}.derived"),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        jwt_secret: "test-secret".into(),
        agent_cwd: None,
        agent_timeout_secs: 30,
        nango_server_url: None,
        nango_secret_key: None,
        kite_api_key: None,
        kite_api_secret: None,
        secret_encryption_key: Some(
            crypto::parse_key("MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=").expect("test key"),
        ),
        gowa_base_url: None,
        gowa_basic_auth: None,
        gowa_webhook_secret: None,
        browser_script_path: None,
        vcs_blob_root: format!("{db_path}.blobs"),
        marketplace_signing_key: None,
        turso_platform_api_token: None,
        turso_org_slug: None,
    }
}

async fn test_app() -> TestApp {
    let db_path = std::env::temp_dir()
        .join(format!("lifeos_storage_{}.db", new_id("t")))
        .to_string_lossy()
        .to_string();
    let _ = std::fs::remove_file(&db_path);
    let state = build_state_with_nango(base_config(&db_path), None).await.expect("build state");
    TestApp { router: routes::router(state), db_path }
}

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

async fn register(app: &Router, name: &str) -> String {
    let (st, body) = send(
        app,
        "POST",
        "/api/register",
        Some(json!({
            "email": format!("{name}@test.example"),
            "name": name,
            "password": "test-password-123",
            "workspace_name": name
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "register {name}: {body:?}");
    body["workspace_id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn creating_a_backend_only_drafts_it_pending_approval() {
    let ta = test_app().await;
    let ws = register(&ta.router, "storage-gated").await;

    let (st, body) = send(
        &ta.router,
        "POST",
        "/api/storage/backends",
        Some(json!({
            "kind": "s3",
            "folder": "lifeos",
            "default": true,
            "keys": {"bucket": "my-bucket", "access_key_id": "AKIA", "secret_access_key": "sekrit"},
            "workspace_id": ws
        })),
    )
    .await;

    assert_eq!(st, StatusCode::OK, "{body:?}");
    assert_eq!(body["status"], "pending_approval");
    assert_eq!(body["module"], "storage");
    assert_eq!(body["type"], "storage_backend");
}

#[tokio::test]
async fn key_material_never_lands_in_entity_attrs() {
    let ta = test_app().await;
    let ws = register(&ta.router, "storage-secrets").await;

    let (st, body) = send(
        &ta.router,
        "POST",
        "/api/storage/backends",
        Some(json!({
            "kind": "webdav",
            "keys": {"base_url": "https://dav.example.com", "username": "chayan", "password": "hunter2"},
            "workspace_id": ws
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body:?}");

    let attrs_str = serde_json::to_string(&body["attrs"]).unwrap();
    assert!(!attrs_str.contains("hunter2"), "secret leaked into attrs: {attrs_str}");
    assert!(!attrs_str.contains("dav.example.com"), "keys blob leaked into attrs: {attrs_str}");
    assert!(body["attrs"]["connection_id"].as_str().is_some(), "attrs should reference the connection: {body:?}");

    // The listing surface leaks nothing either.
    let (st, listed) = send(&ta.router, "GET", &format!("/api/storage/backends?workspace_id={ws}"), None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(!listed.to_string().contains("hunter2"));
}

#[tokio::test]
async fn unknown_kind_is_rejected() {
    let ta = test_app().await;
    let ws = register(&ta.router, "storage-badkind").await;

    let (st, _) = send(
        &ta.router,
        "POST",
        "/api/storage/backends",
        Some(json!({"kind": "carrier-pigeon", "workspace_id": ws})),
    )
    .await;

    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn non_local_backend_without_credentials_is_rejected() {
    let ta = test_app().await;
    let ws = register(&ta.router, "storage-nocreds").await;

    let (st, _) = send(
        &ta.router,
        "POST",
        "/api/storage/backends",
        Some(json!({"kind": "google-drive", "workspace_id": ws})),
    )
    .await;

    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_is_workspace_scoped() {
    let ta = test_app().await;
    let ws_a = register(&ta.router, "storage-ws-a").await;
    let ws_b = register(&ta.router, "storage-ws-b").await;

    let (st, _) = send(
        &ta.router,
        "POST",
        "/api/storage/backends",
        Some(json!({"kind": "local-fs", "workspace_id": ws_a})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    let (_, listed_a) = send(&ta.router, "GET", &format!("/api/storage/backends?workspace_id={ws_a}"), None).await;
    let (_, listed_b) = send(&ta.router, "GET", &format!("/api/storage/backends?workspace_id={ws_b}"), None).await;
    assert_eq!(listed_a.as_array().unwrap().len(), 1);
    assert_eq!(listed_b.as_array().unwrap().len(), 0);
}
