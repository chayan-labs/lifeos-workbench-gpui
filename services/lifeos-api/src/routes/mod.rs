//! HTTP surface for `lifeos-api`. One handler module per resource group.

mod browser;
mod calendar;
mod connection;
mod drive;
mod entity;
mod event;
mod gmail;
mod health;
mod job;
mod kite;
mod llm;
mod metrics;
mod module_request;
mod notion;
mod planned;
mod register;
mod edge;
mod search;
mod slack;
mod stream;
mod whatsapp;
mod workspace;

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
        .route("/api/me", get(workspace::me))
        .route("/api/workspace", get(workspace::get_workspace).patch(workspace::update_workspace))
        // --- generic entity CRUD (the spine the whole system rests on) ---
        .route("/api/entity", post(entity::create).get(entity::list))
        .route("/api/entity/:id", get(entity::get_one).patch(entity::update))
        // --- repair a forced sync conflict by replaying events (docs/DATA-MODEL.md §4.2) ---
        .route("/api/entity/:id/reconcile", post(entity::reconcile))
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
        // --- owned-credential connect/disconnect (issue #47) ---
        .route("/api/connections", get(connection::list))
        .route("/api/connections/session", post(connection::start_session))
        .route("/api/connections/complete", post(connection::complete))
        .route("/api/connections/:id", axum::routing::delete(connection::disconnect))
        // --- Kite Connect: native custom connector, read-only (issue #51) ---
        .route("/api/connections/kite/login-url", get(kite::login_url_handler))
        .route("/api/connections/kite/complete", post(kite::complete))
        // --- WhatsApp via self-hosted GOWA: QR pairing, no send route (issue #52) ---
        .route("/api/connections/whatsapp/session", post(whatsapp::start_session))
        .route("/api/connections/whatsapp/qr", get(whatsapp::qr))
        .route("/api/connections/whatsapp/status", get(whatsapp::status))
        .route("/api/webhooks/whatsapp", post(whatsapp::webhook))
        .route("/api/whatsapp/send", post(whatsapp::send))
        // --- per-provider Nango proxy thin tools (issue #53): reads proxy
        //     straight through, writes only ever draft (docs/SECURITY.md §2) ---
        .route("/api/gmail/list", get(gmail::list))
        .route("/api/gmail/send", post(gmail::send))
        .route("/api/calendar/list", get(calendar::list))
        .route("/api/calendar/create", post(calendar::create))
        .route("/api/drive/list", get(drive::list))
        .route("/api/drive/upload", post(drive::upload))
        .route("/api/notion/list", get(notion::list))
        .route("/api/notion/create", post(notion::create))
        .route("/api/slack/list", get(slack::list))
        .route("/api/slack/post", post(slack::post))
        // --- browser actuator: free read-only scrape, gated act, one
        //     interactive session-capture route (issue #54) ---
        .route("/api/browser/scrape", post(browser::scrape))
        .route("/api/browser/act", post(browser::act))
        .route("/api/connections/browser/session", post(browser::session))
        // --- SSE: module lifecycle events for hot-reload tabs (no polling) ---
        .route("/api/stream/modules", get(stream::modules))
        // --- local agent router (OpenDesign-style) ---
        .route("/api/agents", get(llm::agents))
        .route("/api/llm", post(llm::llm))
        // --- planned routes: enqueue where it makes sense, honest 501 otherwise ---
        .route("/api/ingest", post(planned::ingest))
        .route("/api/pipeline/run", post(planned::pipeline_run))
        .route("/api/vcs/history", get(planned::not_implemented))
        .route("/api/vcs/commit", post(planned::not_implemented))
        // --- read-only broker positions proxy (issue #51) - no order route exists ---
        .route("/api/broker/positions", get(kite::positions))
        .with_state(state)
}
