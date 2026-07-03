//! winit → crossterm mouse translation for the window host: pixel positions
//! map onto the glyph grid (accounting for the centered blit's margins),
//! buttons become Down/Up/Drag events, and wheel deltas - line or pixel -
//! accumulate into whole scroll steps. The shell only ever sees cell-space
//! crossterm events, identical to a terminal with mouse capture on.

use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// Map a physical pixel position to a grid cell. The text texture is
/// `cols`x`rows` cells rendered 1:1 and centered on the surface; margins
/// clamp to the nearest cell so clicks in the padding still land.
#[allow(clippy::too_many_arguments)]
pub fn cell_at(
    px: f64,
    py: f64,
    surface_w: u32,
    surface_h: u32,
    cols: u16,
    rows: u16,
    region_w: u16,
    region_h: u16,
) -> (u16, u16) {
    let axis = |p: f64, surface: u32, n: u16, region: u16| -> u16 {
        if n == 0 || region == 0 {
            return 0;
        }
        let cell = (region / n).max(1) as f64;
        let tex = cell * n as f64;
        let off = (surface as f64 - tex) / 2.0;
        (((p - off) / cell).floor().max(0.0) as u16).min(n - 1)
    };
    (
        axis(px, surface_w, cols, region_w),
        axis(py, surface_h, rows, region_h),
    )
}

fn button(btn: winit::event::MouseButton) -> Option<MouseButton> {
    match btn {
        winit::event::MouseButton::Left => Some(MouseButton::Left),
        winit::event::MouseButton::Right => Some(MouseButton::Right),
        winit::event::MouseButton::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}

/// Per-window mouse state: last hovered cell, held button, wheel remainder.
#[derive(Default)]
pub struct MouseTracker {
    cell: (u16, u16),
    pressed: Option<MouseButton>,
    wheel_lines: f64,
}

impl MouseTracker {
    fn event(&self, kind: MouseEventKind, mods: KeyModifiers) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: self.cell.0,
            row: self.cell.1,
            modifiers: mods,
        })
    }

    /// Cursor moved to a (possibly unchanged) cell; a held button drags.
    pub fn moved(&mut self, cell: (u16, u16), mods: KeyModifiers) -> Option<Event> {
        if cell == self.cell {
            return None;
        }
        self.cell = cell;
        self.pressed
            .map(|btn| self.event(MouseEventKind::Drag(btn), mods))
    }

    /// A button changed state at the last hovered cell.
    pub fn input(
        &mut self,
        btn: winit::event::MouseButton,
        pressed: bool,
        mods: KeyModifiers,
    ) -> Option<Event> {
        let btn = button(btn)?;
        if pressed {
            self.pressed = Some(btn);
            Some(self.event(MouseEventKind::Down(btn), mods))
        } else {
            self.pressed = None;
            Some(self.event(MouseEventKind::Up(btn), mods))
        }
    }

    /// Wheel movement in lines (positive = up, winit convention). Fractional
    /// deltas (trackpads via pixel conversion) accumulate until whole lines.
    pub fn wheel(&mut self, lines: f64, mods: KeyModifiers) -> Vec<Event> {
        self.wheel_lines += lines;
        let whole = self.wheel_lines.trunc();
        self.wheel_lines -= whole;
        let kind = if whole > 0.0 {
            MouseEventKind::ScrollUp
        } else {
            MouseEventKind::ScrollDown
        };
        (0..whole.abs() as usize)
            .map(|_| self.event(kind, mods))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_mapping_accounts_for_centered_margins() {
        // 10x20px cells, 8x4 grid = 80x80 texture on a 100x90 surface:
        // 10px x-margin, 5px y-margin.
        assert_eq!(cell_at(10.0, 5.0, 100, 90, 8, 4, 80, 80), (0, 0));
        assert_eq!(cell_at(29.9, 24.9, 100, 90, 8, 4, 80, 80), (1, 0));
        assert_eq!(cell_at(89.9, 84.9, 100, 90, 8, 4, 80, 80), (7, 3));
        // Padding clicks clamp to the nearest cell instead of vanishing.
        assert_eq!(cell_at(0.0, 0.0, 100, 90, 8, 4, 80, 80), (0, 0));
        assert_eq!(cell_at(99.0, 89.0, 100, 90, 8, 4, 80, 80), (7, 3));
        // Degenerate grids never panic.
        assert_eq!(cell_at(5.0, 5.0, 100, 90, 0, 0, 0, 0), (0, 0));
    }

    #[test]
    fn buttons_produce_down_up_and_drag_between_cells() {
        let mut t = MouseTracker::default();
        t.moved((3, 2), KeyModifiers::NONE);
        let Some(Event::Mouse(down)) =
            t.input(winit::event::MouseButton::Left, true, KeyModifiers::NONE)
        else {
            panic!("expected down event");
        };
        assert_eq!(down.kind, MouseEventKind::Down(MouseButton::Left));
        assert_eq!((down.column, down.row), (3, 2));
        // Moving within the same cell is silent; a new cell drags.
        assert!(t.moved((3, 2), KeyModifiers::NONE).is_none());
        let Some(Event::Mouse(drag)) = t.moved((4, 2), KeyModifiers::NONE) else {
            panic!("expected drag event");
        };
        assert_eq!(drag.kind, MouseEventKind::Drag(MouseButton::Left));
        let Some(Event::Mouse(up)) =
            t.input(winit::event::MouseButton::Left, false, KeyModifiers::NONE)
        else {
            panic!("expected up event");
        };
        assert_eq!(up.kind, MouseEventKind::Up(MouseButton::Left));
        assert!(
            t.moved((5, 2), KeyModifiers::NONE).is_none(),
            "no drag after up"
        );
    }

    #[test]
    fn wheel_accumulates_fractional_deltas_into_whole_steps() {
        let mut t = MouseTracker::default();
        assert!(t.wheel(0.4, KeyModifiers::NONE).is_empty());
        let events = t.wheel(0.7, KeyModifiers::NONE);
        assert_eq!(events.len(), 1);
        let Event::Mouse(m) = events[0] else { panic!() };
        assert_eq!(m.kind, MouseEventKind::ScrollUp);
        // Downward (negative) deltas scroll down; multiples emit multiples.
        let events = t.wheel(-2.1, KeyModifiers::NONE);
        assert_eq!(events.len(), 2);
        let Event::Mouse(m) = events[0] else { panic!() };
        assert_eq!(m.kind, MouseEventKind::ScrollDown);
    }
}
