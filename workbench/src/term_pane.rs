//! Embedded terminal pane: a real shell over `portable-pty`, parsed by the
//! `alacritty_terminal` VTE, rendered into a ratatui buffer. One lives in
//! the workspace's bottom dock; any center pane can become one on demand.

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{cell::Flags, Config as TermConfig, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Processor};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::io::{Read, Write};
use std::sync::Arc;

/// alacritty's event hook - we only need the terminal state machine, so
/// events (bells, clipboard, ...) are ignored for now.
#[derive(Clone)]
struct EventProxy;
impl EventListener for EventProxy {
    fn send_event(&self, _event: Event) {}
}

/// Grid dimensions handed to alacritty (it needs the trait, not numbers).
#[derive(Clone, Copy)]
struct GridSize {
    cols: usize,
    lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// One live terminal: pty child + VTE state. The reader thread feeds pty
/// output into the shared `Term`; rendering and input lock it briefly.
pub struct TermPane {
    term: Arc<FairMutex<Term<EventProxy>>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl TermPane {
    /// Spawn `cmd` (the user's shell by default) in a fresh pty sized
    /// `cols` x `rows`, inheriting the workbench cwd + env.
    pub fn spawn(cmd: Option<CommandBuilder>, cols: u16, rows: u16) -> std::io::Result<TermPane> {
        let pty = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(std::io::Error::other)?;
        let cmd = cmd.unwrap_or_else(default_shell);
        let child = pty
            .slave
            .spawn_command(cmd)
            .map_err(std::io::Error::other)?;
        drop(pty.slave);

        let size = GridSize {
            cols: cols as usize,
            lines: rows as usize,
        };
        let term = Arc::new(FairMutex::new(Term::new(
            TermConfig::default(),
            &size,
            EventProxy,
        )));
        let writer = pty.master.take_writer().map_err(std::io::Error::other)?;
        let mut reader = pty
            .master
            .try_clone_reader()
            .map_err(std::io::Error::other)?;

        let term_for_reader = term.clone();
        std::thread::spawn(move || {
            let mut processor: Processor = Processor::new();
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut term = term_for_reader.lock();
                for byte in &buf[..n] {
                    processor.advance(&mut *term, *byte);
                }
            }
        });

        Ok(TermPane {
            term,
            master: pty.master,
            writer,
            child,
        })
    }

    /// True once the child has exited (`exit`, crash, kill): the pane is a
    /// corpse and the store reaps it so the shell can close the pane.
    pub fn is_exited(&mut self) -> bool {
        self.child.try_wait().map(|s| s.is_some()).unwrap_or(true)
    }

    /// Resize both the pty (so the child relayouts) and the VTE grid.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.term.lock().resize(GridSize {
            cols: cols as usize,
            lines: rows as usize,
        });
    }

    /// Forward a key press to the child as the bytes a terminal would send.
    /// Typing snaps any scrollback view back to the live bottom.
    pub fn send_key(&mut self, key: &KeyEvent) {
        let app_cursor = {
            let mut term = self.term.lock();
            term.scroll_display(Scroll::Bottom);
            term.mode().contains(TermMode::APP_CURSOR)
        };
        if let Some(bytes) = encode_key(key, app_cursor) {
            let _ = self.writer.write_all(&bytes);
            let _ = self.writer.flush();
        }
    }

    /// Mouse wheel: scroll the scrollback on the primary screen; full-screen
    /// apps (alternate screen) get arrow keys instead, the terminal idiom.
    pub fn on_scroll(&mut self, down: bool) {
        let on_alt_screen = self.term.lock().mode().contains(TermMode::ALT_SCREEN);
        if on_alt_screen {
            let code = if down { KeyCode::Down } else { KeyCode::Up };
            for _ in 0..3 {
                self.send_key(&KeyEvent::new(code, KeyModifiers::NONE));
            }
        } else {
            let delta = if down { -3 } else { 3 };
            self.term.lock().scroll_display(Scroll::Delta(delta));
        }
    }

    /// Snapshot the visible grid as styled lines for ratatui.
    pub fn render_lines(&self) -> Vec<Line<'static>> {
        let term = self.term.lock();
        let grid = term.grid();
        let cols = grid.columns();
        let mut lines = Vec::with_capacity(grid.screen_lines());
        let mut current: Vec<Span> = Vec::with_capacity(cols);
        let mut last_line = 0i32;
        for indexed in grid.display_iter() {
            let line = indexed.point.line.0;
            if line != last_line {
                lines.push(Line::from(std::mem::take(&mut current)));
                last_line = line;
            }
            let cell = &indexed.cell;
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            current.push(Span::styled(cell.c.to_string(), cell_style(cell)));
        }
        lines.push(Line::from(current));
        lines
    }
}

fn default_shell() -> CommandBuilder {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut cmd = CommandBuilder::new(shell);
    if let Ok(cwd) = std::env::current_dir() {
        cmd.cwd(cwd);
    }
    cmd
}

fn cell_style(cell: &alacritty_terminal::term::cell::Cell) -> Style {
    let mut style = Style::default();
    if let Some(fg) = convert_color(cell.fg) {
        style = style.fg(fg);
    }
    if let Some(bg) = convert_color(cell.bg) {
        // NamedColor::Background stays as the pane's own background.
        if cell.bg != AnsiColor::Named(NamedColor::Background) {
            style = style.bg(bg);
        }
    }
    if cell.flags.contains(Flags::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.flags.contains(Flags::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.flags.contains(Flags::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.flags.contains(Flags::INVERSE) {
        style = style.add_modifier(Modifier::REVERSED);
    }
    if cell.flags.contains(Flags::DIM) {
        style = style.add_modifier(Modifier::DIM);
    }
    style
}

fn convert_color(color: AnsiColor) -> Option<Color> {
    match color {
        AnsiColor::Spec(rgb) => Some(Color::Rgb(rgb.r, rgb.g, rgb.b)),
        AnsiColor::Indexed(i) => Some(Color::Indexed(i)),
        AnsiColor::Named(named) => named_color(named),
    }
}

fn named_color(named: NamedColor) -> Option<Color> {
    use NamedColor::*;
    Some(match named {
        Black => Color::Black,
        Red => Color::Red,
        Green => Color::Green,
        Yellow => Color::Yellow,
        Blue => Color::Blue,
        Magenta => Color::Magenta,
        Cyan => Color::Cyan,
        White => Color::Gray,
        BrightBlack => Color::DarkGray,
        BrightRed => Color::LightRed,
        BrightGreen => Color::LightGreen,
        BrightYellow => Color::LightYellow,
        BrightBlue => Color::LightBlue,
        BrightMagenta => Color::LightMagenta,
        BrightCyan => Color::LightCyan,
        BrightWhite | BrightForeground => Color::White,
        Foreground => return None,
        Background => return None,
        _ => return None,
    })
}

/// Translate a crossterm key press into the byte sequence a VT sends.
/// Covers the daily-driver set: text, control chars, arrows (normal + app
/// cursor mode), nav keys, and Alt-prefixed input.
pub fn encode_key(key: &KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    let mods = key.modifiers;
    // Cmd chords belong to the app/menu layer, never to the child shell.
    if mods.contains(KeyModifiers::SUPER) {
        return None;
    }
    let mut bytes: Vec<u8> = match key.code {
        KeyCode::Char(c) if mods.contains(KeyModifiers::CONTROL) => {
            let upper = c.to_ascii_uppercase();
            if upper.is_ascii_uppercase() {
                vec![(upper as u8) & 0x1f]
            } else {
                return None;
            }
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => arrow(b'A', app_cursor),
        KeyCode::Down => arrow(b'B', app_cursor),
        KeyCode::Right => arrow(b'C', app_cursor),
        KeyCode::Left => arrow(b'D', app_cursor),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        _ => return None,
    };
    if mods.contains(KeyModifiers::ALT) {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

fn arrow(dir: u8, app_cursor: bool) -> Vec<u8> {
    if app_cursor {
        vec![0x1b, b'O', dir]
    } else {
        vec![0x1b, b'[', dir]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn grid_text(pane: &TermPane) -> String {
        pane.render_lines()
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn wait_for(pane: &TermPane, needle: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if grid_text(pane).contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn shell_runs_in_a_pane_and_renders_output() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "printf 'workbench-pty-ok'; sleep 5"]);
        let pane = TermPane::spawn(Some(cmd), 80, 24).expect("spawn pty");
        assert!(
            wait_for(&pane, "workbench-pty-ok"),
            "pty output not rendered: {}",
            grid_text(&pane)
        );
    }

    #[test]
    fn input_reaches_the_child_shell() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "read line; printf 'got:%s' \"$line\"; sleep 5"]);
        let mut pane = TermPane::spawn(Some(cmd), 80, 24).expect("spawn pty");
        for c in "hello".chars() {
            pane.send_key(&KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        pane.send_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            wait_for(&pane, "got:hello"),
            "child never echoed input: {}",
            grid_text(&pane)
        );
    }

    #[test]
    fn resize_updates_the_grid() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "sleep 5"]);
        let mut pane = TermPane::spawn(Some(cmd), 80, 24).expect("spawn pty");
        pane.resize(40, 10);
        assert_eq!(pane.render_lines().len(), 10);
    }

    #[test]
    fn wheel_scrolls_scrollback_and_typing_snaps_back() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args([
            "-c",
            "i=0; while [ $i -lt 40 ]; do echo line$i; i=$((i+1)); done; sleep 5",
        ]);
        let mut pane = TermPane::spawn(Some(cmd), 80, 10).expect("spawn pty");
        assert!(wait_for(&pane, "line39"), "output: {}", grid_text(&pane));
        pane.on_scroll(false); // scroll up into history
        assert!(
            !grid_text(&pane).contains("line39"),
            "scrolled view must leave the bottom"
        );
        pane.send_key(&KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(
            grid_text(&pane).contains("line39"),
            "typing snaps back to the live bottom"
        );
    }

    #[test]
    fn exited_child_is_detected() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "exit 0"]);
        let mut pane = TermPane::spawn(Some(cmd), 80, 24).expect("spawn pty");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !pane.is_exited() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(pane.is_exited(), "exit must be observable via try_wait");
    }

    #[test]
    fn super_chords_never_reach_the_child() {
        assert_eq!(
            encode_key(
                &KeyEvent::new(KeyCode::Char('k'), KeyModifiers::SUPER),
                false
            ),
            None
        );
    }

    #[test]
    fn key_encoding_covers_control_arrows_and_alt() {
        assert_eq!(
            encode_key(
                &KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                false
            ),
            Some(vec![0x03])
        );
        assert_eq!(
            encode_key(&KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), false),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            encode_key(&KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), true),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            encode_key(&KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT), false),
            Some(vec![0x1b, b'f'])
        );
    }
}
