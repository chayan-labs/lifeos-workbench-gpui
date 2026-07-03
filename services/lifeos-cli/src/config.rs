//! Local CLI configuration (api url, token, workspace).
//!
//! Stored as JSON at `$XDG_CONFIG_HOME/lifeos/config.json` (falls back to
//! `$HOME/.config/lifeos/config.json`). This is the CLI's own config - it is
//! never sent to the API except as the resolved auth/workspace headers.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_API_URL: &str = "http://127.0.0.1:8080";

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct CliConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

impl CliConfig {
    pub fn path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config")
            });
        base.join("lifeos").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        std::fs::write(&path, raw)
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        match key {
            "api_url" => self.api_url.as_ref(),
            "token" => self.token.as_ref(),
            "workspace" => self.workspace.as_ref(),
            _ => None,
        }
    }

    /// Returns false if the key is not a known setting.
    pub fn set(&mut self, key: &str, value: String) -> bool {
        match key {
            "api_url" => self.api_url = Some(value),
            "token" => self.token = Some(value),
            "workspace" => self.workspace = Some(value),
            _ => return false,
        }
        true
    }
}

/// Effective connection settings after merging flags > env > config > default.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub api_url: String,
    pub token: Option<String>,
    pub workspace: Option<String>,
}

pub fn resolve(
    flag_api_url: Option<String>,
    flag_token: Option<String>,
    flag_workspace: Option<String>,
) -> Resolved {
    let cfg = CliConfig::load();
    let api_url = flag_api_url
        .or_else(|| env_nonempty("LIFEOS_API_URL"))
        .or(cfg.api_url)
        .unwrap_or_else(|| DEFAULT_API_URL.to_string());
    let token = flag_token
        .or_else(|| env_nonempty("LIFEOS_TOKEN"))
        .or(cfg.token);
    let workspace = flag_workspace
        .or_else(|| env_nonempty("LIFEOS_WORKSPACE"))
        .or(cfg.workspace);
    Resolved {
        api_url: api_url.trim_end_matches('/').to_string(),
        token,
        workspace,
    }
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_roundtrip() {
        let mut cfg = CliConfig::default();
        assert!(cfg.set("api_url", "http://x".into()));
        assert!(cfg.set("token", "abc".into()));
        assert_eq!(cfg.get("api_url"), Some(&"http://x".to_string()));
        assert_eq!(cfg.get("token"), Some(&"abc".to_string()));
        assert!(cfg.get("workspace").is_none());
    }

    #[test]
    fn set_rejects_unknown_key() {
        let mut cfg = CliConfig::default();
        assert!(!cfg.set("bogus", "v".into()));
    }

    #[test]
    fn explicit_flags_win_over_everything() {
        // Flags take priority regardless of env/config; trailing slash trimmed.
        let r = resolve(
            Some("http://flag:1/".into()),
            Some("flagtok".into()),
            Some("ws_flag".into()),
        );
        assert_eq!(r.api_url, "http://flag:1");
        assert_eq!(r.token.as_deref(), Some("flagtok"));
        assert_eq!(r.workspace.as_deref(), Some("ws_flag"));
    }
}
