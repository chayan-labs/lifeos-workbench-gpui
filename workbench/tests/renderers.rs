//! Issues #10-#12 end-to-end: real manifests drive the renderer dispatch
//! over real entities served by lifeos-api in-process, and a board card
//! move persists through the same API.

use lifeos_api::config::Config;
use lifeos_workbench::api::InProcessApi;
use lifeos_workbench::manifest;
use lifeos_workbench::views::{dispatch, Rendered};
use serde_json::{json, Value};
use std::path::Path;

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

async fn setup() -> (TempDb, InProcessApi, String) {
    let db = TempDb(
        std::env::temp_dir()
            .join(format!("workbench_render_{}.db", ulid::Ulid::new()))
            .to_string_lossy()
            .to_string(),
    );
    let api = InProcessApi::new(test_config(&db.0)).await.expect("state");
    let reg = api
        .post(
            "/api/register",
            json!({"email": "render@test.example", "name": "render",
                   "password": "test-password-123", "workspace_name": "render"}),
            None,
        )
        .await;
    assert!(reg.is_success(), "register: {:?}", reg.body);
    let token = reg.body["key_token"].as_str().unwrap().to_string();
    (db, api, token)
}

async fn create_task(
    api: &InProcessApi,
    token: &str,
    title: &str,
    status: &str,
    priority: &str,
) -> String {
    let created = api
        .post(
            "/api/entity",
            json!({"module": "tasks", "type": "task", "title": title, "status": status,
                   "attrs": {"priority": priority}}),
            Some(token),
        )
        .await;
    assert!(created.is_success(), "create: {:?}", created.body);
    created.body["id"].as_str().unwrap().to_string()
}

async fn fetch_tasks(api: &InProcessApi, token: &str) -> Vec<Value> {
    let listed = api.get("/api/entity?type=task", Some(token)).await;
    assert!(listed.is_success(), "list: {:?}", listed.body);
    listed.body.as_array().cloned().unwrap_or_default()
}

fn tasks_module() -> manifest::ModuleManifest {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../modules");
    let (manifests, errors) = manifest::load_all(&root);
    assert!(errors.is_empty(), "{errors:?}");
    manifests
        .into_iter()
        .find(|m| m.id == "tasks")
        .expect("tasks module")
}

#[tokio::test]
async fn real_manifest_renders_list_and_filters_from_real_entities() {
    let (_db, api, token) = setup().await;
    create_task(&api, &token, "Ship the fork", "in_progress", "high").await;
    create_task(&api, &token, "Later thing", "todo", "low").await;

    let module = tasks_module();
    let view = module
        .views
        .iter()
        .find(|v| v.kind == "list")
        .expect("list view");
    let et = &module.entity_types[&view.entity_type];
    let entities = fetch_tasks(&api, &token).await;

    let Rendered::List(list) = dispatch(view, et, &entities) else {
        panic!("expected list");
    };
    // The upstream 'My Today' view filters status = 'in_progress'.
    assert_eq!(list.rows.len(), 1, "filter must apply");
    assert_eq!(list.rows[0].title, "Ship the fork");
    assert_eq!(list.rows[0].badge, "high");
}

#[tokio::test]
async fn board_renders_and_card_moves_persist_via_lifeos_api() {
    let (_db, api, token) = setup().await;
    let id = create_task(&api, &token, "Move me", "todo", "medium").await;

    let module = tasks_module();
    let view = module
        .views
        .iter()
        .find(|v| v.kind == "board")
        .expect("board view");
    let et = &module.entity_types[&view.entity_type];

    let Rendered::Board(board) = dispatch(view, et, &fetch_tasks(&api, &token).await) else {
        panic!("expected board");
    };
    let (entity_id, new_status) = board.move_card(&id, 1).expect("move right");
    assert_eq!(new_status, "in_progress");

    // Persist the move through the same in-process API the SPA would use.
    let updated = api
        .request(
            "PATCH",
            &format!("/api/entity/{entity_id}"),
            Some(json!({"status": new_status})),
            Some(&token),
        )
        .await;
    assert!(updated.is_success(), "patch: {:?}", updated.body);

    // Re-render: the card is now in the in_progress column.
    let Rendered::Board(board) = dispatch(view, et, &fetch_tasks(&api, &token).await) else {
        panic!("expected board");
    };
    let col = board
        .columns
        .iter()
        .find(|c| c.status == "in_progress")
        .unwrap();
    assert!(
        col.cards.iter().any(|c| c.id == id),
        "moved card must persist"
    );
}
