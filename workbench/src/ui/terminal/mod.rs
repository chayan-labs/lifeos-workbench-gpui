//! Integrated terminal: a real shell rendered by a custom gpui element with a
//! visible cursor.
//!
//! - [`backend`] owns the pty + `alacritty_terminal` VTE and exposes a
//!   renderer-neutral snapshot (the de-ratatui'd port of the origin repo's
//!   `term_pane.rs`).
//! - [`ansi`] resolves ANSI colours to concrete RGB / theme-default.
//! - [`element`] paints the grid and the cursor.
//! - [`view`] is the focusable entity that routes input and drives repaint.

pub mod ansi;
pub mod backend;
pub mod element;
pub mod view;

pub use view::TerminalView;
