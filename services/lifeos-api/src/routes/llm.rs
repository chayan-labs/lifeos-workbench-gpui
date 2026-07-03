//! `GET /api/agents` + `POST /api/llm` - the OpenDesign-style local agent router.
//!
//! `/api/agents` lists the agent CLIs detected on PATH (and the default).
//! `/api/llm` routes `{ system?, prompt, agent?, model? }` to the chosen/default
//! agent and returns `{ text }` - the exact shape every AI surface already reads.

use crate::agents;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn agents(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "agents": &*state.agents,
        "default": state.default_agent(),
    }))
}

#[derive(Deserialize)]
pub struct LlmRequest {
    prompt: String,
    system: Option<String>,
    /// Which detected agent to use; omit for the default.
    agent: Option<String>,
    /// Optional model override passed through to the agent CLI.
    model: Option<String>,
}

pub async fn llm(
    State(state): State<AppState>,
    Json(req): Json<LlmRequest>,
) -> ApiResult<Json<Value>> {
    if req.prompt.trim().is_empty() {
        return Err(ApiError::BadRequest("prompt is required".into()));
    }

    let text = agents::run(
        &state.agents,
        &state.config,
        req.agent.as_deref(),
        req.system.as_deref(),
        req.model.as_deref(),
        &req.prompt,
    )
    .await?;

    let used = req.agent.or_else(|| state.default_agent());
    Ok(Json(json!({ "text": text, "agent": used })))
}
