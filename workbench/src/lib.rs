//! Library surface of the Workbench so integration tests (and later panes)
//! can use the in-process API handle and shell components directly.

pub mod acp;
pub mod agent_pane;
pub mod api;
pub mod decorations;
pub mod diff;
pub mod driver;
pub mod editor;
pub mod file_tree;
pub mod gui;
pub mod highlight;
pub mod layout;
pub mod lifeos_pane;
pub mod lsp;
pub mod manifest;
pub mod markdown;
pub mod mcp_server;
pub mod mouse;
pub mod palette;
pub mod pane_store;
pub mod search_pane;
pub mod shell;
pub mod term_pane;
pub mod theme;
pub mod views;
pub mod workspace;
