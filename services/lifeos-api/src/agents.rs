//! Thin adapter over the shared `lifeos-agents` crate (the local agent-CLI
//! router used by every AI lane in the system). This module only maps the
//! crate's error type into `ApiError` and threads the API config's cwd and
//! timeout - the registry, detection and spawn logic live in `lifeos-agents`
//! so `lifeos-drain` uses the exact same engine.

use crate::config::Config;
use crate::error::ApiError;

pub use lifeos_agents::{default_agent_id, detect, DetectedAgent};

impl From<lifeos_agents::AgentError> for ApiError {
    fn from(e: lifeos_agents::AgentError) -> Self {
        match e {
            lifeos_agents::AgentError::NotInstalled(_) => ApiError::BadRequest(e.to_string()),
            lifeos_agents::AgentError::NoneDetected => ApiError::NotImplemented(e.to_string()),
            lifeos_agents::AgentError::Invocation(msg) => ApiError::Upstream(msg),
        }
    }
}

/// Run a prompt through a detected agent and return its text answer.
pub async fn run(
    agents: &[DetectedAgent],
    config: &Config,
    agent_id: Option<&str>,
    system: Option<&str>,
    model: Option<&str>,
    prompt: &str,
) -> Result<String, ApiError> {
    let opts = lifeos_agents::RunOptions {
        agent_id: agent_id.map(|s| s.to_string()),
        system: system.map(|s| s.to_string()),
        model: model.map(|s| s.to_string()),
        cwd: config.agent_cwd.clone().map(std::path::PathBuf::from),
        timeout_secs: config.agent_timeout_secs,
    };
    Ok(lifeos_agents::run(agents, &opts, prompt).await?)
}
