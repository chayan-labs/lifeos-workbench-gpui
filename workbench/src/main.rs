//! Workbench entry point.
//!
//! Opens the GPU-native gpui window. The in-process `lifeos-api` state and the
//! tokio runtime are owned by the gpui frontend (`ui::app`); async surfaces
//! (Life OS API, PTYs, ACP, LSP) spawn onto that runtime off the render thread.
//!
//! (The legacy `--tui`, `--mcp`, and `--check` entrypoints live behind the
//! `legacy-tui` feature in the origin repo; they are re-added here as their
//! backing modules are ported into `ui/`.)

use lifeos_workbench::ui;

/// Finder launches the .app with cwd `/` and a bare environment; the DB paths
/// in `Config::from_env` are cwd-relative. Fall back to a per-user state dir +
/// $HOME cwd so double-clicking Workbench.app just works.
fn fix_finder_launch_env() {
    let cwd_is_root = std::env::current_dir()
        .map(|d| d == std::path::Path::new("/"))
        .unwrap_or(true);
    if !cwd_is_root {
        return;
    }
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let home = std::path::PathBuf::from(home);
    let _ = std::env::set_current_dir(&home);
    if std::env::var_os("LIFEOS_DB_PATH").is_none() {
        let state = home.join("Library/Application Support/LifeOS");
        if std::fs::create_dir_all(&state).is_ok() {
            std::env::set_var("LIFEOS_DB_PATH", state.join("lifeos.db"));
            if std::env::var_os("LIFEOS_DERIVED_DB_PATH").is_none() {
                std::env::set_var("LIFEOS_DERIVED_DB_PATH", state.join("lifeos-derived.db"));
            }
        }
    }
}

fn main() {
    fix_finder_launch_env();
    ui::run();
}
