//! The custom gpui [`Element`] that paints the terminal grid.
//!
//! It shapes one line of monospace text per grid row (grouping cells into
//! `TextRun`s by style), paints per-run backgrounds then glyphs, and draws a
//! real cursor: a solid block when focused (with the underlying glyph redrawn
//! on top for contrast) or a hollow outline when not. This is the headline
//! fix over the origin repo, whose renderer never drew a cursor at all.
//!
//! Layout drives the pty size: each prepaint measures the cell box, derives
//! `cols`/`rows` from the element bounds, and asks the view to resize the
//! backend before snapshotting - so the shell always matches the visible grid.

use gpui::{
    fill, point, px, relative, size, App, Bounds, Element, ElementId, Font, FontStyle, FontWeight,
    GlobalElementId, Hitbox, HitboxBehavior, Hsla, IntoElement, LayoutId, Pixels, Rgba, ShapedLine,
    Style, TextAlign, TextRun, UnderlineStyle, Window,
};
use gpui_component::ActiveTheme;

use super::ansi::CellColor;
use super::backend::{TermCell, TermSnapshot};
use super::view::TerminalView;
use gpui::Entity;

/// Monospace family and size for the terminal grid. Made configurable via Lua
/// in a later step; a fixed high-quality default for now.
const FONT_FAMILY: &str = "Menlo";
const FONT_SIZE: f32 = 13.0;
const LINE_HEIGHT_RATIO: f32 = 1.4;
/// Fallback cell width if the font fails to measure (missing family).
const FALLBACK_CELL_W: f32 = 8.0;

/// Paints one [`TerminalView`]'s grid. Holds the view entity so it can resize
/// and snapshot the backend during layout, mirroring gpui-component's own
/// `TextElement` pattern.
pub struct TerminalElement {
    view: Entity<TerminalView>,
}

impl TerminalElement {
    pub fn new(view: Entity<TerminalView>) -> Self {
        Self { view }
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// State carried from prepaint to paint.
pub struct TermPrepaint {
    lines: Vec<ShapedLine>,
    cell_w: Pixels,
    cell_h: Pixels,
    cursor: Option<(usize, usize)>,
    cursor_char: char,
    focused: bool,
    blink: bool,
    hitbox: Hitbox,
    base_font: Font,
    font_size: Pixels,
    bg: Hsla,
    caret: Hsla,
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = TermPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        style.flex_grow = 1.0;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let theme = cx.theme();
        // Translucent under the glass theme (see `ui::theme::pane_bg`) so the
        // window's real macOS vibrancy blur shows through the terminal's base
        // fill; per-cell glyph runs below stay fully opaque for legibility.
        let bg = super::super::theme::pane_bg(cx);
        let fg = theme.foreground;
        let caret = theme.caret;

        let base_font = font_for(false, false);
        let font_size = px(FONT_SIZE);
        let cell_h = px((FONT_SIZE * LINE_HEIGHT_RATIO).round());

        // Measure the cell box from a representative glyph.
        let probe = window.text_system().shape_line(
            "M".into(),
            font_size,
            &[TextRun {
                len: 1,
                font: base_font.clone(),
                color: fg,
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let cell_w = if probe.width() > px(0.) {
            probe.width()
        } else {
            px(FALLBACK_CELL_W)
        };

        let cols = ((bounds.size.width.as_f32() / cell_w.as_f32()).floor() as usize).max(1);
        let rows = ((bounds.size.height.as_f32() / cell_h.as_f32()).floor() as usize).max(1);

        // Resize the pty to match the visible grid, then snapshot it.
        let snapshot = self.view.update(cx, |view, _| {
            view.sync_and_snapshot(cols as u16, rows as u16)
        });

        let (lines, cursor, cursor_char) = match snapshot {
            Some(snap) => {
                let lines = shape_grid(&snap, &base_font, font_size, fg, window);
                let cursor_char = snap.cursor.map(|(r, c)| snap.cell(r, c).c).unwrap_or(' ');
                (lines, snap.cursor, cursor_char)
            }
            None => (Vec::new(), None, ' '),
        };

        let view = self.view.read(cx);
        let focused = view.focused(window);
        let blink = view.blink();

        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);

        TermPrepaint {
            lines,
            cell_w,
            cell_h,
            cursor,
            cursor_char,
            focused,
            blink,
            hitbox,
            base_font,
            font_size,
            bg,
            caret,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = bounds.origin;
        let cell_h = prepaint.cell_h;
        let cell_w = prepaint.cell_w;

        // Base background.
        window.paint_quad(fill(bounds, prepaint.bg));

        // Grid rows: per-run backgrounds, then glyphs.
        for (row, line) in prepaint.lines.iter().enumerate() {
            let p = point(origin.x, origin.y + cell_h * row as f32);
            let _ = line.paint_background(p, cell_h, TextAlign::Left, None, window, cx);
            let _ = line.paint(p, cell_h, TextAlign::Left, None, window, cx);
        }

        // Cursor.
        if let Some((row, col)) = prepaint.cursor {
            let cursor_origin = point(
                origin.x + cell_w * col as f32,
                origin.y + cell_h * row as f32,
            );
            let cursor_bounds = Bounds::new(cursor_origin, size(cell_w, cell_h));

            if prepaint.focused && prepaint.blink {
                // Solid block, with the glyph redrawn on top in the background
                // colour so it stays legible.
                window.paint_quad(fill(cursor_bounds, prepaint.caret));
                if prepaint.cursor_char != ' ' {
                    let run = TextRun {
                        len: prepaint.cursor_char.len_utf8(),
                        font: prepaint.base_font.clone(),
                        color: prepaint.bg,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let glyph = window.text_system().shape_line(
                        prepaint.cursor_char.to_string().into(),
                        prepaint.font_size,
                        &[run],
                        None,
                    );
                    let _ = glyph.paint(cursor_origin, cell_h, TextAlign::Left, None, window, cx);
                }
            } else {
                // Hollow outline (unfocused / blink-off): four 1px edges.
                let t = px(1.0);
                let c = prepaint.caret;
                window.paint_quad(fill(Bounds::new(cursor_origin, size(cell_w, t)), c));
                window.paint_quad(fill(
                    Bounds::new(
                        point(cursor_origin.x, cursor_origin.y + cell_h - t),
                        size(cell_w, t),
                    ),
                    c,
                ));
                window.paint_quad(fill(Bounds::new(cursor_origin, size(t, cell_h)), c));
                window.paint_quad(fill(
                    Bounds::new(
                        point(cursor_origin.x + cell_w - t, cursor_origin.y),
                        size(t, cell_h),
                    ),
                    c,
                ));
            }
        }

        window.set_cursor_style(gpui::CursorStyle::IBeam, &prepaint.hitbox);
    }
}

/// Shape every grid row into a [`ShapedLine`], grouping cells into runs by
/// style so runs of the same colour/weight shape together.
fn shape_grid(
    snap: &TermSnapshot,
    base_font: &Font,
    font_size: Pixels,
    fg_default: Hsla,
    window: &mut Window,
) -> Vec<ShapedLine> {
    let mut lines = Vec::with_capacity(snap.rows);
    for row in 0..snap.rows {
        let (text, runs) = build_line(snap, row, base_font, fg_default);
        let shaped = window
            .text_system()
            .shape_line(text.into(), font_size, &runs, None);
        lines.push(shaped);
    }
    lines
}

/// Style key for run grouping.
#[derive(Clone, Copy, PartialEq)]
struct RunStyle {
    fg: Hsla,
    bg: Option<Hsla>,
    bold: bool,
    italic: bool,
    underline: bool,
}

/// Build the concatenated text of a row plus the `TextRun`s describing it.
fn build_line(
    snap: &TermSnapshot,
    row: usize,
    base_font: &Font,
    fg_default: Hsla,
) -> (String, Vec<TextRun>) {
    let mut text = String::with_capacity(snap.cols);
    let mut runs: Vec<TextRun> = Vec::new();
    let mut current: Option<(RunStyle, usize)> = None; // (style, byte len)

    for col in 0..snap.cols {
        let cell = snap.cell(row, col);
        let (fg, bg) = cell_colors(&cell, fg_default);
        let style = RunStyle {
            fg,
            bg,
            bold: cell.bold,
            italic: cell.italic,
            underline: cell.underline,
        };
        text.push(cell.c);
        let clen = cell.c.len_utf8();

        match &mut current {
            Some((cur_style, len)) if *cur_style == style => {
                *len += clen;
            }
            _ => {
                if let Some((cur_style, len)) = current.take() {
                    runs.push(run_from(cur_style, len, base_font));
                }
                current = Some((style, clen));
            }
        }
    }
    if let Some((cur_style, len)) = current.take() {
        runs.push(run_from(cur_style, len, base_font));
    }

    (text, runs)
}

fn run_from(style: RunStyle, len: usize, base_font: &Font) -> TextRun {
    let mut font = base_font.clone();
    if style.bold {
        font.weight = FontWeight::BOLD;
    }
    if style.italic {
        font.style = FontStyle::Italic;
    }
    let underline = if style.underline {
        Some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(style.fg),
            wavy: false,
        })
    } else {
        None
    };
    TextRun {
        len,
        font,
        color: style.fg,
        background_color: style.bg,
        underline,
        strikethrough: None,
    }
}

/// Resolve a cell's effective (fg, optional bg) as `Hsla`, applying inverse
/// and dim.
fn cell_colors(cell: &TermCell, fg_default: Hsla) -> (Hsla, Option<Hsla>) {
    let mut fg = match cell.fg {
        CellColor::Default => fg_default,
        CellColor::Rgb(r, g, b) => rgb_to_hsla(r, g, b),
    };
    let mut bg = match cell.bg {
        CellColor::Default => None,
        CellColor::Rgb(r, g, b) => Some(rgb_to_hsla(r, g, b)),
    };
    if cell.inverse {
        let new_bg = fg;
        let new_fg = bg.unwrap_or(fg_default);
        fg = new_fg;
        bg = Some(new_bg);
    }
    if cell.dim {
        fg = fg.opacity(0.65);
    }
    (fg, bg)
}

fn rgb_to_hsla(r: u8, g: u8, b: u8) -> Hsla {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

fn font_for(bold: bool, italic: bool) -> Font {
    let mut font = gpui::font(FONT_FAMILY);
    if bold {
        font.weight = FontWeight::BOLD;
    }
    if italic {
        font.style = FontStyle::Italic;
    }
    font
}
