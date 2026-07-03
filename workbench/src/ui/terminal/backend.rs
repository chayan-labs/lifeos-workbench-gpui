//! Terminal backend: a real shell over `portable-pty`, parsed by the
//! `alacritty_terminal` VTE. This is the de-ratatui'd port of the origin
//! repo's `term_pane.rs`; instead of emitting ratatui `Line`s it exposes a
//! renderer-neutral [`TermSnapshot`] (cells + cursor) that the gpui element
//! paints, and it takes input as a gpui-free [`KeyInput`] so the encoding
//! stays unit-testable.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config as TermConfig, Term, TermMode};
use alacritty_terminal::vte::ansi::{CursorShape, Processor};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use super::ansi::{resolve, CellColor};

/// alacritty's event hook. We drive damage tracking from the reader thread
/// (which owns the byte stream) rather than these events, so we ignore them.
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

/// One rendered grid cell, colours already resolved to [`CellColor`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TermCell {
    pub c: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub dim: bool,
}

impl Default for TermCell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: CellColor::Default,
            bg: CellColor::Default,
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
            dim: false,
        }
    }
}

/// An immutable snapshot of the visible grid + cursor, produced under a brief
/// lock and then rendered without holding it.
pub struct TermSnapshot {
    pub cols: usize,
    pub rows: usize,
    /// Row-major, `rows * cols` cells.
    pub cells: Vec<TermCell>,
    /// Cursor position `(row, col)` in visible-grid coordinates, present only
    /// when the cursor is visible and on-screen.
    pub cursor: Option<(usize, usize)>,
}

impl TermSnapshot {
    /// The cell at `(row, col)`, or a blank default if out of range.
    pub fn cell(&self, row: usize, col: usize) -> TermCell {
        self.cells
            .get(row * self.cols + col)
            .copied()
            .unwrap_or_default()
    }
}

/// A gpui-free view of a key press, enough to encode VT input.
pub struct KeyInput {
    pub key: String,
    pub key_char: Option<String>,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub platform: bool,
}

/// One live terminal: pty child + VTE state. The reader thread feeds pty
/// output into the shared `Term` and flags `dirty`; rendering and input lock
/// it briefly.
pub struct TermBackend {
    term: Arc<FairMutex<Term<EventProxy>>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    dirty: Arc<AtomicBool>,
}

impl TermBackend {
    /// Spawn `cmd` (the user's shell by default) in a fresh pty sized
    /// `cols` x `rows`, inheriting the workbench cwd + env.
    pub fn spawn(
        cmd: Option<CommandBuilder>,
        cols: u16,
        rows: u16,
    ) -> std::io::Result<TermBackend> {
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

        let dirty = Arc::new(AtomicBool::new(true));
        let term_for_reader = term.clone();
        let dirty_for_reader = dirty.clone();
        std::thread::spawn(move || {
            let mut processor: Processor = Processor::new();
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                {
                    let mut term = term_for_reader.lock();
                    for byte in &buf[..n] {
                        processor.advance(&mut *term, *byte);
                    }
                }
                dirty_for_reader.store(true, Ordering::Release);
            }
        });

        Ok(TermBackend {
            term,
            master: pty.master,
            writer,
            child,
            dirty,
        })
    }

    /// Take and clear the "grid changed since last render" flag.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    /// True once the child has exited (`exit`, crash, kill). A `try_wait`
    /// hiccup fails *open* (still alive) so a transient error never reaps a
    /// live terminal.
    pub fn is_exited(&mut self) -> bool {
        self.child.try_wait().map(|s| s.is_some()).unwrap_or(false)
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
        self.dirty.store(true, Ordering::Release);
    }

    /// Forward a key press to the child as the bytes a terminal would send.
    /// Typing snaps any scrollback view back to the live bottom.
    pub fn send_input(&mut self, input: &KeyInput) {
        let app_cursor = {
            let mut term = self.term.lock();
            term.scroll_display(Scroll::Bottom);
            term.mode().contains(TermMode::APP_CURSOR)
        };
        if let Some(bytes) = encode_input(input, app_cursor) {
            let _ = self.writer.write_all(&bytes);
            let _ = self.writer.flush();
        }
    }

    /// Mouse wheel: scroll the scrollback on the primary screen; full-screen
    /// apps (alternate screen) get arrow keys instead, the terminal idiom.
    pub fn on_scroll(&mut self, down: bool) {
        let on_alt_screen = self.term.lock().mode().contains(TermMode::ALT_SCREEN);
        if on_alt_screen {
            let key = if down { "down" } else { "up" };
            for _ in 0..3 {
                self.send_input(&KeyInput {
                    key: key.to_string(),
                    key_char: None,
                    ctrl: false,
                    alt: false,
                    shift: false,
                    platform: false,
                });
            }
        } else {
            let delta = if down { -3 } else { 3 };
            self.term.lock().scroll_display(Scroll::Delta(delta));
            self.dirty.store(true, Ordering::Release);
        }
    }

    /// Snapshot the visible grid + cursor for rendering.
    pub fn snapshot(&self) -> TermSnapshot {
        let term = self.term.lock();
        let cols = term.grid().columns();
        let rows = term.grid().screen_lines();
        let content = term.renderable_content();

        let cursor_point = content.cursor.point;
        let cursor_visible = content.cursor.shape != CursorShape::Hidden;

        let mut cells = vec![TermCell::default(); rows * cols];
        let mut cursor = None;
        let mut row: usize = 0;
        let mut last_line: Option<i32> = None;

        for indexed in content.display_iter {
            let line = indexed.point.line.0;
            if let Some(prev) = last_line {
                if prev != line {
                    row += 1;
                }
            }
            last_line = Some(line);

            let col = indexed.point.column.0;
            if row >= rows || col >= cols {
                continue;
            }

            let cell = &indexed.cell;
            if cursor_visible && indexed.point == cursor_point {
                cursor = Some((row, col));
            }

            // Wide-char spacers carry no glyph; leave the blank default so the
            // leading wide glyph is not doubled.
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }

            cells[row * cols + col] = TermCell {
                c: cell.c,
                fg: resolve(cell.fg),
                bg: resolve(cell.bg),
                bold: cell.flags.contains(Flags::BOLD),
                italic: cell.flags.contains(Flags::ITALIC),
                underline: cell.flags.contains(Flags::UNDERLINE),
                inverse: cell.flags.contains(Flags::INVERSE),
                dim: cell.flags.contains(Flags::DIM),
            };
        }

        TermSnapshot {
            cols,
            rows,
            cells,
            cursor,
        }
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

/// Translate a key press into the byte sequence a VT sends. Covers the
/// daily-driver set: text, control chars, arrows (normal + app cursor mode),
/// nav keys, and Alt-prefixed input.
pub fn encode_input(input: &KeyInput, app_cursor: bool) -> Option<Vec<u8>> {
    // Cmd chords belong to the app/menu layer, never to the child shell.
    if input.platform {
        return None;
    }

    // Named (non-text) keys.
    let named: Option<Vec<u8>> = match input.key.as_str() {
        "enter" => Some(vec![b'\r']),
        "escape" => Some(vec![0x1b]),
        "backspace" => Some(vec![0x7f]),
        "tab" => Some(if input.shift {
            b"\x1b[Z".to_vec()
        } else {
            vec![b'\t']
        }),
        "up" => Some(arrow(b'A', app_cursor)),
        "down" => Some(arrow(b'B', app_cursor)),
        "right" => Some(arrow(b'C', app_cursor)),
        "left" => Some(arrow(b'D', app_cursor)),
        "home" => Some(b"\x1b[H".to_vec()),
        "end" => Some(b"\x1b[F".to_vec()),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "insert" => Some(b"\x1b[2~".to_vec()),
        _ => None,
    };
    if let Some(mut bytes) = named {
        if input.alt {
            bytes.insert(0, 0x1b);
        }
        return Some(bytes);
    }

    // Control chords -> C0 codes.
    if input.ctrl {
        let byte = control_byte(&input.key)?;
        let mut bytes = vec![byte];
        if input.alt {
            bytes.insert(0, 0x1b);
        }
        return Some(bytes);
    }

    // Printable input: prefer key_char (already honours shift + option layout);
    // fall back to the base key for single characters and space.
    let text = if input.key == "space" {
        " ".to_string()
    } else if let Some(kc) = &input.key_char {
        kc.clone()
    } else if input.key.chars().count() == 1 {
        input.key.clone()
    } else {
        return None;
    };
    if text.is_empty() {
        return None;
    }
    let mut bytes = text.into_bytes();
    if input.alt {
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

/// The C0 control byte for a `Ctrl`-chord key, or `None` if the key has no
/// control encoding.
fn control_byte(key: &str) -> Option<u8> {
    if key.chars().count() == 1 {
        let c = key.chars().next().unwrap();
        let upper = c.to_ascii_uppercase();
        if upper.is_ascii_uppercase() {
            return Some((upper as u8) & 0x1f);
        }
        return match c {
            ' ' => Some(0x00),
            '[' => Some(0x1b),
            '\\' => Some(0x1c),
            ']' => Some(0x1d),
            '^' => Some(0x1e),
            '_' => Some(0x1f),
            _ => None,
        };
    }
    match key {
        "space" => Some(0x00),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn key(k: &str) -> KeyInput {
        KeyInput {
            key: k.to_string(),
            key_char: if k.chars().count() == 1 {
                Some(k.to_string())
            } else {
                None
            },
            ctrl: false,
            alt: false,
            shift: false,
            platform: false,
        }
    }

    fn grid_text(snap: &TermSnapshot) -> String {
        (0..snap.rows)
            .map(|r| {
                (0..snap.cols)
                    .map(|c| snap.cell(r, c).c)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn wait_for(backend: &TermBackend, needle: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if grid_text(&backend.snapshot()).contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn shell_output_is_rendered_and_cursor_is_visible() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "printf 'workbench-pty-ok'; sleep 5"]);
        let backend = TermBackend::spawn(Some(cmd), 80, 24).expect("spawn pty");
        assert!(
            wait_for(&backend, "workbench-pty-ok"),
            "pty output not rendered: {}",
            grid_text(&backend.snapshot())
        );
        let snap = backend.snapshot();
        let (row, col) = snap.cursor.expect("cursor should be visible");
        assert!(row < snap.rows && col < snap.cols, "cursor within the grid");
    }

    #[test]
    fn input_reaches_the_child_shell() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "read line; printf 'got:%s' \"$line\"; sleep 5"]);
        let mut backend = TermBackend::spawn(Some(cmd), 80, 24).expect("spawn pty");
        for c in "hello".chars() {
            backend.send_input(&key(&c.to_string()));
        }
        backend.send_input(&key("enter"));
        assert!(
            wait_for(&backend, "got:hello"),
            "child never echoed input: {}",
            grid_text(&backend.snapshot())
        );
    }

    #[test]
    fn resize_updates_the_grid() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "sleep 5"]);
        let mut backend = TermBackend::spawn(Some(cmd), 80, 24).expect("spawn pty");
        backend.resize(40, 10);
        let snap = backend.snapshot();
        assert_eq!(snap.rows, 10);
        assert_eq!(snap.cols, 40);
    }

    #[test]
    fn exited_child_is_detected() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "exit 0"]);
        let mut backend = TermBackend::spawn(Some(cmd), 80, 24).expect("spawn pty");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !backend.is_exited() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(backend.is_exited(), "exit must be observable via try_wait");
    }

    #[test]
    fn super_chords_never_reach_the_child() {
        let mut k = key("k");
        k.platform = true;
        assert_eq!(encode_input(&k, false), None);
    }

    #[test]
    fn key_encoding_covers_control_arrows_and_alt() {
        let mut ctrl_c = key("c");
        ctrl_c.ctrl = true;
        assert_eq!(encode_input(&ctrl_c, false), Some(vec![0x03]));

        assert_eq!(encode_input(&key("up"), false), Some(b"\x1b[A".to_vec()));
        assert_eq!(encode_input(&key("up"), true), Some(b"\x1bOA".to_vec()));

        let mut alt_f = key("f");
        alt_f.alt = true;
        assert_eq!(encode_input(&alt_f, false), Some(vec![0x1b, b'f']));
    }

    #[test]
    fn plain_text_uses_key_char() {
        assert_eq!(encode_input(&key("a"), false), Some(b"a".to_vec()));
        assert_eq!(encode_input(&key("space"), false), Some(b" ".to_vec()));
    }
}
