//! gpui application bootstrap.
//!
//! Owns the process entry: initialises `gpui-component`, opens the main
//! window, and installs the root [`WorkspaceView`] wrapped in a
//! `gpui_component::Root` (required for dialogs, sheets, notifications).
//!
//! A tokio runtime is created here and its handle published globally so the
//! async surfaces (in-process `lifeos-api`, PTY readers, ACP/LSP children)
//! can spawn work off the gpui render thread. Results are delivered back into
//! gpui views via channels / `cx.spawn` + `Entity::update`.

// Glob import brings the `AppContext` trait (providing `cx.new(..)`) and the
// other context traits into scope, matching gpui-component's examples.
use gpui::*;
use gpui_component::{Root, Theme, ThemeMode, TitleBar};
use std::sync::OnceLock;
use tokio::runtime::{Handle, Runtime};

use super::actions::{self, Quit};
use super::config::{self, Config, ThemePref};
use super::menu;
use super::workspace_view::WorkspaceView;

/// The process-wide tokio runtime backing every async surface. Kept alive for
/// the life of the app; its handle is what off-thread work spawns onto.
static TOKIO: OnceLock<Runtime> = OnceLock::new();

/// Handle to the shared tokio runtime. Panics if called before [`run`] has
/// initialised it (it never is, in practice - `run` sets it first thing).
pub fn tokio_handle() -> Handle {
    TOKIO
        .get()
        .expect("tokio runtime initialised by ui::app::run")
        .handle()
        .clone()
}

/// Launch the GPU-native Workbench window. Blocks until the window closes.
pub fn run() {
    // Stand up the async runtime before gpui takes over the main thread.
    let runtime = Runtime::new().expect("build tokio runtime");
    let _ = TOKIO.set(runtime);

    gpui_platform::application().run(move |cx| {
        // Must precede any gpui-component use.
        gpui_component::init(cx);

        // First launch: seed a config.lua from the user's VS Code / WezTerm
        // settings if we have not written one yet. Then resolve config (defaults
        // -> config.lua -> env) and apply the visual parts (theme mode + fonts).
        if let Some(dir) = config::config_dir() {
            super::import::run_first_launch_import(&dir);
        }
        apply_config(&Config::load(), cx);

        // Commands: keymap + app-global handlers + the native menu bar.
        actions::bind_keys(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        menu::install(cx);
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // Custom in-window title bar (hides the native one) so the menu +
            // tab strip live in the chrome, Zed-style.
            titlebar: Some(TitleBar::title_bar_options()),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| WorkspaceView::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("open main window");
    });
}

/// Apply the visual parts of the resolved config: theme mode, then any font
/// overrides on top of the mode's theme. Editor engine/options are consumed
/// separately by the editor view.
fn apply_config(config: &Config, cx: &mut App) {
    let mode = match config.theme {
        ThemePref::Dark => ThemeMode::Dark,
        ThemePref::Light => ThemeMode::Light,
    };
    Theme::change(mode, None, cx);

    let theme = Theme::global_mut(cx);
    if let Some(f) = &config.font.ui_family {
        theme.font_family = f.clone().into();
    }
    if let Some(f) = &config.font.mono_family {
        theme.mono_font_family = f.clone().into();
    }
    if let Some(s) = config.font.ui_size {
        theme.font_size = px(s);
    }
    if let Some(s) = config.font.mono_size {
        theme.mono_font_size = px(s);
    }
}
