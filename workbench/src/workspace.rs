//! IDE workspace chrome (the Zed-model shell): a tab bar on top, a
//! persistent file-tree sidebar on the left, the editor pane tree in the
//! center, an integrated terminal dock along the bottom, and the statusline.
//! Pure value-state + geometry so every region is unit-testable; rendering
//! and event routing live in `shell.rs` / `mouse.rs`.

use crate::layout::{Layout, PaneId};
use ratatui::layout::Rect;

/// The terminal dock's pane id in the `PaneStore` - outside the layout
/// allocator's range so it never collides with center panes.
pub const DOCK_PANE: PaneId = u64::MAX;

/// Which chrome region owns the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Center,
    Dock,
    Sidebar,
}

/// Cloneable chrome state (the sidebar's tree itself lives on the shell).
#[derive(Clone, Debug)]
pub struct Chrome {
    pub dock_open: bool,
    pub focus: Region,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            dock_open: true,
            focus: Region::Center,
        }
    }
}

/// The computed rectangles of every chrome region for one frame.
#[derive(Clone, Debug)]
pub struct ChromeRects {
    pub tab_bar: Rect,
    pub sidebar: Option<Rect>,
    pub center: Rect,
    pub dock: Option<Rect>,
    pub status: Rect,
}

/// Sidebar width for a given frame width (0 = too narrow to show).
fn sidebar_width(area_width: u16) -> u16 {
    if area_width < 48 {
        return 0;
    }
    (area_width / 5).clamp(24, 36)
}

/// Dock height for a given body height (0 = too short to show).
fn dock_height(body_height: u16) -> u16 {
    if body_height < 10 {
        return 0;
    }
    (body_height * 3 / 10).max(8)
}

/// Slice the frame into chrome regions. `None` when the frame is too small
/// to render anything meaningful.
pub fn chrome_rects(area: Rect, sidebar_open: bool, dock_open: bool) -> Option<ChromeRects> {
    if area.height < 4 || area.width < 10 {
        return None;
    }
    let tab_bar = Rect { height: 1, ..area };
    let status = Rect {
        y: area.y + area.height - 1,
        height: 1,
        ..area
    };
    let body = Rect {
        y: area.y + 1,
        height: area.height - 2,
        ..area
    };

    let sb_width = if sidebar_open {
        sidebar_width(area.width)
    } else {
        0
    };
    let sidebar = (sb_width > 0).then_some(Rect {
        width: sb_width,
        ..body
    });
    let right = Rect {
        x: body.x + sb_width,
        width: body.width - sb_width,
        ..body
    };

    let d_height = if dock_open {
        dock_height(body.height)
    } else {
        0
    };
    let dock = (d_height > 0).then_some(Rect {
        y: right.y + right.height - d_height,
        height: d_height,
        ..right
    });
    let center = Rect {
        height: right.height - d_height,
        ..right
    };

    Some(ChromeRects {
        tab_bar,
        sidebar,
        center,
        dock,
        status,
    })
}

/// A pane's one-row header (kind dot + title + close button). Panes are
/// flat Zed-style surfaces: header on top, content below, no box borders.
pub fn pane_header(rect: Rect) -> Rect {
    Rect { height: 1, ..rect }
}

/// The drawable content region below the header.
pub fn pane_content(rect: Rect) -> Rect {
    Rect {
        y: rect.y + 1,
        height: rect.height.saturating_sub(1),
        ..rect
    }
}

/// The clickable ` × ` at the header's right edge.
pub fn close_button(rect: Rect) -> Rect {
    let width = 3.min(rect.width);
    Rect {
        x: rect.x + rect.width - width,
        y: rect.y,
        width,
        height: 1,
    }
}

/// What clicking the tab bar at a given column does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabHit {
    Tab(usize),
    NewTab,
}

/// The tab bar's segments as (label, hit) pairs; render and hit-testing
/// both walk these so clicks always match what is drawn.
pub fn tab_bar_items(layout: &Layout) -> Vec<(String, TabHit)> {
    let mut items: Vec<(String, TabHit)> = layout
        .tabs
        .iter()
        .enumerate()
        .map(|(i, _)| (format!(" {} ", i + 1), TabHit::Tab(i)))
        .collect();
    items.push((" + ".to_string(), TabHit::NewTab));
    items
}

/// Hit-test a click at `col` columns past the tab bar's left edge.
pub fn tab_hit(layout: &Layout, col: u16) -> Option<TabHit> {
    let mut x = 0u16;
    for (label, hit) in tab_bar_items(layout) {
        let w = label.chars().count() as u16;
        if col >= x && col < x + w {
            return Some(hit);
        }
        x += w;
    }
    None
}

/// The welcome surface shown by panes with nothing open yet.
pub fn welcome_lines() -> Vec<(&'static str, bool)> {
    // (text, emphasized)
    vec![
        ("", false),
        ("LIFE OS WORKBENCH", true),
        ("terminal-weight IDE · life os inside", false),
        ("", false),
        (
            "ctrl+o   open file            ctrl+k   command palette",
            false,
        ),
        (
            "alt+f    files sidebar        alt+j    terminal dock",
            false,
        ),
        ("alt+e    editor here          alt+a    agent pane", false),
        (
            "alt+l    life os modules      alt+/    recall search",
            false,
        ),
        ("alt+s/v  split right/down     alt+x    close pane", false),
        ("", false),
        ("click anywhere · scroll everywhere · drag files in", false),
    ]
}

/// First visible row of a list that keeps `selected` in view.
pub fn scroll_offset(selected: usize, height: usize) -> usize {
    if height == 0 {
        return 0;
    }
    selected.saturating_sub(height - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn regions_tile_the_frame_without_overlap() {
        let cr = chrome_rects(rect(120, 40), true, true).unwrap();
        let sidebar = cr.sidebar.expect("wide frame has a sidebar");
        let dock = cr.dock.expect("tall frame has a dock");
        assert_eq!(cr.tab_bar.height, 1);
        assert_eq!(cr.status.y, 39);
        // Sidebar spans the full body height; dock sits right of it.
        assert_eq!(sidebar.height, 38);
        assert_eq!(dock.x, sidebar.width);
        assert_eq!(dock.y + dock.height, 39);
        // Center + dock + sidebar cover the body exactly.
        let body_cells = 120u32 * 38;
        let covered = sidebar.area() + dock.area() + cr.center.area();
        assert_eq!(covered, body_cells);
    }

    #[test]
    fn closed_chrome_gives_the_center_everything() {
        let cr = chrome_rects(rect(120, 40), false, false).unwrap();
        assert!(cr.sidebar.is_none() && cr.dock.is_none());
        assert_eq!(cr.center, Rect::new(0, 1, 120, 38));
    }

    #[test]
    fn narrow_or_short_frames_degrade_gracefully() {
        // Too narrow for a sidebar, too short for a dock - but still valid.
        let cr = chrome_rects(rect(40, 8), true, true).unwrap();
        assert!(cr.sidebar.is_none());
        assert!(cr.dock.is_none());
        assert!(chrome_rects(rect(120, 3), true, true).is_none());
    }

    #[test]
    fn tab_bar_hits_match_rendered_segments() {
        let layout = Layout::new().new_tab().0;
        assert_eq!(tab_hit(&layout, 0), Some(TabHit::Tab(0)));
        assert_eq!(tab_hit(&layout, 4), Some(TabHit::Tab(1)));
        assert_eq!(tab_hit(&layout, 7), Some(TabHit::NewTab));
        assert_eq!(tab_hit(&layout, 60), None);
    }

    #[test]
    fn scroll_offset_keeps_selection_visible() {
        assert_eq!(scroll_offset(0, 10), 0);
        assert_eq!(scroll_offset(9, 10), 0);
        assert_eq!(scroll_offset(15, 10), 6);
        assert_eq!(scroll_offset(5, 0), 0);
    }
}
