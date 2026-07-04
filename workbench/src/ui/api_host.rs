//! Shared, lazily-bootstrapped handle to the in-process `lifeos-api`.
//!
//! The Life OS and Recall panes both talk to Life OS through one
//! [`InProcessApi`] (no socket, no second process - see [`crate::api`]). Building
//! it stands up an `AppState` (a libSQL connection + migrations), which is async
//! and can take a moment, so it happens once on the shared tokio runtime and the
//! result is published into this handle. Panes clone the `ApiHost` (cheap - it is
//! an `Arc`) and read whatever state the bootstrap has reached.
//!
//! The default configuration is fully offline (`Config::from_env` falls back to a
//! local `lifeos.db` file with no Turso sync), so the pane data path is real. If
//! the bootstrap fails - a locked DB, a bad `LIFEOS_DB_PATH` - the panes render an
//! honest error rather than pretending the backend is up.

use std::sync::{Arc, Mutex};

use crate::api::{ApiConfig, InProcessApi};

/// Where the one-time bootstrap has got to.
#[derive(Clone)]
enum State {
    Booting,
    Ready {
        api: InProcessApi,
        token: Option<String>,
    },
    Failed(String),
}

/// A cheaply-clonable handle to the in-process API. All clones share one
/// bootstrap.
#[derive(Clone)]
pub struct ApiHost {
    inner: Arc<Mutex<State>>,
}

/// A read-only snapshot of the host state, for a pane to branch on when it
/// renders.
pub enum HostStatus {
    Booting,
    Ready(InProcessApi, Option<String>),
    Failed(String),
}

impl ApiHost {
    /// Create the handle in the `Booting` state. Call [`ApiHost::bootstrap`] on a
    /// tokio runtime to fill it in.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(State::Booting)),
        }
    }

    /// Kick the one-time bootstrap on `runtime`. Idempotent in effect: only the
    /// first published result matters (later panes just read it), but callers
    /// should invoke it exactly once at startup.
    pub fn bootstrap(&self, runtime: &tokio::runtime::Handle) {
        let inner = self.inner.clone();
        runtime.spawn(async move {
            let config = ApiConfig::from_env();
            // Mint a dev token for the seeded personal workspace before `config`
            // is moved into the API, so entity/search requests are scoped exactly
            // as the HTTP surface scopes them.
            let token = mint_dev_token(&config);
            let next = match InProcessApi::new(config).await {
                Ok(api) => State::Ready { api, token },
                Err(e) => State::Failed(format!("lifeos-api bootstrap failed: {e}")),
            };
            if let Ok(mut guard) = inner.lock() {
                *guard = next;
            }
        });
    }

    /// Snapshot the current state for rendering.
    pub fn status(&self) -> HostStatus {
        match self.inner.lock() {
            Ok(guard) => match &*guard {
                State::Booting => HostStatus::Booting,
                State::Ready { api, token } => HostStatus::Ready(api.clone(), token.clone()),
                State::Failed(e) => HostStatus::Failed(e.clone()),
            },
            Err(_) => HostStatus::Failed("api host lock poisoned".to_string()),
        }
    }

    /// The API + token if the bootstrap has completed, else `None`.
    pub fn ready(&self) -> Option<(InProcessApi, Option<String>)> {
        match self.status() {
            HostStatus::Ready(api, token) => Some((api, token)),
            _ => None,
        }
    }
}

impl Default for ApiHost {
    fn default() -> Self {
        Self::new()
    }
}

/// Sign a dev `key_token` for the default workspace. The JWT secret comes from
/// the same `Config` the API uses, so the signature verifies in-process.
fn mint_dev_token(config: &ApiConfig) -> Option<String> {
    let token = lifeos_api::auth::issue_token(
        &config.jwt_secret,
        "workbench-local-user",
        lifeos_api::config::DEFAULT_WORKSPACE,
        "workbench@localhost",
    );
    (!token.is_empty()).then_some(token)
}
