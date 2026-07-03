//! Terminal Brutalism theme (frontend/DESIGN.md): the palette with
//! truecolor / 256-color / 16-color fallbacks, SGR emphasis, box-drawing
//! border weights, and the single statusline.

use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};

/// What the terminal can actually show. Detected once at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSupport {
    TrueColor,
    Ansi256,
    Ansi16,
}

impl ColorSupport {
    /// Detect from the standard `COLORTERM` / `TERM` env contract.
    pub fn detect() -> Self {
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        if colorterm.contains("truecolor") || colorterm.contains("24bit") {
            return Self::TrueColor;
        }
        let term = std::env::var("TERM").unwrap_or_default();
        if term.contains("256color") {
            Self::Ansi256
        } else {
            Self::Ansi16
        }
    }
}

/// One palette entry: truecolor hex plus its two degraded fallbacks.
#[derive(Clone, Copy, Debug)]
pub struct PaletteColor {
    rgb: (u8, u8, u8),
    ansi256: u8,
    ansi16: Color,
}

impl PaletteColor {
    pub const fn new(rgb: (u8, u8, u8), ansi256: u8, ansi16: Color) -> Self {
        Self {
            rgb,
            ansi256,
            ansi16,
        }
    }

    /// Raw truecolor components (used by the GPU host for shader uniforms).
    pub const fn rgb(&self) -> (u8, u8, u8) {
        self.rgb
    }

    pub fn resolve(&self, support: ColorSupport) -> Color {
        match support {
            ColorSupport::TrueColor => Color::Rgb(self.rgb.0, self.rgb.1, self.rgb.2),
            ColorSupport::Ansi256 => Color::Indexed(self.ansi256),
            ColorSupport::Ansi16 => self.ansi16,
        }
    }
}

// DESIGN.md front-matter palette, verbatim.
pub const BG: PaletteColor = PaletteColor::new((0x12, 0x12, 0x0a), 233, Color::Black);
pub const BG_ALT: PaletteColor = PaletteColor::new((0x1c, 0x1c, 0x0f), 234, Color::Black);
pub const SURFACE: PaletteColor = PaletteColor::new((0x26, 0x26, 0x1a), 235, Color::DarkGray);
pub const FG: PaletteColor = PaletteColor::new((0xf5, 0xf1, 0xdc), 230, Color::White);
pub const FG_DIM: PaletteColor = PaletteColor::new((0xa8, 0xa6, 0x8c), 144, Color::DarkGray);
pub const PRIMARY: PaletteColor = PaletteColor::new((0x4b, 0x4c, 0xff), 63, Color::LightBlue);
pub const ACCENT: PaletteColor = PaletteColor::new((0xff, 0xff, 0x00), 226, Color::LightYellow);
pub const SUCCESS: PaletteColor = PaletteColor::new((0x00, 0xff, 0x9d), 48, Color::LightGreen);
pub const ERROR: PaletteColor = PaletteColor::new((0xff, 0x4b, 0x4b), 203, Color::LightRed);
pub const OUTLINE: PaletteColor = PaletteColor::new((0x79, 0x78, 0x5f), 101, Color::DarkGray);
pub const OUTLINE_FOCUS: PaletteColor = ACCENT;
/// Human-gate required (outward/irreversible) - same hue as error by design.
pub const GATE: PaletteColor = ERROR;

/// Resolved theme handle every widget draws with.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub support: ColorSupport,
}

impl Theme {
    pub fn new(support: ColorSupport) -> Self {
        Self { support }
    }

    pub fn text(&self) -> Style {
        Style::default().fg(FG.resolve(self.support))
    }

    pub fn muted(&self) -> Style {
        Style::default()
            .fg(FG_DIM.resolve(self.support))
            .add_modifier(Modifier::DIM)
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(FG.resolve(self.support))
            .add_modifier(Modifier::BOLD)
    }

    /// Level 3: the selected row/item - the "pressed" analogue.
    pub fn active_item(&self) -> Style {
        Style::default()
            .fg(ACCENT.resolve(self.support))
            .add_modifier(Modifier::REVERSED)
    }

    /// Anything human-gated: never color alone - bold carries it on monochrome.
    pub fn gated(&self) -> Style {
        Style::default()
            .fg(GATE.resolve(self.support))
            .add_modifier(Modifier::BOLD)
    }

    /// Chrome panel fill (tab bar, sidebar, statusline): one shade above
    /// the editor background, the Zed panel idiom.
    pub fn panel_bg(&self) -> Style {
        Style::default()
            .bg(BG_ALT.resolve(self.support))
            .fg(FG_DIM.resolve(self.support))
    }

    /// The focused pane's header row - raised surface, readable title.
    pub fn header_focused(&self) -> Style {
        Style::default()
            .bg(SURFACE.resolve(self.support))
            .fg(FG.resolve(self.support))
            .add_modifier(Modifier::BOLD)
    }

    /// Unfocused pane headers recede into the panel shade.
    pub fn header_unfocused(&self) -> Style {
        Style::default()
            .bg(BG_ALT.resolve(self.support))
            .fg(FG_DIM.resolve(self.support))
    }

    /// The active tab connects to the editor surface below it.
    pub fn tab_active(&self) -> Style {
        Style::default()
            .bg(BG.resolve(self.support))
            .fg(FG.resolve(self.support))
            .add_modifier(Modifier::BOLD)
    }

    pub fn tab_inactive(&self) -> Style {
        Style::default()
            .bg(BG_ALT.resolve(self.support))
            .fg(FG_DIM.resolve(self.support))
    }

    /// Level 1: resting pane border.
    pub fn border_inactive(&self) -> (Style, border::Set) {
        (
            Style::default().fg(OUTLINE.resolve(self.support)),
            border::PLAIN,
        )
    }

    /// Level 2: the one focused pane - thick border in outline-focus.
    pub fn border_focus(&self) -> (Style, border::Set) {
        (
            Style::default()
                .fg(OUTLINE_FOCUS.resolve(self.support))
                .add_modifier(Modifier::BOLD),
            border::THICK,
        )
    }

    /// Gates, modals, destructive confirms: double border, stop-and-look.
    pub fn border_emphasis(&self) -> (Style, border::Set) {
        (
            Style::default().fg(ACCENT.resolve(self.support)),
            border::DOUBLE,
        )
    }
}

/// Everything the single reserved bottom line displays.
#[derive(Clone, Debug, Default)]
pub struct StatuslineState {
    pub mode: String,
    pub cwd: String,
    pub workspace: String,
    pub agent: String,
    pub debug: String,
}

/// Render `mode · cwd · workspace · agent state · debug/replay state`.
pub fn statusline(theme: &Theme, s: &StatuslineState) -> Line<'static> {
    let sep = Span::styled(" · ".to_string(), theme.muted());
    let mut spans = vec![
        Span::styled(
            "◆ ".to_string(),
            Style::default().fg(PRIMARY.resolve(theme.support)),
        ),
        Span::styled(s.mode.clone(), theme.title()),
        sep.clone(),
        Span::styled(s.cwd.clone(), theme.text()),
        sep.clone(),
        Span::styled(
            s.workspace.clone(),
            Style::default().fg(PRIMARY.resolve(theme.support)),
        ),
    ];
    if !s.agent.is_empty() {
        spans.push(sep.clone());
        spans.push(Span::styled(
            s.agent.clone(),
            Style::default().fg(SUCCESS.resolve(theme.support)),
        ));
    }
    if !s.debug.is_empty() {
        spans.push(sep);
        spans.push(Span::styled(s.debug.clone(), theme.gated()));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_resolves_per_support_level() {
        assert_eq!(
            PRIMARY.resolve(ColorSupport::TrueColor),
            Color::Rgb(0x4b, 0x4c, 0xff)
        );
        assert_eq!(PRIMARY.resolve(ColorSupport::Ansi256), Color::Indexed(63));
        assert_eq!(PRIMARY.resolve(ColorSupport::Ansi16), Color::LightBlue);
    }

    #[test]
    fn focus_border_is_thick_and_bold_so_it_survives_monochrome() {
        let theme = Theme::new(ColorSupport::Ansi16);
        let (style, set) = theme.border_focus();
        assert!(style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(set.top_left, border::THICK.top_left);
    }

    #[test]
    fn statusline_includes_all_populated_segments() {
        let theme = Theme::new(ColorSupport::TrueColor);
        let line = statusline(
            &theme,
            &StatuslineState {
                mode: "NORMAL".into(),
                cwd: "~/proj".into(),
                workspace: "personal".into(),
                agent: "agent: idle".into(),
                debug: "replay @ e42".into(),
            },
        );
        let text: String = line.spans.iter().map(|s| s.content.clone()).collect();
        assert!(text.contains("NORMAL") && text.contains("~/proj") && text.contains("personal"));
        assert!(text.contains("agent: idle") && text.contains("replay @ e42"));
    }

    #[test]
    fn empty_agent_and_debug_segments_are_omitted() {
        let theme = Theme::new(ColorSupport::TrueColor);
        let line = statusline(
            &theme,
            &StatuslineState {
                mode: "NORMAL".into(),
                cwd: "/".into(),
                workspace: "ws".into(),
                ..Default::default()
            },
        );
        let text: String = line.spans.iter().map(|s| s.content.clone()).collect();
        assert_eq!(text.matches(" · ").count(), 2);
    }
}
