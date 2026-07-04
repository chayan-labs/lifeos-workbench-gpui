//! Design tokens shared by every pane: spacing scale, an accent identity
//! distinct from the stock `gpui-component` default, and the one function
//! (`pane_bg`) every pane's root background routes through.
//!
//! `pane_bg` is what makes the macOS glass theme apply to the *whole app*
//! rather than one pane: it reads the process-wide [`GlassMode`] global (set
//! once at startup from `Config`) and returns a translucent fill when glass is
//! active, so retrofitting a pane for glass is "call `pane_bg(cx)` instead of
//! `cx.theme().background`" rather than bespoke per-pane work.

use gpui::{px, App, Global, Hsla, Pixels};
use gpui_component::ActiveTheme;

/// Whether the glass theme is active, published once at startup by
/// [`super::app::run`] so any view/element can read it without threading
/// `Config` through every constructor.
#[derive(Clone, Copy, Default)]
pub struct GlassMode(pub bool);

impl Global for GlassMode {}

/// True if glass mode has been installed and is on. Safe to call before the
/// global is set (defaults to off).
pub fn glass_active(cx: &App) -> bool {
    cx.try_global::<GlassMode>().is_some_and(|g| g.0)
}

/// Chrome surfaces (sidebar/tab-strip/statusbar/dock header) stay a touch more
/// opaque than content panes so text stays legible over a busier desktop.
const CHROME_GLASS_OPACITY: f32 = 0.78;
/// Content panes (editor, terminal, Life OS, agent, recall, settings, ...).
const CONTENT_GLASS_OPACITY: f32 = 0.62;

/// The background every pane's root container should use instead of a literal
/// `cx.theme().background`/`cx.theme().sidebar`. Opaque unless glass is active.
pub fn pane_bg(cx: &App) -> Hsla {
    let theme = cx.theme();
    if glass_active(cx) {
        theme.background.opacity(CONTENT_GLASS_OPACITY)
    } else {
        theme.background
    }
}

/// Same as [`pane_bg`] but for chrome (sidebar/tab-strip/statusbar), which
/// wants a slightly higher opacity than content panes.
pub fn chrome_bg(cx: &App) -> Hsla {
    let theme = cx.theme();
    if glass_active(cx) {
        theme.sidebar.opacity(CHROME_GLASS_OPACITY)
    } else {
        theme.sidebar
    }
}

/// A 1px top border used as a shared "glass edge" highlight. Subtle even when
/// glass is off, so it doubles as a general pane-separator touch.
pub fn glass_edge(cx: &App) -> Hsla {
    cx.theme().border.opacity(0.4)
}

// ------------------------------------------------------------------ spacing

pub const SPACE_1: Pixels = px(2.0);
pub const SPACE_2: Pixels = px(4.0);
pub const SPACE_3: Pixels = px(8.0);
pub const SPACE_4: Pixels = px(12.0);
pub const SPACE_5: Pixels = px(16.0);
pub const SPACE_6: Pixels = px(24.0);

/// An accent distinct from the stock theme default, giving the app a visual
/// identity for interactive affordances (buttons, active rail rows, focus
/// rings) instead of reading as an unmodified component demo.
pub fn accent(cx: &App) -> Hsla {
    Hsla {
        h: 210.0 / 360.0,
        s: 0.85,
        l: if cx.theme().is_dark() { 0.62 } else { 0.5 },
        a: 1.0,
    }
}
