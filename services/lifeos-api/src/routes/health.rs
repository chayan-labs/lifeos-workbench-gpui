//! `GET /api/health` - liveness probe. The frontend checks `data.status === 'healthy'`.

use crate::config::DEFAULT_WORKSPACE;
use crate::error::ApiResult;
use crate::state::AppState;
use axum::{extract::State, Json};
use serde_json::{json, Value};

pub async fn health(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    // Touch the DB so "healthy" means the data plane is actually reachable.
    let mut rows = state
        .conn
        .query("SELECT id FROM workspaces LIMIT 1", ())
        .await?;
    let workspace_id = match rows.next().await? {
        Some(row) => row.get::<String>(0).unwrap_or_else(|_| DEFAULT_WORKSPACE.to_string()),
        None => DEFAULT_WORKSPACE.to_string(),
    };

    Ok(Json(json!({
        "status": "healthy",
        "workspace_id": workspace_id,
        "agents": state.agents.len(),
    })))
}
