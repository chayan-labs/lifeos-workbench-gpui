//! Issue #2 spike: prove the Workbench reads/writes entities through
//! `lifeos-api` linked in-process - no socket, no HTTP client, and with the
//! same auth + workspace scoping the HTTP surface enforces.

use lifeos_api::config::Config;
use lifeos_workbench::api::InProcessApi;
use serde_json::json;

fn test_config(db_path: &str) -> Config {
    Config {
        db_path: db_path.to_string(),
        turso_url: None,
        turso_token: None,
        sync_interval_secs: 60,
        derived_db_path: format!("{db_path}.derived"),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        jwt_secret: "workbench-test-secret".into(),
        agent_cwd: None,
        agent_timeout_secs: 30,
        nango_server_url: None,
        nango_secret_key: None,
        kite_api_key: None,
        kite_api_secret: None,
        secret_encryption_key: None,
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

struct TempDb(String);
impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(format!("{}.derived", self.0));
        let _ = std::fs::remove_dir_all(format!("{}.blobs", self.0));
    }
}

fn temp_db() -> TempDb {
    TempDb(
        std::env::temp_dir()
            .join(format!("workbench_spike_{}.db", ulid::Ulid::new()))
            .to_string_lossy()
            .to_string(),
    )
}

async fn register(api: &InProcessApi, name: &str) -> (String, String) {
    let reg = api
        .post(
            "/api/register",
            json!({
                "email": format!("{name}@test.example"),
                "name": name,
                "password": "test-password-123",
                "workspace_name": name
            }),
            None,
        )
        .await;
    assert!(reg.is_success(), "register failed: {:?}", reg.body);
    (
        reg.body["workspace_id"]
            .as_str()
            .expect("workspace_id")
            .to_string(),
        reg.body["key_token"]
            .as_str()
            .expect("key_token")
            .to_string(),
    )
}

#[tokio::test]
async fn reads_and_writes_entities_in_process_with_no_socket() {
    let db = temp_db();
    let api = InProcessApi::new(test_config(&db.0))
        .await
        .expect("in-process state");
    let (ws, token) = register(&api, "spike").await;

    // Write an entity through the same handlers the HTTP server uses.
    let created = api
        .post(
            "/api/entity",
            json!({
                "module": "notes",
                "type": "note",
                "title": "in-process spike",
                "attrs": {"body": "written with no socket"}
            }),
            Some(&token),
        )
        .await;
    assert!(
        created.is_success(),
        "create entity failed: {:?}",
        created.body
    );
    let id = created.body["id"].as_str().expect("entity id").to_string();
    assert_eq!(
        created.body["workspace_id"],
        json!(ws),
        "row must be workspace-scoped"
    );

    // Read it back through the same scoped list handler.
    let listed = api.get("/api/entity?type=note", Some(&token)).await;
    assert!(
        listed.is_success(),
        "list entities failed: {:?}",
        listed.body
    );
    let found = listed
        .body
        .as_array()
        .map(|items| items.iter().any(|e| e["id"] == json!(id)))
        .or_else(|| {
            listed.body["items"]
                .as_array()
                .map(|items| items.iter().any(|e| e["id"] == json!(id)))
        })
        .unwrap_or(false);
    assert!(found, "created entity not found in list: {:?}", listed.body);
}

#[tokio::test]
async fn token_workspace_wins_over_body_workspace() {
    let db = temp_db();
    let api = InProcessApi::new(test_config(&db.0))
        .await
        .expect("in-process state");
    let (ws_a, token_a) = register(&api, "tenant-a").await;
    let (ws_b, _token_b) = register(&api, "tenant-b").await;

    // A caller authenticated as tenant A cannot smuggle a write into tenant
    // B via the body: resolve_workspace gives the token precedence.
    let created = api
        .post(
            "/api/entity",
            json!({
                "module": "notes",
                "type": "note",
                "title": "scoping check",
                "workspace_id": ws_b
            }),
            Some(&token_a),
        )
        .await;
    assert!(created.is_success(), "create failed: {:?}", created.body);
    assert_eq!(
        created.body["workspace_id"],
        json!(ws_a),
        "write must land in the token's workspace, not the body's"
    );
}
