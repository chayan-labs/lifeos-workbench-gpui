//! The agent toolbelt (issue #17): `workbench --mcp` turns this same binary
//! into a stdio MCP server over the in-process `lifeos-api`, and the ACP
//! session passes it in `mcpServers` - so the coding agent gets hybrid
//! search, memory recall, and entity read/write in one context, API-first
//! thin tools (docs/ARCHITECTURE.md rule 7), no HTTP.
//!
//! Wire: newline-delimited JSON-RPC 2.0 (MCP stdio transport). Methods
//! served: initialize, tools/list, tools/call. Auth: `LIFEOS_TOKEN` if set,
//! else the default workspace - same resolution as the HTTP surface.

use crate::api::InProcessApi;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const PROTOCOL: &str = "2024-11-05";

fn tool(name: &str, description: &str, props: Value, required: Vec<&str>) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": props, "required": required}
    })
}

pub fn tools() -> Vec<Value> {
    vec![
        tool(
            "lifeos_search",
            "Hybrid recall over Life OS (FTS5 lexical + memvec semantic, RRF-fused): \
             code notes, second brain, every module's entities.",
            json!({"query": {"type": "string"}, "limit": {"type": "integer"}}),
            vec!["query"],
        ),
        tool(
            "lifeos_recall",
            "Recall from cognitive memory (past decisions, opinions, context).",
            json!({"query": {"type": "string"}}),
            vec!["query"],
        ),
        tool(
            "lifeos_entity_list",
            "List Life OS entities by type (e.g. task, note, session).",
            json!({"type": {"type": "string"}}),
            vec!["type"],
        ),
        tool(
            "lifeos_entity_create",
            "Create a Life OS entity (e.g. write a debug artifact or note back).",
            json!({"module": {"type": "string"}, "type": {"type": "string"},
                   "title": {"type": "string"}, "attrs": {"type": "object"}}),
            vec!["module", "type", "title"],
        ),
    ]
}

async fn call_tool(api: &InProcessApi, token: Option<&str>, name: &str, args: &Value) -> Value {
    let response = match name {
        "lifeos_search" => {
            let q = args["query"].as_str().unwrap_or_default();
            let limit = args["limit"].as_u64().unwrap_or(20);
            api.get(
                &format!(
                    "/api/search?q={}&limit={limit}",
                    crate::search_pane::urlencode(q)
                ),
                token,
            )
            .await
        }
        "lifeos_recall" => {
            api.post(
                "/api/memory/recall",
                json!({"query": args["query"].as_str().unwrap_or_default()}),
                token,
            )
            .await
        }
        "lifeos_entity_list" => {
            api.get(
                &format!(
                    "/api/entity?type={}",
                    crate::search_pane::urlencode(args["type"].as_str().unwrap_or_default())
                ),
                token,
            )
            .await
        }
        "lifeos_entity_create" => api.post("/api/entity", args.clone(), token).await,
        _ => {
            return json!({"content": [{"type": "text", "text": format!("unknown tool {name}")}],
                          "isError": true})
        }
    };
    json!({
        "content": [{"type": "text", "text": response.body.to_string()}],
        "isError": !response.is_success()
    })
}

/// Handle one MCP message; `None` for notifications (no response due).
pub async fn handle(api: &InProcessApi, token: Option<&str>, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => json!({
            "protocolVersion": PROTOCOL,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "lifeos-workbench", "version": env!("CARGO_PKG_VERSION")}
        }),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or_default();
            call_tool(api, token, name, &msg["params"]["arguments"]).await
        }
        "ping" => json!({}),
        _ => {
            return Some(json!({"jsonrpc": "2.0", "id": id,
                               "error": {"code": -32601, "message": "method not found"}}))
        }
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// Serve MCP on stdin/stdout until EOF. Runs inside the tokio runtime.
pub async fn serve_stdio(api: InProcessApi) -> std::io::Result<()> {
    let token = std::env::var("LIFEOS_TOKEN").ok();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(response) = handle(&api, token.as_deref(), &msg).await {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifeos_api::config::Config;

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

    #[tokio::test]
    async fn serves_initialize_tools_list_and_search_over_lifeos() {
        let db = TempDb(
            std::env::temp_dir()
                .join(format!("wb_mcp_{}.db", std::process::id()))
                .to_string_lossy()
                .to_string(),
        );
        let api = InProcessApi::new(test_config(&db.0)).await.expect("state");
        let reg = api
            .post(
                "/api/register",
                json!({"email": "mcp@test.example", "name": "mcp",
                       "password": "test-password-123", "workspace_name": "mcp"}),
                None,
            )
            .await;
        let token = reg.body["key_token"].as_str().unwrap().to_string();

        let init = handle(
            &api,
            Some(&token),
            &json!({"jsonrpc": "2.0", "id": 1,
            "method": "initialize", "params": {}}),
        )
        .await
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL);

        let list = handle(
            &api,
            Some(&token),
            &json!({"jsonrpc": "2.0", "id": 2,
            "method": "tools/list"}),
        )
        .await
        .unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(names.contains(&"lifeos_search") && names.contains(&"lifeos_entity_create"));

        // Create an entity through the toolbelt, then find it via search.
        let create = handle(
            &api,
            Some(&token),
            &json!({"jsonrpc": "2.0", "id": 3,
            "method": "tools/call",
            "params": {"name": "lifeos_entity_create",
                       "arguments": {"module": "tasks", "type": "task",
                                     "title": "Debug the flaky replay checkpoint"}}}),
        )
        .await
        .unwrap();
        assert_eq!(create["result"]["isError"], false, "{create}");

        let search = handle(
            &api,
            Some(&token),
            &json!({"jsonrpc": "2.0", "id": 4,
            "method": "tools/call",
            "params": {"name": "lifeos_search",
                       "arguments": {"query": "flaky replay checkpoint"}}}),
        )
        .await
        .unwrap();
        assert_eq!(search["result"]["isError"], false, "{search}");
        let text = search["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("Debug the flaky replay"),
            "hit missing: {text}"
        );

        // Notifications get no response.
        assert!(handle(
            &api,
            None,
            &json!({"jsonrpc": "2.0",
            "method": "notifications/initialized"})
        )
        .await
        .is_none());
    }
}
