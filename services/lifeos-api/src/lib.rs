//! `lifeos-api` library surface. The binary (`main.rs`) is a thin wrapper; the
//! whole app is here so integration tests can build the router in-process.

pub mod agents;
pub mod audit;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod ids;
pub mod models;
pub mod reconcile;
pub mod routes;
pub mod state;

use crate::config::Config;
use crate::state::AppState;
use std::sync::Arc;

/// Open the DB, detect agents, and assemble shared state from a config.
pub async fn build_state(config: Config) -> Result<AppState, libsql::Error> {
    let db = db::connect(&config).await?;
    let agents = agents::detect();
    Ok(AppState {
        conn: Arc::new(db.conn),
        database: Arc::new(db.database),
        config,
        agents: Arc::new(agents),
    })
}
