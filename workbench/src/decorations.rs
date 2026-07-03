//! Sub-cell decoration vocabulary (issue #29), expressed in cell space so it
//! works identically above both backends: severity colors for diagnostics,
//! the ghost-text style for edit prediction, and a marked scrollbar strip
//! (Zed-style: search/diagnostic marks visible at a glance). True sub-pixel
//! variants (wavy underlines, alpha-blended ghost text) land with the
//! renderer-v2 text stack.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

/// LSP severity → color, honest in 256-color terminals too.
/// 1 = error, 2 = warning, everything else = info.
pub fn severity_color(severity: u8) -> Color {
    match severity {
        1 => Color::Indexed(203), // error red
        2 => Color::Indexed(226), // warning yellow
        _ => Color::Indexed(63),  // info blue
    }
}

/// Ghost text (edit-prediction previews, inline hints): dim + italic so it
/// reads as "not yet real" in every backend.
pub fn ghost() -> Style {
    Style::default()
        .fg(Color::Indexed(144))
        .add_modifier(Modifier::DIM | Modifier::ITALIC)
}

/// The squiggle-degraded diagnostic emphasis for a run of text.
pub fn squiggle(severity: u8) -> Style {
    Style::default()
        .add_modifier(Modifier::UNDERLINED)
        .underline_color(severity_color(severity))
}

/// A 1-column scrollbar strip with a viewport thumb and per-line marks.
/// Marks win over the thumb so problems stay visible while scrolling.
pub struct MarkedScrollbar {
    total_lines: usize,
    scroll_top: usize,
    viewport_lines: usize,
    /// (line, severity) pairs; lowest severity number wins per row.
    marks: Vec<(usize, u8)>,
}

impl MarkedScrollbar {
    pub fn new(
        total_lines: usize,
        scroll_top: usize,
        viewport_lines: usize,
        marks: Vec<(usize, u8)>,
    ) -> Self {
        Self {
            total_lines,
            scroll_top,
            viewport_lines,
            marks,
        }
    }

    /// Map a document line onto a strip row.
    pub fn line_to_row(line: usize, total_lines: usize, height: usize) -> usize {
        if total_lines <= 1 || height == 0 {
            return 0;
        }
        (line * height / total_lines).min(height.saturating_sub(1))
    }

    /// The inclusive row range covered by the viewport thumb.
    pub fn thumb_rows(&self, height: usize) -> (usize, usize) {
        let start = Self::line_to_row(self.scroll_top, self.total_lines, height);
        let last_visible = self
            .scroll_top
            .saturating_add(self.viewport_lines.saturating_sub(1))
            .min(self.total_lines.saturating_sub(1));
        let end = Self::line_to_row(last_visible, self.total_lines, height);
        (start, end.max(start))
    }
}

impl Widget for MarkedScrollbar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let height = area.height as usize;
        let (thumb_start, thumb_end) = self.thumb_rows(height);

        // Everything fits: no scrollbar noise, but marks still show.
        let fits = self.total_lines <= self.viewport_lines;

        let mut rows: Vec<Option<u8>> = vec![None; height];
        for (line, severity) in &self.marks {
            let row = Self::line_to_row(*line, self.total_lines.max(1), height);
            rows[row] = Some(rows[row].map_or(*severity, |s| s.min(*severity)));
        }

        for (row, mark) in rows.iter().enumerate() {
            let y = area.y + row as u16;
            let (symbol, style) = if let Some(severity) = *mark {
                ("◆", Style::default().fg(severity_color(severity)))
            } else if !fits && row >= thumb_start && row <= thumb_end {
                ("█", Style::default().fg(Color::Indexed(101)))
            } else if !fits {
                (
                    "│",
                    Style::default()
                        .fg(Color::Indexed(101))
                        .add_modifier(Modifier::DIM),
                )
            } else {
                (" ", Style::default())
            };
            buf.set_string(area.x, y, symbol, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_maps_to_error_warning_info() {
        assert_eq!(severity_color(1), Color::Indexed(203));
        assert_eq!(severity_color(2), Color::Indexed(226));
        assert_eq!(severity_color(3), Color::Indexed(63));
        assert_eq!(severity_color(4), Color::Indexed(63));
    }

    #[test]
    fn line_to_row_scales_into_strip() {
        assert_eq!(MarkedScrollbar::line_to_row(0, 100, 10), 0);
        assert_eq!(MarkedScrollbar::line_to_row(50, 100, 10), 5);
        assert_eq!(MarkedScrollbar::line_to_row(99, 100, 10), 9);
        // Clamps rather than overflowing the strip.
        assert_eq!(MarkedScrollbar::line_to_row(150, 100, 10), 9);
        assert_eq!(MarkedScrollbar::line_to_row(5, 1, 10), 0);
    }

    #[test]
    fn thumb_covers_viewport_proportion() {
        let bar = MarkedScrollbar::new(100, 40, 20, vec![]);
        let (start, end) = bar.thumb_rows(10);
        assert_eq!(start, 4);
        assert_eq!(end, 5);
    }

    #[test]
    fn renders_marks_over_thumb() {
        let bar = MarkedScrollbar::new(100, 0, 10, vec![(5, 1)]);
        let area = Rect::new(0, 0, 1, 10);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
        // Line 5 of 100 maps to row 0, where the thumb also sits: mark wins.
        assert_eq!(buf[(0, 0)].symbol(), "◆");
    }

    #[test]
    fn short_documents_render_blank_track() {
        let bar = MarkedScrollbar::new(5, 0, 20, vec![]);
        let area = Rect::new(0, 0, 1, 10);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), " ");
        assert_eq!(buf[(0, 9)].symbol(), " ");
    }

    #[test]
    fn error_outranks_warning_on_same_row() {
        let bar = MarkedScrollbar::new(100, 0, 10, vec![(50, 2), (52, 1)]);
        let area = Rect::new(0, 0, 1, 10);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
        assert_eq!(buf[(0, 5)].symbol(), "◆");
        assert_eq!(buf[(0, 5)].fg, severity_color(1));
    }
}
