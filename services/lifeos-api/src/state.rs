//! Shared, cheaply-cloneable application state handed to every handler.

use crate::agents::DetectedAgent;
use crate::browser::BrowserActuator;
use crate::config::Config;
use crate::kite::KiteClient;
use crate::nango::NangoClient;
use crate::reading::ArticleFetcher;
use crate::whatsapp::WhatsAppClient;
use libsql::{Connection, Database};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub conn: Arc<Connection>,
    /// Retained so the embedded-replica's background replicator stays alive for the
    /// process lifetime and so periodic `database.sync()` can be triggered. Held even
    /// in local-only mode (where it is an inert local handle).
    pub database: Arc<Database>,
    pub config: Config,
    /// Agent CLIs detected on PATH at boot (the `/api/llm` engines).
    pub agents: Arc<Vec<DetectedAgent>>,
    /// Self-hosted Nango client. `None` until infra/nango/ is deployed and a
    /// secret key is configured - connection routes surface NotImplemented
    /// rather than pretending to work (docs/MANUAL-SETUP.md #47-55).
    pub nango: Option<Arc<dyn NangoClient>>,
    /// Kite Connect client. `None` until a Kite app + encryption key are
    /// configured (docs/MANUAL-SETUP.md #51) - `/api/connections/kite/*` and
    /// `/api/broker/positions` surface NotImplemented rather than pretending.
    /// Deliberately read-only: see `kite::KiteClient`.
    pub kite: Option<Arc<dyn KiteClient>>,
    /// Self-hosted GOWA client. `None` until infra/gowa/ is deployed
    /// (docs/MANUAL-SETUP.md #52) - `/api/connections/whatsapp/*` and
    /// `/api/webhooks/whatsapp` surface NotImplemented rather than
    /// pretending. Deliberately has no send capability: see `whatsapp::WhatsAppClient`.
    pub whatsapp: Option<Arc<dyn WhatsAppClient>>,
    /// Browser actuator process wrapper. `None` until
    /// `BROWSER_ACTUATOR_SCRIPT` is configured (docs/MANUAL-SETUP.md #54) -
    /// `/api/browser/*` and `/api/connections/browser/session` surface
    /// NotImplemented rather than pretending. Deliberately has no method for
    /// arbitrary state-changing actions: see `browser::BrowserActuator`.
    pub browser: Option<Arc<dyn BrowserActuator>>,
    /// Article fetcher for the Reading module. Unlike the clients above,
    /// this needs no owned credentials, so it is always `Some` in
    /// production (`None` only appears in tests that want `/api/reading/*`
    /// to surface NotImplemented).
    pub reading: Option<Arc<dyn ArticleFetcher>>,
    /// lifeos-vcs CAS object store (issue #81/#86). Needs no credentials, so
    /// unlike the connectors above this is always present.
    pub vcs_store: Arc<lifeos_vcs::ObjectStore>,
}

impl AppState {
    /// The default agent id (first detected in preference order), if any.
    pub fn default_agent(&self) -> Option<String> {
        self.agents.first().map(|a| a.id.clone())
    }
}
