//! status / metrics / file / config commands.

use crate::cli::{ConfigCmd, FileCmd};
use crate::client::{CliError, Client};
use crate::config::CliConfig;
use crate::output::Output;
use base64::Engine as _;
use reqwest::Method;
use serde_json::{json, Value};

pub async fn status(client: &Client, out: Output) -> Result<(), CliError> {
    match client.request(Method::GET, "/api/health", &[], None).await {
        Ok(v) => {
            let ws = v.get("workspace_id").and_then(Value::as_str).unwrap_or("?");
            out.ok(&format!("lifeos-api: ONLINE (workspace {ws})"), &v);
            Ok(())
        }
        Err(CliError::Connection(_)) => {
            out.ok(
                "lifeos-api: OFFLINE",
                &json!({ "status": "offline", "api": false }),
            );
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub async fn metrics(client: &Client, out: Output) -> Result<(), CliError> {
    let v = client.request(Method::GET, "/api/metrics", &[], None).await?;
    out.ok("metrics", &v);
    Ok(())
}

/// `file` maps to the real lifeos-vcs HTTP surface (issue #86, building on
/// #81-#85): commit (read a local path, upload the bytes), history (a plain
/// query over `version.created` events), checkout (retrieval by hash - no
/// separate mutating verb, and no history-rewrite verb exists at all, per
/// docs/AGENT-CONTROL.md §1).
pub async fn file(client: &Client, out: Output, cmd: FileCmd) -> Result<(), CliError> {
    match cmd {
        FileCmd::History { entity_id } => {
            let q = vec![("entity_id", entity_id)];
            let v = client.request(Method::GET, "/api/vcs/history", &q, None).await?;
            out.ok("history", &v);
        }
        FileCmd::Commit { path, message, entity_id } => {
            let bytes = std::fs::read(&path).map_err(|e| CliError::Local(format!("cannot read '{path}': {e}")))?;
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            let content_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

            let mut body = serde_json::Map::new();
            body.insert("name".into(), Value::String(name));
            body.insert("content_base64".into(), Value::String(content_base64));
            if let Some(m) = message {
                body.insert("message".into(), Value::String(m));
            }
            if let Some(e) = entity_id {
                body.insert("entity_id".into(), Value::String(e));
            }
            let v = client
                .request(Method::POST, "/api/vcs/commit", &[], Some(Value::Object(body)))
                .await?;
            out.ok("committed", &v);
        }
        FileCmd::Checkout { entity_id, blob_ref, out: out_path } => {
            let q = vec![("entity_id", entity_id), ("blob_ref", blob_ref.unwrap_or_default())];
            let bytes = client.request_raw(Method::GET, "/api/vcs/checkout", &q).await?;
            std::fs::write(&out_path, &bytes).map_err(|e| CliError::Local(format!("cannot write '{out_path}': {e}")))?;
            out.ok(
                &format!("checked out {} bytes to {out_path}", bytes.len()),
                &json!({ "out": out_path, "bytes": bytes.len() }),
            );
        }
    }
    Ok(())
}

const KNOWN_KEYS: [&str; 3] = ["api_url", "token", "workspace"];

pub fn config(out: Output, cmd: ConfigCmd) -> Result<(), CliError> {
    match cmd {
        ConfigCmd::Path => {
            out.ok("", &json!({ "path": CliConfig::path().display().to_string() }));
        }
        ConfigCmd::List => {
            let cfg = CliConfig::load();
            // Mask the token so `config list` is safe to paste into a log.
            let masked = cfg.token.as_ref().map(|t| mask(t));
            out.ok(
                "",
                &json!({
                    "api_url": cfg.api_url,
                    "token": masked,
                    "workspace": cfg.workspace,
                }),
            );
        }
        ConfigCmd::Get { key } => {
            ensure_known(&key)?;
            let cfg = CliConfig::load();
            out.ok("", &json!({ &key: cfg.get(&key) }));
        }
        ConfigCmd::Set { key, value } => {
            ensure_known(&key)?;
            let mut cfg = CliConfig::load();
            cfg.set(&key, value);
            cfg.save().map_err(|e| CliError::Local(format!("could not write config: {e}")))?;
            out.ok(&format!("set {key}"), &json!({ "ok": true }));
        }
    }
    Ok(())
}

fn ensure_known(key: &str) -> Result<(), CliError> {
    if KNOWN_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(CliError::Local(format!(
            "unknown config key '{key}' (known: {})",
            KNOWN_KEYS.join(", ")
        )))
    }
}

fn mask(token: &str) -> String {
    if token.len() <= 8 {
        "****".into()
    } else {
        format!("{}…{}", &token[..4], &token[token.len() - 4..])
    }
}
