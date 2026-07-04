//! Standalone window host: a winit window painted by a wgpu glyph-grid
//! renderer (`ratatui-wgpu`) implementing ratatui's `Backend`, so the whole
//! shell - panes, editor, terminals, agent, Life OS views - runs unmodified.
//! This is the primary face of the app; `--tui` keeps the crossterm path.
//!
//! Window-mode polish (issues #27/#28): real font variants + CJK/symbol
//! fallbacks, pixel-exact centered blit with theme-colored padding, live
//! scale-factor handling, native menu bar, Cmd+V paste, IME commit, and
//! drag-and-drop opening files in the editor.

pub mod fonts;
pub mod input;
pub mod mouse;
pub mod postprocess;

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use muda::accelerator::{Accelerator, Code, CMD_OR_CTRL};
use muda::{AboutMetadata, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use ratatui::backend::Backend;
use ratatui::Terminal;
use ratatui_wgpu::{Builder, Dimensions, Fonts, Viewport, WgpuBackend};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState};
use winit::window::{Window, WindowAttributes};

/// The uncomposed key for chord matching. macOS composes Option into the
/// logical key; other platforms deliver the base key already.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn key_without_modifiers(key: &winit::event::KeyEvent) -> Key {
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    key.key_without_modifiers()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn key_without_modifiers(key: &winit::event::KeyEvent) -> Key {
    key.logical_key.clone()
}

use crate::api::InProcessApi;
use crate::driver;
use crate::palette::CommandId;
use crate::pane_store::PaneStore;
use crate::shell::Shell;
use crate::theme::{ColorSupport, Theme, BG};
use postprocess::CenteredPostProcessor;
use std::collections::HashMap;

/// Frame cadence while idle: terminal panes produce output asynchronously,
/// so redraw on a short tick (mirrors the 50ms poll of the TUI loop).
const IDLE_FRAME: Duration = Duration::from_millis(50);
const DEFAULT_FONT_PT: f64 = 13.0;
/// Minimum window padding reserved around the grid, in logical px per side.
const PADDING_LOGICAL: f64 = 8.0;

type GpuTerminal = Terminal<WgpuBackend<'static, 'static, CenteredPostProcessor>>;

pub fn run_gui(api: InProcessApi, workspace: String) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + IDLE_FRAME));
    let cwd = std::env::current_dir().unwrap_or_default();
    let fonts = fonts::load_fonts()?;
    let (menu, menu_actions) = build_menu_bar();
    let mut app = GuiApp {
        window: None,
        terminal: None,
        shell: Some(Shell::new(
            Theme::new(ColorSupport::TrueColor),
            workspace.clone(),
        )),
        panes: PaneStore::new(&cwd, Some(api)),
        modifiers: ModifiersState::empty(),
        mouse: mouse::MouseTracker::default(),
        fonts,
        title: window_title(&workspace, &cwd),
        warmup_frames: 2,
        menu_actions,
        _menu: menu,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

fn window_title(workspace: &str, cwd: &std::path::Path) -> String {
    let dir = cwd
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.display().to_string());
    format!("{workspace} - {dir}")
}

/// A wired menu entry: its click (or Cmd accelerator) runs a shell command.
fn menu_item(
    actions: &mut HashMap<MenuId, CommandId>,
    title: &str,
    cmd: CommandId,
    accel: Option<Accelerator>,
) -> MenuItem {
    let item = MenuItem::new(title, true, accel);
    actions.insert(item.id().clone(), cmd);
    item
}

/// Native macOS menu bar (muda), every item wired to a `CommandId` (drained
/// in `about_to_wait`). Predefined items handle About/Hide/Quit through the
/// standard responder chain, including the Cmd+Q accelerator.
fn build_menu_bar() -> (Option<Menu>, HashMap<MenuId, CommandId>) {
    let mut actions = HashMap::new();
    let cmd = |code| Some(Accelerator::new(Some(CMD_OR_CTRL), code));
    let menu = Menu::new();

    let about = AboutMetadata {
        name: Some("Life OS Workbench".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        ..Default::default()
    };
    let app_menu = Submenu::new("Workbench", true);
    let app_items: [&dyn muda::IsMenuItem; 5] = [
        &PredefinedMenuItem::about(None, Some(about)),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::quit(None),
    ];

    let file = Submenu::new("File", true);
    let file_items: [&dyn muda::IsMenuItem; 5] = [
        &menu_item(&mut actions, "New Tab", CommandId::NewTab, cmd(Code::KeyT)),
        &menu_item(
            &mut actions,
            "Open File…",
            CommandId::OpenFilePicker,
            cmd(Code::KeyO),
        ),
        &menu_item(
            &mut actions,
            "New Terminal Here",
            CommandId::TerminalHere,
            None,
        ),
        &PredefinedMenuItem::separator(),
        &menu_item(
            &mut actions,
            "Close Pane",
            CommandId::ClosePane,
            cmd(Code::KeyW),
        ),
    ];

    let view = Submenu::new("View", true);
    let view_items: [&dyn muda::IsMenuItem; 7] = [
        &menu_item(
            &mut actions,
            "Command Palette",
            CommandId::OpenPalette,
            cmd(Code::KeyK),
        ),
        &PredefinedMenuItem::separator(),
        &menu_item(
            &mut actions,
            "Files Sidebar",
            CommandId::ToggleSidebar,
            cmd(Code::KeyB),
        ),
        &menu_item(
            &mut actions,
            "Terminal Dock",
            CommandId::ToggleDock,
            cmd(Code::KeyJ),
        ),
        &PredefinedMenuItem::separator(),
        &menu_item(
            &mut actions,
            "Split Right",
            CommandId::SplitHorizontal,
            cmd(Code::KeyD),
        ),
        &menu_item(
            &mut actions,
            "Split Down",
            CommandId::SplitVertical,
            Some(Accelerator::new(
                Some(CMD_OR_CTRL | muda::accelerator::Modifiers::SHIFT),
                Code::KeyD,
            )),
        ),
    ];

    let lifeos = Submenu::new("Life OS", true);
    let lifeos_items: [&dyn muda::IsMenuItem; 3] = [
        &menu_item(
            &mut actions,
            "Browse Modules",
            CommandId::OpenLifeOsPane,
            cmd(Code::KeyL),
        ),
        &menu_item(&mut actions, "Agent Pane", CommandId::OpenAgentPane, None),
        &menu_item(
            &mut actions,
            "Recall Search",
            CommandId::OpenSearchPane,
            None,
        ),
    ];

    let built = (|| {
        app_menu.append_items(&app_items).ok()?;
        file.append_items(&file_items).ok()?;
        view.append_items(&view_items).ok()?;
        lifeos.append_items(&lifeos_items).ok()?;
        menu.append_items(&[&app_menu, &file, &view, &lifeos]).ok()
    })();
    if built.is_none() {
        return (None, HashMap::new());
    }
    #[cfg(target_os = "macos")]
    menu.init_for_nsapp();
    (Some(menu), actions)
}

struct GuiApp {
    window: Option<Arc<Window>>,
    terminal: Option<GpuTerminal>,
    /// Owned by `Option` so dispatch can move the immutable shell through
    /// `Shell::on_event` and put the successor back.
    shell: Option<Shell>,
    panes: PaneStore,
    modifiers: ModifiersState,
    mouse: mouse::MouseTracker,
    fonts: fonts::FontSet,
    title: String,
    /// Full-repaint countdown for the first frames (cold glyph atlas).
    warmup_frames: u8,
    /// Menu item → shell command; drained from muda's event channel.
    menu_actions: HashMap<MenuId, CommandId>,
    _menu: Option<Menu>,
}

impl GuiApp {
    fn font_size_px(scale: f64) -> u32 {
        let pt = std::env::var("WORKBENCH_FONT_SIZE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(DEFAULT_FONT_PT);
        // pt → CSS px (4/3) → device px.
        ((pt * 4.0 / 3.0) * scale).round().max(8.0) as u32
    }

    fn padding_px(scale: f64) -> u32 {
        (PADDING_LOGICAL * scale).round() as u32
    }

    fn build_fonts(&self, scale: f64) -> Fonts<'static> {
        let mut f = Fonts::new(self.fonts.regular.clone(), Self::font_size_px(scale));
        f.add_regular_fonts(self.fonts.fallbacks.iter().cloned());
        f.add_bold_fonts(self.fonts.bold.iter().cloned());
        f.add_italic_fonts(self.fonts.italic.iter().cloned());
        f.add_bold_italic_fonts(self.fonts.bold_italic.iter().cloned());
        f
    }

    fn redraw(&mut self) {
        // Shells that exited (`exit`, crash) close their pane before layout.
        let dead = self.panes.reap_exited_terminals();
        if !dead.is_empty() {
            if let Some(shell) = self.shell.take() {
                self.shell = Some(dead.iter().fold(shell, |s, id| s.on_pane_exit(*id)));
            }
        }
        let (Some(terminal), Some(shell)) = (self.terminal.as_mut(), self.shell.as_ref()) else {
            return;
        };
        // Warmup repaints: the first full rasterization drops glyphs while
        // the atlas is cold (ratatui-wgpu 0.4.1, tracked in issue #54), and
        // cells that never change again would keep the holes forever. Clear
        // for the first two frames so frame two re-renders everything
        // against the warm atlas.
        if self.warmup_frames > 0 {
            self.warmup_frames -= 1;
            let _ = terminal.clear();
        }
        let area = terminal.get_frame().area();
        self.panes.sync(&shell.pane_rects(area), &shell.desires);
        let panes = &mut self.panes;
        let _ = terminal.draw(|frame| shell.draw(frame, panes));
    }

    /// The grid geometry needed for pixel→cell mapping: (cols, rows,
    /// text-region px width, height) from the live backend.
    fn grid_metrics(&mut self) -> Option<(u16, u16, u16, u16)> {
        let ws = self.terminal.as_mut()?.backend_mut().window_size().ok()?;
        Some((
            ws.columns_rows.width,
            ws.columns_rows.height,
            ws.pixels.width,
            ws.pixels.height,
        ))
    }

    fn cell_at(&mut self, px: f64, py: f64) -> Option<(u16, u16)> {
        let size = self.window.as_ref()?.inner_size();
        let (cols, rows, rw, rh) = self.grid_metrics()?;
        Some(mouse::cell_at(
            px,
            py,
            size.width,
            size.height,
            cols,
            rows,
            rw,
            rh,
        ))
    }

    fn handle_event(&mut self, ev: &crossterm::event::Event, event_loop: &ActiveEventLoop) {
        if let Some(shell) = self.shell.take() {
            let area = self
                .terminal
                .as_mut()
                .map(|t| t.get_frame().area())
                .unwrap_or_default();
            let next = driver::dispatch(shell, &mut self.panes, ev, area);
            let running = next.running;
            self.shell = Some(next);
            if !running {
                event_loop.exit();
            }
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn inject_text(&mut self, text: &str, event_loop: &ActiveEventLoop) {
        for ev in input::text_to_events(text) {
            self.handle_event(&ev, event_loop);
        }
    }

    fn paste_clipboard(&mut self, event_loop: &ActiveEventLoop) {
        let text = arboard::Clipboard::new().and_then(|mut c| c.get_text());
        if let Ok(text) = text {
            self.inject_text(&text, event_loop);
        }
    }

    fn open_dropped_file(&mut self, path: std::path::PathBuf) {
        if !path.is_file() {
            return;
        }
        if let Some(shell) = self.shell.take() {
            self.shell = Some(shell.open_in_focused(path));
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for GuiApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = WindowAttributes::default()
            .with_title(self.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 860.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("workbench: failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };
        window.set_ime_allowed(true);

        let scale = window.scale_factor();
        let size = window.inner_size();
        let pad = Self::padding_px(scale);
        let bg = [
            BG.rgb().0 as f32 / 255.0,
            BG.rgb().1 as f32 / 255.0,
            BG.rgb().2 as f32 / 255.0,
            1.0,
        ];
        let backend = pollster::block_on(
            Builder::<CenteredPostProcessor>::from_font_and_user_data(
                self.fonts.regular.clone(),
                bg,
            )
            .with_regular_fonts(self.fonts.fallbacks.iter().cloned())
            .with_bold_fonts(self.fonts.bold.iter().cloned())
            .with_italic_fonts(self.fonts.italic.iter().cloned())
            .with_bold_italic_fonts(self.fonts.bold_italic.iter().cloned())
            .with_font_size_px(Self::font_size_px(scale))
            .with_fg_color(crate::theme::FG.resolve(ColorSupport::TrueColor))
            .with_bg_color(BG.resolve(ColorSupport::TrueColor))
            .with_viewport(Viewport::Shrink {
                width: pad * 2,
                height: pad * 2,
            })
            .with_width_and_height(Dimensions {
                width: NonZeroU32::new(size.width.max(1)).unwrap(),
                height: NonZeroU32::new(size.height.max(1)).unwrap(),
            })
            .build_with_target(window.clone()),
        );
        match backend
            .map_err(|e| e.to_string())
            .and_then(|b| Terminal::new(b).map_err(|e| e.to_string()))
        {
            Ok(t) => self.terminal = Some(t),
            Err(e) => {
                eprintln!("workbench: failed to initialize GPU renderer: {e}");
                event_loop.exit();
                return;
            }
        }
        // No eager draw here: the surface has never presented yet, and cells
        // drawn now would be diffed away. The first RedrawRequested paints.
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(mods) => self.modifiers = mods.state(),
            WindowEvent::Resized(size) => {
                if let Some(terminal) = self.terminal.as_mut() {
                    terminal
                        .backend_mut()
                        .resize(size.width.max(1), size.height.max(1));
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let fonts = self.build_fonts(scale_factor);
                if let Some(terminal) = self.terminal.as_mut() {
                    terminal.backend_mut().update_fonts(fonts);
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => self.inject_text(&text, event_loop),
            WindowEvent::DroppedFile(path) => self.open_dropped_file(path),
            WindowEvent::CursorMoved { position, .. } => {
                let mods = input::to_modifiers(self.modifiers);
                if let Some(cell) = self.cell_at(position.x, position.y) {
                    if let Some(ev) = self.mouse.moved(cell, mods) {
                        self.handle_event(&ev, event_loop);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let mods = input::to_modifiers(self.modifiers);
                let pressed = state == ElementState::Pressed;
                if let Some(ev) = self.mouse.input(button, pressed, mods) {
                    self.handle_event(&ev, event_loop);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let mods = input::to_modifiers(self.modifiers);
                let lines = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y as f64,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        // Convert trackpad pixels to lines via the cell height.
                        let cell_h = self
                            .grid_metrics()
                            .map(|(_, rows, _, rh)| rh.max(1) as f64 / rows.max(1) as f64)
                            .unwrap_or(16.0);
                        pos.y / cell_h
                    }
                };
                for ev in self.mouse.wheel(lines, mods) {
                    self.handle_event(&ev, event_loop);
                }
            }
            WindowEvent::KeyboardInput { event: key, .. } => {
                if key.state != ElementState::Pressed {
                    return;
                }
                // Chords need the base key: macOS Option composes characters
                // (alt+f arrives as "ƒ", alt+e as a dead key), so strip the
                // composition whenever a command modifier is held.
                let logical = if self.modifiers.alt_key()
                    || self.modifiers.control_key()
                    || self.modifiers.super_key()
                {
                    key_without_modifiers(&key)
                } else {
                    key.logical_key.clone()
                };
                if self.modifiers.super_key() {
                    if let Key::Character(s) = &logical {
                        if s.as_str().eq_ignore_ascii_case("v") {
                            self.paste_clipboard(event_loop);
                            return;
                        }
                    }
                }
                if let Some(ev) = input::translate_key(&logical, self.modifiers) {
                    self.handle_event(&ev, event_loop);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Menu clicks and Cmd accelerators arrive on muda's channel.
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if let Some(cmd) = self.menu_actions.get(ev.id()).copied() {
                if let Some(shell) = self.shell.take() {
                    let next = shell.run_command(cmd);
                    let running = next.running;
                    self.shell = Some(next);
                    if !running {
                        event_loop.exit();
                    }
                }
            }
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + IDLE_FRAME));
    }
}
