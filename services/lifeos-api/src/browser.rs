//! Browser actuator via the vendored `external/browser-use` submodule
//! (issue #54, docs/INTEGRATIONS.md §4, docs/SECURITY.md §4). Drives a real
//! browser for services with no usable API.
//!
//! `scrape` runs with every state-changing browser-use action
//! (`click`/`input_text`/`upload_file`/`send_keys`/...) excluded from the
//! agent's action space entirely (`scripts/browser_actuator.py`,
//! `Tools(exclude_actions=[...])`) - it cannot change external state even
//! if an adversarial task string asked it to, so it needs no gating.
//!
//! Deliberately no generic "act" method on this trait: performing an
//! arbitrary (possibly state-changing) task is a gated action
//! (`routes/browser.rs`) that only ever creates a draft entity, never calls
//! this client at all. `capture_session` is the one interactive exception -
//! it opens a real headed browser for a human to log into themselves; it
//! never submits anything on the agent's behalf.

use crate::error::{ApiError, ApiResult};
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait BrowserActuator: Send + Sync {
    /// Read-only: navigate to `url` and perform `task` using a
    /// state-change-free action set. Returns the agent's final result.
    async fn scrape(&self, url: &str, task: &str) -> ApiResult<Value>;

    /// Interactive, Mac-only: opens a real browser window for a human to log
    /// into `site` themselves, then returns the captured session (cookies +
    /// storage) as an opaque string for the caller to encrypt. Never called
    /// from a gated draft.
    async fn capture_session(&self, site: &str) -> ApiResult<String>;
}

/// Real implementation: spawns `python3 <script> <subcommand> ...` against
/// the vendored `external/browser-use` package, same subprocess pattern as
/// `routes/search.rs`'s memvec call.
pub struct ProcessBrowserActuator {
    script_path: String,
    timeout: std::time::Duration,
}

impl ProcessBrowserActuator {
    pub fn new(script_path: String, timeout_secs: u64) -> Self {
        Self { script_path, timeout: std::time::Duration::from_secs(timeout_secs) }
    }

    async fn run(&self, args: &[&str]) -> ApiResult<Vec<u8>> {
        let child = tokio::process::Command::new("python3")
            .arg(&self.script_path)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output();

        let output = tokio::time::timeout(self.timeout, child)
            .await
            .map_err(|_| ApiError::Upstream("browser actuator timed out".into()))?
            .map_err(|e| {
                tracing::error!("browser actuator subprocess failed to launch: {e}");
                ApiError::Upstream("browser actuator unavailable".into())
            })?;

        if !output.status.success() {
            tracing::error!(
                "browser actuator exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(ApiError::Upstream("browser actuator failed".into()));
        }
        Ok(output.stdout)
    }
}

#[async_trait]
impl BrowserActuator for ProcessBrowserActuator {
    async fn scrape(&self, url: &str, task: &str) -> ApiResult<Value> {
        let stdout = self.run(&["scrape", url, task]).await?;
        serde_json::from_slice(&stdout).map_err(|e| {
            tracing::error!("browser actuator scrape response decode failed: {e}");
            ApiError::Upstream("malformed browser actuator response".into())
        })
    }

    async fn capture_session(&self, site: &str) -> ApiResult<String> {
        let stdout = self.run(&["capture-session", site]).await?;
        String::from_utf8(stdout).map_err(|e| {
            tracing::error!("browser actuator session response decode failed: {e}");
            ApiError::Upstream("malformed browser actuator response".into())
        })
    }
}

/// In-memory fake for tests - no real Python/browser-use/Chromium needed to
/// exercise the API surface. Exposed unconditionally so `tests/` can
/// construct one too.
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct MockBrowserActuator {
        scrape_responses: Mutex<std::collections::HashMap<String, Value>>,
        session_responses: Mutex<std::collections::HashMap<String, String>>,
        /// Every call made, in order - lets tests assert a gated `act`
        /// request never reached this client at all.
        pub calls: Mutex<Vec<String>>,
    }

    impl MockBrowserActuator {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn seed_scrape(&self, url: &str, response: Value) {
            self.scrape_responses.lock().unwrap().insert(url.to_string(), response);
        }

        pub fn seed_session(&self, site: &str, session: &str) {
            self.session_responses.lock().unwrap().insert(site.to_string(), session.to_string());
        }
    }

    #[async_trait]
    impl BrowserActuator for MockBrowserActuator {
        async fn scrape(&self, url: &str, _task: &str) -> ApiResult<Value> {
            self.calls.lock().unwrap().push("scrape".into());
            self.scrape_responses
                .lock()
                .unwrap()
                .get(url)
                .cloned()
                .ok_or_else(|| ApiError::Upstream("no mock scrape response seeded".into()))
        }

        async fn capture_session(&self, site: &str) -> ApiResult<String> {
            self.calls.lock().unwrap().push("capture_session".into());
            self.session_responses
                .lock()
                .unwrap()
                .get(site)
                .cloned()
                .ok_or_else(|| ApiError::Upstream("no mock session seeded".into()))
        }
    }
}
