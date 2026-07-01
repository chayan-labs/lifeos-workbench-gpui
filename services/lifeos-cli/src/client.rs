//! Thin authed HTTP client over lifeos-api. The CLI NEVER touches the DB
//! directly - every operation round-trips through localhost HTTP.

use crate::config::Resolved;
use reqwest::Method;
use serde_json::Value;

/// Structured failure with a stable exit code (see `exit_code`).
#[derive(Debug)]
pub enum CliError {
    /// Could not reach the API (connection refused, DNS, timeout).
    Connection(String),
    /// API responded with a non-2xx status.
    Api { status: u16, body: String },
    /// Response body could not be parsed as JSON.
    Parse(String),
    /// Local/usage error (bad input, IO).
    Local(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Local(_) => 2,
            CliError::Api { .. } => 3,
            CliError::Connection(_) => 4,
            CliError::Parse(_) => 5,
        }
    }

    pub fn message(&self) -> String {
        match self {
            CliError::Connection(m) => format!(
                "cannot reach lifeos-api: {m}\nhint: is it running? `cargo run -p lifeos-api`"
            ),
            CliError::Api { status, body } => format!("API error {status}: {body}"),
            CliError::Parse(m) => format!("could not parse API response: {m}"),
            CliError::Local(m) => m.clone(),
        }
    }
}

pub struct Client {
    http: reqwest::Client,
    settings: Resolved,
}

impl Client {
    pub fn new(settings: Resolved) -> Self {
        Self {
            http: reqwest::Client::new(),
            settings,
        }
    }

    /// Issue a request and return the parsed JSON body on success.
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value, CliError> {
        let url = format!("{}{}", self.settings.api_url, path);
        let mut req = self.http.request(method, &url);

        let filtered: Vec<(&str, String)> = query
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .cloned()
            .collect();
        if !filtered.is_empty() {
            req = req.query(&filtered);
        }
        if let Some(token) = &self.settings.token {
            req = req.bearer_auth(token);
        }
        if let Some(ws) = &self.settings.workspace {
            req = req.header("X-Workspace-Id", ws);
        }
        if let Some(json) = body {
            req = req.json(&json);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| CliError::Connection(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| CliError::Connection(e.to_string()))?;

        if !status.is_success() {
            return Err(CliError::Api {
                status: status.as_u16(),
                body: text,
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| CliError::Parse(e.to_string()))
    }

    /// Like `request`, but returns the raw response body instead of parsing
    /// it as JSON - for endpoints like `/api/vcs/checkout` that return file
    /// bytes directly.
    pub async fn request_raw(&self, method: Method, path: &str, query: &[(&str, String)]) -> Result<Vec<u8>, CliError> {
        let url = format!("{}{}", self.settings.api_url, path);
        let mut req = self.http.request(method, &url);

        let filtered: Vec<(&str, String)> = query.iter().filter(|(_, v)| !v.is_empty()).cloned().collect();
        if !filtered.is_empty() {
            req = req.query(&filtered);
        }
        if let Some(token) = &self.settings.token {
            req = req.bearer_auth(token);
        }
        if let Some(ws) = &self.settings.workspace {
            req = req.header("X-Workspace-Id", ws);
        }

        let resp = req.send().await.map_err(|e| CliError::Connection(e.to_string()))?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(|e| CliError::Connection(e.to_string()))?;

        if !status.is_success() {
            return Err(CliError::Api {
                status: status.as_u16(),
                body: String::from_utf8_lossy(&bytes).to_string(),
            });
        }
        Ok(bytes.to_vec())
    }
}
