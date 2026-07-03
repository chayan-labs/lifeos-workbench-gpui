//! `lifeos-api` - the single DB-token owner. A localhost-only Rust/axum service
//! that is the only process holding the canonical DB credential. Everything
//! (CLI, bot proxy, SPA) talks to the data plane through here, workspace-scoped.

use lifeos_api::{agents, build_state, config::Config, routes};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lifeos_api=info,tower_http=warn".into()),
        )
        .init();

    let config = Config::from_env();
    tracing::info!(db = %config.db_path, "opening canonical DB");
    let state = build_state(config.clone())
        .await
        .expect("failed to open/migrate the database");

    match agents::detect() {
        d if d.is_empty() => {
            tracing::warn!("no local agent CLIs detected on PATH - /api/llm will return 501")
        }
        d => tracing::info!(
            agents = %d.iter().map(|a| a.id.as_str()).collect::<Vec<_>>().join(", "),
            "detected local agent CLIs"
        ),
    }

    // Localhost-only API; permissive CORS so the Vite dev server can call it.
    // `allow_private_network` answers Chrome's Private Network Access preflight
    // (`Access-Control-Request-Private-Network`) - without it, browsers that
    // enforce PNA silently hang every non-simple (POST/PATCH/DELETE) request
    // from the dev server origin to this API.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_private_network(true);

    let app = routes::router(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {}: {e}", config.bind_addr));
    tracing::info!("Life OS local API listening on http://{}", config.bind_addr);
    axum::serve(listener, app).await.expect("server error");
}
