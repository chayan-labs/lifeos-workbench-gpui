//! Per-stage agent execution (issue #92). Same DI-trait shape as
//! `lifeos-ingest/src/vision.rs::Captioner`: a `NoopStageRunner` fails
//! loudly (a pipeline stage is not optional - unlike OCR in the ingest
//! crate, there is no safe "degrade to empty" for an agent stage) and a
//! `HaikuStageRunner` calls the Anthropic Messages API directly over
//! `reqwest`. No Rust "Claude Agent SDK" crate exists anywhere in this
//! workspace (checked before writing this) - this is the established
//! pattern, not a shortcut.

use crate::StageSpec;
use async_trait::async_trait;
use serde_json::{json, Value};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const STAGE_MODEL: &str = "claude-haiku-4-5-20251001";

#[derive(Debug, Clone)]
pub struct StageResult {
    pub output: Value,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub model: String,
}

/// Runs one DAG stage and returns its output. `input` is the pipeline run's
/// original input; `prior` is every earlier stage's output, in order.
#[async_trait]
pub trait PipelineStageRunner: Send + Sync {
    async fn run_stage(&self, stage: &StageSpec, input: &Value, prior: &[Value]) -> Result<StageResult, String>;
}

/// Used when `ANTHROPIC_API_KEY` is unset. Fails loudly: a pipeline job
/// that can't actually run its stages must fail, not silently "complete"
/// with fabricated output.
pub struct NoopStageRunner;

#[async_trait]
impl PipelineStageRunner for NoopStageRunner {
    async fn run_stage(&self, _stage: &StageSpec, _input: &Value, _prior: &[Value]) -> Result<StageResult, String> {
        Err("no pipeline stage runner configured (ANTHROPIC_API_KEY unset)".to_string())
    }
}

/// Real stage execution via the Anthropic Messages API (Haiku).
pub struct HaikuStageRunner {
    pub api_key: String,
}

fn build_prompt(stage: &StageSpec, input: &Value, prior: &[Value]) -> String {
    let mut prompt = format!("You are the '{}' stage of an agent pipeline.\n", stage.agent);
    if let Some(skill) = stage.skill {
        prompt.push_str(&format!("Apply the '{skill}' skill.\n"));
    }
    if let Some(tool) = stage.tool {
        prompt.push_str(&format!("(Reference tool for this stage: {tool}; not actually invoked by this runner.)\n"));
    }
    prompt.push_str(&format!("Run input: {input}\n"));
    if !prior.is_empty() {
        prompt.push_str(&format!("Prior stage outputs: {}\n", Value::Array(prior.to_vec())));
    }
    prompt.push_str("Respond with the stage's output as plain text.");
    prompt
}

#[async_trait]
impl PipelineStageRunner for HaikuStageRunner {
    async fn run_stage(&self, stage: &StageSpec, input: &Value, prior: &[Value]) -> Result<StageResult, String> {
        let prompt = build_prompt(stage, input, prior);
        let body = json!({
            "model": STAGE_MODEL,
            "max_tokens": 1024,
            "messages": [{ "role": "user", "content": prompt }]
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("anthropic request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("anthropic api error {status}: {text}"));
        }

        let parsed: Value = resp.json().await.map_err(|e| format!("anthropic response parse failed: {e}"))?;
        let text = parsed
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "anthropic response had no stage output text".to_string())?;

        let tokens_in = parsed.get("usage").and_then(|u| u.get("input_tokens")).and_then(|v| v.as_i64()).unwrap_or(0);
        let tokens_out =
            parsed.get("usage").and_then(|u| u.get("output_tokens")).and_then(|v| v.as_i64()).unwrap_or(0);

        Ok(StageResult { output: json!({ "text": text }), tokens_in, tokens_out, model: STAGE_MODEL.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_stage_runner_fails_loudly() {
        let stage = StageSpec { name: "x", agent: "x", tool: None, skill: None, gate: None, gated: false };
        let result = NoopStageRunner.run_stage(&stage, &json!({}), &[]).await;
        assert!(result.unwrap_err().contains("ANTHROPIC_API_KEY"));
    }
}
