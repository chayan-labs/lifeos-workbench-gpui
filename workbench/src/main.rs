//! Workbench entry point: opens the in-process `lifeos-api` state (no
//! socket), then runs the app. The primary face is the standalone GPU
//! window (winit + wgpu glyph grid); `--tui` renders in the host terminal
//! instead (SSH/headless), `--mcp` serves the agent toolbelt over stdio,
//! and `--check` just proves the in-process linkage (used by CI/scripts).

use crossterm::event;
use lifeos_api::config::{Config, DEFAULT_WORKSPACE};
use lifeos_workbench::api::InProcessApi;
use lifeos_workbench::driver;
use lifeos_workbench::pane_store::PaneStore;
use lifeos_workbench::shell::Shell;
use lifeos_workbench::theme::{ColorSupport, Theme};
use std::time::Duration;

/// Finder launches the .app with cwd `/` and a bare environment; the DB
/// paths in `Config::from_env` are cwd-relative. Fall back to a per-user
/// state dir + $HOME cwd so double-clicking Workbench.app just works.
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
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("workbench: failed to start async runtime: {e}");
            std::process::exit(1);
        }
    };

    let config = Config::from_env();
    let api = runtime.block_on(async {
        let api = match InProcessApi::new(config).await {
            Ok(api) => api,
            Err(e) => {
                eprintln!("workbench: failed to open Life OS state: {e}");
                std::process::exit(1);
            }
        };
        let health = api.get("/api/health", None).await;
        if !health.is_success() {
            eprintln!(
                "workbench: in-process health check failed: {}",
                health.status
            );
            std::process::exit(1);
        }
        api
    });

    if std::env::args().any(|a| a == "--mcp") {
        // Toolbelt mode (issue #17): serve MCP over stdio for the ACP agent.
        if let Err(e) = runtime.block_on(lifeos_workbench::mcp_server::serve_stdio(api)) {
            eprintln!("workbench: mcp server error: {e}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|a| a == "--check") {
        println!(
            "workbench {}: lifeos-api linked in-process, health OK",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    // Panes spawn async work (PTYs, ACP, LSP); keep the runtime entered for
    // the lifetime of either frontend.
    let _guard = runtime.enter();
    if std::env::args().any(|a| a == "--tui") {
        if let Err(e) = run_tui(api) {
            eprintln!("workbench: shell error: {e}");
            std::process::exit(1);
        }
    } else if let Err(e) = lifeos_workbench::gui::run_gui(api, DEFAULT_WORKSPACE.to_string()) {
        eprintln!("workbench: window error: {e}");
        std::process::exit(1);
    }
}

fn run_tui(api: InProcessApi) -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), event::EnableMouseCapture);
    let theme = Theme::new(ColorSupport::detect());
    let mut shell = Shell::new(theme, DEFAULT_WORKSPACE.to_string());
    let mut panes = PaneStore::new(&std::env::current_dir().unwrap_or_default(), Some(api));
    let result = (|| -> std::io::Result<()> {
        while shell.running {
            for id in panes.reap_exited_terminals() {
                shell = shell.on_pane_exit(id);
            }
            let area = terminal.get_frame().area();
            panes.sync(&shell.pane_rects(area), &shell.desires);
            terminal.draw(|frame| shell.draw(frame, &mut panes))?;
            if event::poll(Duration::from_millis(50))? {
                let ev = event::read()?;
                shell = driver::dispatch(shell, &mut panes, &ev, area);
            }
        }
        Ok(())
    })();
    let _ = crossterm::execute!(std::io::stdout(), event::DisableMouseCapture);
    ratatui::restore();
    result
}
