//! Library surface of the Workbench.
//!
//! The GPU-native gpui frontend lives in [`ui`]. The in-process `lifeos-api`
//! handle ([`api`]) is renderer-agnostic and shared by both frontends.
//!
//! The legacy ratatui / glyph-grid renderer inherited from the origin repo is
//! preserved behind the off-by-default `legacy-tui` feature while its logic is
//! ported into `ui/`. Building without that feature (the default) compiles
//! only the gpui frontend and the shared crates - see CLAUDE.md build order.

// Shared, renderer-agnostic: the in-process API handle.
pub mod api;

// Renderer-agnostic logic reused by the gpui frontend (no ratatui/crossterm):
// the ACP client + its diff model, and the module-manifest parser. These were
// inherited from the origin repo but carry no TUI coupling, so the gpui panes
// link them directly rather than re-implementing them.
pub mod acp;
pub mod diff;
pub mod manifest;

// GPU-native gpui frontend (default).
pub mod ui;

// Legacy ratatui frontend (feature-gated). Ported into `ui/` incrementally.
#[cfg(feature = "legacy-tui")]
pub mod agent_pane;
#[cfg(feature = "legacy-tui")]
pub mod decorations;
#[cfg(feature = "legacy-tui")]
pub mod driver;
#[cfg(feature = "legacy-tui")]
pub mod editor;
#[cfg(feature = "legacy-tui")]
pub mod file_tree;
#[cfg(feature = "legacy-tui")]
pub mod gui;
#[cfg(feature = "legacy-tui")]
pub mod highlight;
#[cfg(feature = "legacy-tui")]
pub mod layout;
#[cfg(feature = "legacy-tui")]
pub mod lifeos_pane;
#[cfg(feature = "legacy-tui")]
pub mod lsp;
#[cfg(feature = "legacy-tui")]
pub mod markdown;
#[cfg(feature = "legacy-tui")]
pub mod mcp_server;
#[cfg(feature = "legacy-tui")]
pub mod mouse;
#[cfg(feature = "legacy-tui")]
pub mod palette;
#[cfg(feature = "legacy-tui")]
pub mod pane_store;
#[cfg(feature = "legacy-tui")]
pub mod search_pane;
#[cfg(feature = "legacy-tui")]
pub mod shell;
#[cfg(feature = "legacy-tui")]
pub mod term_pane;
#[cfg(feature = "legacy-tui")]
pub mod theme;
#[cfg(feature = "legacy-tui")]
pub mod views;
#[cfg(feature = "legacy-tui")]
pub mod workspace;
