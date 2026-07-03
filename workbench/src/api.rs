//! In-process linkage contract between the Workbench and `lifeos-api`.
//!
//! The Workbench links `lifeos-api` as a crate and calls it in one address
//! space: requests are dispatched straight into the axum `Router` as tower
//! services (`oneshot`), so there is no socket, no `localhost:8080`
//! round-trip, and no second process for the app itself. The retained
//! `127.0.0.1` HTTP server (spawned by `lifeos-api`'s own binary) exists
//! only for external consumers (Worker bot, Telegram lane, curl).
//!
//! Every upstream invariant holds because we reuse the exact same router and
//! `AppState`:
//! - single DB-token owner: exactly one `AppState` (one libSQL connection
//!   set) is built here, and this handle is the only way panes touch it;
//! - workspace scoping / auth: requests still pass through the same
//!   middleware and route handlers, so `workspace_id` scoping and JWT checks
//!   are enforced identically to the HTTP surface;
//! - append-only `events`: unchanged - same handlers, same SQL.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use lifeos_api::config::Config;
use lifeos_api::routes;
use serde_json::Value;
use tower::ServiceExt;

/// Handle every Workbench pane uses to talk to Life OS. Cheap to clone
/// (`Router` is an `Arc` bundle); all clones share the one `AppState`.
#[derive(Clone)]
pub struct InProcessApi {
    router: Router,
}

/// A parsed in-process response: HTTP status + JSON body (Null when the
/// body was empty or not JSON).
pub struct ApiResponse {
    pub status: StatusCode,
    pub body: Value,
}

impl ApiResponse {
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

impl InProcessApi {
    /// Build the single shared state from config and wrap the full router.
    /// This is the one place the Workbench constructs `AppState`.
    pub async fn new(config: Config) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let state = lifeos_api::build_state(config).await?;
        Ok(Self {
            router: routes::router(state),
        })
    }

    /// Dispatch a request into the router in-process. `token`, when present,
    /// is sent as the same `Authorization: Bearer` header the HTTP surface
    /// expects, so auth/workspace scoping is identical.
    pub async fn request(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        token: Option<&str>,
    ) -> ApiResponse {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        let request = match body {
            Some(b) => builder
                .header("content-type", "application/json")
                .body(Body::from(b.to_string()))
                .expect("valid in-process request"),
            None => builder
                .body(Body::empty())
                .expect("valid in-process request"),
        };
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router service is infallible");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_default();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        ApiResponse { status, body }
    }

    pub async fn get(&self, uri: &str, token: Option<&str>) -> ApiResponse {
        self.request("GET", uri, None, token).await
    }

    pub async fn post(&self, uri: &str, body: Value, token: Option<&str>) -> ApiResponse {
        self.request("POST", uri, Some(body), token).await
    }
}

// Re-export so callers of this module never need to depend on libsql
// directly just to name the error type.
pub use lifeos_api::config::Config as ApiConfig;
