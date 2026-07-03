//! Local agent-CLI router - "the agents you already have on your machine are
//! the engine" (docs/ARCHITECTURE.md, owned-credential model). Shared by
//! `lifeos-api` (`/api/agents` + `/api/llm`) and `lifeos-drain` (pipeline
//! stages, eval judge, media captioning) so every AI lane in the system runs
//! keyless through an already-authenticated local CLI.
//!
//! We scan `PATH` for known coding-agent CLIs (Claude Code, Gemini CLI,
//! Codex, OpenCode, Hermes, Antigravity, ...). Each known CLI has a small
//! adapter describing how to invoke it headlessly and how to read its output.
//! A prompt is routed to the chosen (or default) detected agent, spawned as a
//! subprocess with no shell (so prompts can never be injected as shell
//! syntax), and its text answer is returned.
//!
//! Defaults are switchable at runtime: `LIFEOS_AGENT` / `LIFEOS_AGENT_MODEL`
//! env vars pick the fleet-wide default; every call can still override both
//! per request (that is what the frontend's on-the-go picker does).

use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Error from detection or a CLI invocation. Kept as a plain message so both
/// consumers can wrap it into their own error enums.
#[derive(Debug)]
pub enum AgentError {
    /// The request named an agent that is not installed / not known.
    NotInstalled(String),
    /// No agent CLI was found on PATH at all.
    NoneDetected,
    /// Spawn/IO/timeout/output-shape failure from the CLI itself.
    Invocation(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::NotInstalled(id) => write!(f, "agent '{id}' is not installed"),
            AgentError::NoneDetected => write!(
                f,
                "no local agent CLI detected on PATH (install Claude Code, Gemini CLI, ...)"
            ),
            AgentError::Invocation(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AgentError {}

/// How the prompt text is delivered to the CLI.
#[derive(Clone, Copy, Debug)]
enum PromptMode {
    /// Passed as the value of a flag, e.g. `gemini -p "<prompt>"`.
    Flag(&'static str),
    /// Passed as the final positional argument, e.g. `opencode run "<prompt>"`.
    PositionalLast,
    /// Written to the process's stdin. Part of the adapter contract; no
    /// currently-registered CLI uses it, but the spawn path supports it.
    #[allow(dead_code)]
    Stdin,
}

/// How to read the answer out of the CLI's stdout.
#[derive(Clone, Copy, Debug)]
enum OutputParse {
    /// Trimmed stdout is the answer.
    Raw,
    /// Parse stdout as JSON and read this top-level string field.
    JsonField(&'static str),
}

/// Static description of how to drive one known agent CLI.
#[derive(Clone, Debug)]
struct AgentSpec {
    id: &'static str,
    label: &'static str,
    bin: &'static str,
    /// Fixed args placed before the prompt (sub-command, output flags, ...).
    base_args: &'static [&'static str],
    prompt_mode: PromptMode,
    /// Flag used to inject a system prompt, if the CLI supports one.
    system_flag: Option<&'static str>,
    /// Flag used to select a model, if supported.
    model_flag: Option<&'static str>,
    /// Suggested model ids for the picker UI - suggestions only, never a
    /// closed list; any string the CLI accepts can be passed through.
    models: &'static [&'static str],
    output: OutputParse,
    /// `false` => invocation contract is a best guess; surfaced to the client.
    verified: bool,
}

/// The known-CLI registry. Detection filters this down to what is installed.
/// Ordering also defines default-agent preference (first installed wins,
/// unless `LIFEOS_AGENT` overrides).
const REGISTRY: &[AgentSpec] = &[
    AgentSpec {
        id: "claude",
        label: "Claude Code",
        bin: "claude",
        base_args: &["-p", "--output-format", "json"],
        prompt_mode: PromptMode::PositionalLast,
        system_flag: Some("--append-system-prompt"),
        model_flag: Some("--model"),
        models: &["opus", "sonnet", "haiku"],
        output: OutputParse::JsonField("result"),
        verified: true,
    },
    AgentSpec {
        id: "gemini",
        label: "Gemini CLI",
        bin: "gemini",
        base_args: &["-o", "text"],
        prompt_mode: PromptMode::Flag("-p"),
        system_flag: None,
        model_flag: Some("-m"),
        models: &["gemini-3-pro-preview", "gemini-2.5-pro", "gemini-2.5-flash"],
        output: OutputParse::Raw,
        verified: true,
    },
    AgentSpec {
        id: "codex",
        label: "OpenAI Codex",
        bin: "codex",
        base_args: &["exec"],
        prompt_mode: PromptMode::PositionalLast,
        system_flag: None,
        model_flag: Some("-m"),
        models: &["gpt-5.1-codex", "gpt-5.1-codex-mini"],
        output: OutputParse::Raw,
        verified: false,
    },
    AgentSpec {
        id: "opencode",
        label: "OpenCode",
        bin: "opencode",
        base_args: &["run"],
        prompt_mode: PromptMode::PositionalLast,
        system_flag: None,
        model_flag: Some("-m"),
        models: &[
            "anthropic/claude-sonnet-4-5",
            "anthropic/claude-haiku-4-5",
            "google/gemini-2.5-pro",
        ],
        output: OutputParse::Raw,
        verified: true,
    },
    AgentSpec {
        id: "hermes",
        label: "Hermes Agent",
        bin: "hermes",
        base_args: &[],
        prompt_mode: PromptMode::Flag("-z"),
        system_flag: None,
        model_flag: Some("-m"),
        models: &["hermes-4-405b", "hermes-4-70b"],
        output: OutputParse::Raw,
        verified: true,
    },
    AgentSpec {
        id: "antigravity",
        label: "Antigravity (Google)",
        bin: "antigravity",
        base_args: &["run"],
        prompt_mode: PromptMode::PositionalLast,
        system_flag: None,
        model_flag: Some("-m"),
        models: &["gemini-3-pro-preview"],
        output: OutputParse::Raw,
        verified: false,
    },
    AgentSpec {
        id: "openclaw",
        label: "OpenClaw",
        bin: "openclaw",
        base_args: &["agent", "--local"],
        prompt_mode: PromptMode::PositionalLast,
        system_flag: None,
        model_flag: None,
        models: &[],
        output: OutputParse::Raw,
        verified: false,
    },
];

/// A registry entry that was found on `PATH`.
#[derive(Clone, Debug, Serialize)]
pub struct DetectedAgent {
    pub id: String,
    pub label: String,
    pub bin: String,
    pub path: String,
    pub verified: bool,
    /// Suggested models for this CLI (picker suggestions, not a closed list).
    pub models: Vec<String>,
    #[serde(skip)]
    spec_index: usize,
}

/// Scan `PATH` once and return the installed subset of the registry, in
/// preference order.
pub fn detect() -> Vec<DetectedAgent> {
    REGISTRY
        .iter()
        .enumerate()
        .filter_map(|(i, spec)| {
            which_on_path(spec.bin).map(|path| DetectedAgent {
                id: spec.id.to_string(),
                label: spec.label.to_string(),
                bin: spec.bin.to_string(),
                path: path.to_string_lossy().to_string(),
                verified: spec.verified,
                models: spec.models.iter().map(|m| m.to_string()).collect(),
                spec_index: i,
            })
        })
        .collect()
}

/// The default agent id: `LIFEOS_AGENT` if that agent is installed, else the
/// first detected (registry preference order).
pub fn default_agent_id(agents: &[DetectedAgent]) -> Option<String> {
    if let Ok(preferred) = std::env::var("LIFEOS_AGENT") {
        if agents.iter().any(|a| a.id == preferred) {
            return Some(preferred);
        }
    }
    agents.first().map(|a| a.id.clone())
}

/// The default model override, if any (`LIFEOS_AGENT_MODEL`). Per-request
/// model choices always win over this.
pub fn default_model() -> Option<String> {
    std::env::var("LIFEOS_AGENT_MODEL").ok().filter(|s| !s.is_empty())
}

/// Minimal `which`: first entry on `PATH` that contains an executable `bin`.
fn which_on_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &std::path::Path) -> bool {
    p.is_file()
}

/// Per-invocation options. `agent_id`/`model` of `None` fall back to
/// `LIFEOS_AGENT` / `LIFEOS_AGENT_MODEL`, then registry preference order.
#[derive(Clone, Debug, Default)]
pub struct RunOptions {
    pub agent_id: Option<String>,
    pub system: Option<String>,
    pub model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub timeout_secs: u64,
}

/// Run a prompt through a detected agent and return its text answer.
pub async fn run(
    agents: &[DetectedAgent],
    opts: &RunOptions,
    prompt: &str,
) -> Result<String, AgentError> {
    let resolved_id = opts
        .agent_id
        .clone()
        .or_else(|| default_agent_id(agents));
    let detected = match resolved_id {
        Some(id) => agents
            .iter()
            .find(|a| a.id == id)
            .ok_or(AgentError::NotInstalled(id))?,
        None => return Err(AgentError::NoneDetected),
    };
    let spec = &REGISTRY[detected.spec_index];
    let model = opts.model.clone().or_else(default_model);

    // Build the argv with no shell involvement.
    let mut args: Vec<String> = spec.base_args.iter().map(|s| s.to_string()).collect();
    if let (Some(flag), Some(sys)) = (spec.system_flag, opts.system.as_deref()) {
        if !sys.is_empty() {
            args.push(flag.to_string());
            args.push(sys.to_string());
        }
    }
    if let (Some(flag), Some(m)) = (spec.model_flag, model.as_deref()) {
        if !m.is_empty() {
            args.push(flag.to_string());
            args.push(m.to_string());
        }
    }
    let mut use_stdin = false;
    match spec.prompt_mode {
        PromptMode::Flag(flag) => {
            args.push(flag.to_string());
            args.push(prompt.to_string());
        }
        PromptMode::PositionalLast => args.push(prompt.to_string()),
        PromptMode::Stdin => use_stdin = true,
    }

    let mut cmd = Command::new(&detected.bin);
    cmd.args(&args)
        .stdin(if use_stdin { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &opts.cwd {
        cmd.current_dir(cwd);
    }

    tracing::info!(agent = %detected.id, model = model.as_deref().unwrap_or("default"), "invoking local agent CLI");
    let mut child = cmd
        .spawn()
        .map_err(|e| AgentError::Invocation(format!("failed to spawn {}: {e}", detected.bin)))?;

    if use_stdin {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes()).await;
            let _ = stdin.shutdown().await;
        }
    }

    let timeout = std::time::Duration::from_secs(opts.timeout_secs.max(1));
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(AgentError::Invocation(format!("agent process error: {e}"))),
        Err(_) => {
            return Err(AgentError::Invocation(format!(
                "agent '{}' timed out after {}s",
                detected.id,
                opts.timeout_secs.max(1)
            )))
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AgentError::Invocation(format!(
            "agent '{}' exited with {}: {}",
            detected.id,
            output.status,
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_output(spec.output, &stdout, &detected.id)
}

fn parse_output(mode: OutputParse, stdout: &str, agent_id: &str) -> Result<String, AgentError> {
    match mode {
        OutputParse::Raw => Ok(stdout.trim().to_string()),
        OutputParse::JsonField(field) => {
            let v: serde_json::Value = serde_json::from_str(stdout.trim()).map_err(|e| {
                AgentError::Invocation(format!("agent '{agent_id}' returned non-JSON output: {e}"))
            })?;
            v.get(field)
                .and_then(|f| f.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    AgentError::Invocation(format!(
                        "agent '{agent_id}' JSON missing '{field}' field"
                    ))
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_known_subset_in_preference_order() {
        let found = detect();
        // Every detected agent must correspond to a real registry id.
        for a in &found {
            assert!(REGISTRY.iter().any(|s| s.id == a.id));
            assert!(!a.path.is_empty());
        }
    }

    #[test]
    fn every_registry_entry_serializes_model_suggestions() {
        for (i, spec) in REGISTRY.iter().enumerate() {
            let agent = DetectedAgent {
                id: spec.id.to_string(),
                label: spec.label.to_string(),
                bin: spec.bin.to_string(),
                path: "/tmp/x".into(),
                verified: spec.verified,
                models: spec.models.iter().map(|m| m.to_string()).collect(),
                spec_index: i,
            };
            let v = serde_json::to_value(&agent).unwrap();
            assert!(v.get("models").unwrap().is_array());
            assert!(v.get("spec_index").is_none(), "spec_index must not leak");
        }
    }

    #[test]
    fn json_field_extraction_reads_claude_result_shape() {
        let out = r#"{"type":"result","subtype":"success","result":"hello world"}"#;
        let text = parse_output(OutputParse::JsonField("result"), out, "claude").unwrap();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn raw_output_is_trimmed() {
        let text = parse_output(OutputParse::Raw, "  answer\n", "x").unwrap();
        assert_eq!(text, "answer");
    }

    #[tokio::test]
    async fn unknown_agent_id_is_a_clear_error() {
        let agents = detect();
        let opts = RunOptions {
            agent_id: Some("definitely-not-installed".into()),
            timeout_secs: 5,
            ..Default::default()
        };
        let err = run(&agents, &opts, "hi").await.unwrap_err();
        assert!(matches!(err, AgentError::NotInstalled(_)));
    }
}
