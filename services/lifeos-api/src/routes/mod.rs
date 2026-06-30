//! HTTP surface for `lifeos-api`. One handler module per resource group.

mod entity;
mod event;
mod health;
mod job;
mod llm;
mod metrics;
mod module_request;
mod planned;
mod register;
mod edge;
mod search;

use crate::state::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        // --- liveness + identity ---
        .route("/api/health", get(health::health))
        .route("/api/register", post(register::register))
        // --- generic entity CRUD (the spine the whole system rests on) ---
        .route("/api/entity", post(entity::create).get(entity::list))
        .route("/api/entity/:id", get(entity::get_one).patch(entity::update))
        // --- graph edges ---
        .route("/api/edge", post(edge::create).get(edge::list))
        .route("/api/edge/:id", patch(edge::update))
        // --- events: append-only. Only POST (append) + GET (read) are wired;
        //     PUT/PATCH/DELETE resolve to 405 because no route defines them. ---
        .route("/api/event", post(event::create).get(event::list))
        // --- job queue (read for the UI, enqueue for producers) ---
        .route("/api/jobs", get(job::list))
        .route("/api/job", post(job::create))
        // --- hybrid recall: FTS5 (+ best-effort vectors) over the derived DB ---
        .route("/api/search", get(search::search))
        // --- dashboards: pure SQL aggregation over events ---
        .route("/api/metrics", get(metrics::metrics))
        // --- self-extension intake ---
        .route("/api/module-request", post(module_request::create))
        // --- local agent router (OpenDesign-style) ---
        .route("/api/agents", get(llm::agents))
        .route("/api/llm", post(llm::llm))
        // --- planned routes: enqueue where it makes sense, honest 501 otherwise ---
        .route("/api/ingest", post(planned::ingest))
        .route("/api/pipeline/run", post(planned::pipeline_run))
        .route("/api/vcs/history", get(planned::not_implemented))
        .route("/api/vcs/commit", post(planned::not_implemented))
        .route("/api/broker/positions", get(planned::not_implemented))
        .with_state(state)
}
