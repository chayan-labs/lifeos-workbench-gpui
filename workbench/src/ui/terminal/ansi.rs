//! ANSI colour resolution for the terminal grid.
//!
//! alacritty hands us cells coloured as `AnsiColor::{Named,Indexed,Spec}`.
//! We resolve those to a renderer-neutral [`CellColor`]: either a concrete
//! RGB triple (from the standard xterm 256 palette or a truecolour spec) or
//! [`CellColor::Default`], meaning "substitute the theme's foreground /
//! background at paint time". Keeping this gpui-free makes it unit-testable
//! and lets the element own all theme coupling.

use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};

/// A resolved cell colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellColor {
    /// The terminal's default fg/bg; the element maps this to the theme.
    Default,
    /// A concrete 8-bit-per-channel colour.
    Rgb(u8, u8, u8),
}

/// Resolve an alacritty colour to a [`CellColor`].
pub fn resolve(color: AnsiColor) -> CellColor {
    match color {
        AnsiColor::Spec(rgb) => CellColor::Rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(i) => indexed(i),
        AnsiColor::Named(named) => resolve_named(named),
    }
}

/// The 16 standard/bright ANSI colours (xterm defaults).
const SYSTEM_16: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), // 0 black
    (0x80, 0x00, 0x00), // 1 red
    (0x00, 0x80, 0x00), // 2 green
    (0x80, 0x80, 0x00), // 3 yellow
    (0x00, 0x00, 0x80), // 4 blue
    (0x80, 0x00, 0x80), // 5 magenta
    (0x00, 0x80, 0x80), // 6 cyan
    (0xc0, 0xc0, 0xc0), // 7 white
    (0x80, 0x80, 0x80), // 8 bright black
    (0xff, 0x00, 0x00), // 9 bright red
    (0x00, 0xff, 0x00), // 10 bright green
    (0xff, 0xff, 0x00), // 11 bright yellow
    (0x00, 0x00, 0xff), // 12 bright blue
    (0xff, 0x00, 0xff), // 13 bright magenta
    (0x00, 0xff, 0xff), // 14 bright cyan
    (0xff, 0xff, 0xff), // 15 bright white
];

/// Resolve a palette index (0-255) to an RGB colour via the xterm layout:
/// 0-15 system, 16-231 the 6x6x6 cube, 232-255 the grayscale ramp.
fn indexed(i: u8) -> CellColor {
    let (r, g, b) = match i {
        0..=15 => SYSTEM_16[i as usize],
        16..=231 => {
            let i = i - 16;
            let levels = [0u8, 95, 135, 175, 215, 255];
            let r = levels[(i / 36) as usize];
            let g = levels[((i / 6) % 6) as usize];
            let b = levels[(i % 6) as usize];
            (r, g, b)
        }
        232..=255 => {
            let v = 8 + 10 * (i - 232);
            (v, v, v)
        }
    };
    CellColor::Rgb(r, g, b)
}

/// Map a named colour to a palette entry, leaving the terminal's default
/// fg/bg (and their bright/dim aliases) as [`CellColor::Default`] so the
/// theme drives them.
fn resolve_named(named: NamedColor) -> CellColor {
    use NamedColor::*;
    let idx = match named {
        Black => 0,
        Red => 1,
        Green => 2,
        Yellow => 3,
        Blue => 4,
        Magenta => 5,
        Cyan => 6,
        White => 7,
        BrightBlack => 8,
        BrightRed => 9,
        BrightGreen => 10,
        BrightYellow => 11,
        BrightBlue => 12,
        BrightMagenta => 13,
        BrightCyan => 14,
        BrightWhite => 15,
        DimBlack => 0,
        DimRed => 1,
        DimGreen => 2,
        DimYellow => 3,
        DimBlue => 4,
        DimMagenta => 5,
        DimCyan => 6,
        DimWhite => 7,
        // Foreground/Background and their aliases follow the theme.
        _ => return CellColor::Default,
    };
    SYSTEM_16
        .get(idx)
        .map(|&(r, g, b)| CellColor::Rgb(r, g, b))
        .unwrap_or(CellColor::Default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_is_passed_through() {
        let rgb = alacritty_terminal::vte::ansi::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        assert_eq!(resolve(AnsiColor::Spec(rgb)), CellColor::Rgb(10, 20, 30));
    }

    #[test]
    fn foreground_and_background_stay_default() {
        assert_eq!(
            resolve(AnsiColor::Named(NamedColor::Foreground)),
            CellColor::Default
        );
        assert_eq!(
            resolve(AnsiColor::Named(NamedColor::Background)),
            CellColor::Default
        );
    }

    #[test]
    fn named_red_resolves_to_system_palette() {
        assert_eq!(
            resolve(AnsiColor::Named(NamedColor::Red)),
            CellColor::Rgb(0x80, 0, 0)
        );
    }

    #[test]
    fn cube_index_is_computed() {
        // 16 is the first cube entry: (0,0,0).
        assert_eq!(indexed(16), CellColor::Rgb(0, 0, 0));
        // 231 is the last cube entry: (255,255,255).
        assert_eq!(indexed(231), CellColor::Rgb(255, 255, 255));
    }

    #[test]
    fn grayscale_ramp_is_computed() {
        assert_eq!(indexed(232), CellColor::Rgb(8, 8, 8));
        assert_eq!(indexed(255), CellColor::Rgb(238, 238, 238));
    }
}
