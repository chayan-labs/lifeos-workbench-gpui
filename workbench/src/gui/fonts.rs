//! Font discovery for the window host (issue #27). Builds a full `FontSet`:
//! the user's preferred monospace face plus its bold/italic/bold-italic
//! variants (real faces, not synthesized), monospace fallbacks, and CJK /
//! symbol fallback faces so non-Latin text shapes instead of tofu-boxing.
//! Font bytes are leaked once at startup - they must outlive the glyph
//! atlas anyway. Faces above `MAX_FACE_BYTES` are skipped (Apple Color
//! Emoji alone is ~180MB resident, which would blow the weight budget);
//! color-emoji-at-scale is deferred to the renderer-v2 issue.

use fontdb::{Database, Style, Weight};
use ratatui_wgpu::Font;

/// Preference order when `WORKBENCH_FONT` is unset. First match wins.
pub const PREFERRED_FAMILIES: &[&str] = &[
    "JetBrains Mono",
    "JetBrainsMono Nerd Font Mono",
    "SF Mono",
    "Menlo",
    "Cascadia Mono",
    "Fira Code",
    "Monaco",
];

/// Non-monospace fallbacks worth shaping through: CJK + symbols + any
/// reasonably-sized emoji face.
const EXTRA_FALLBACK_FAMILIES: &[&str] = &[
    "PingFang SC",
    "Hiragino Sans",
    "Noto Sans Mono CJK SC",
    "Apple Symbols",
    "Noto Color Emoji",
    "Noto Emoji",
];

/// Faces larger than this are skipped to protect the resident-memory budget.
const MAX_FACE_BYTES: usize = 32 * 1024 * 1024;

/// Everything the renderer needs: the primary face, its style variants, and
/// the shaping-fallback chain.
pub struct FontSet {
    pub regular: Font<'static>,
    pub bold: Vec<Font<'static>>,
    pub italic: Vec<Font<'static>>,
    pub bold_italic: Vec<Font<'static>>,
    pub fallbacks: Vec<Font<'static>>,
}

/// Case-insensitive pick of the first preferred family present.
pub fn pick_family(available: &[String], preferred: &[&str]) -> Option<String> {
    preferred.iter().find_map(|want| {
        available
            .iter()
            .find(|have| have.eq_ignore_ascii_case(want))
            .cloned()
    })
}

/// Classify a face's style slot within its family.
/// Returns (is_bold, is_italic).
pub fn style_slot(weight: u16, style: Style) -> (bool, bool) {
    (weight >= Weight::SEMIBOLD.0, style != Style::Normal)
}

fn load_face(db: &Database, id: fontdb::ID) -> Option<Font<'static>> {
    let data = db.with_face_data(id, |d, _| {
        if d.len() > MAX_FACE_BYTES {
            None
        } else {
            Some(d.to_vec())
        }
    })??;
    let leaked: &'static [u8] = Box::leak(data.into_boxed_slice());
    Font::new(leaked)
}

/// Load the primary face, its bold/italic variants, and the fallback chain
/// from the system font database.
pub fn load_fonts() -> Result<FontSet, String> {
    let mut db = Database::new();
    db.load_system_fonts();

    let mono_families: Vec<String> = db
        .faces()
        .filter(|f| f.monospaced && f.index == 0)
        .filter_map(|f| f.families.first().map(|(name, _)| name.clone()))
        .collect();

    let env_font = std::env::var("WORKBENCH_FONT").ok();
    let mut preferred: Vec<&str> = Vec::new();
    if let Some(name) = env_font.as_deref() {
        preferred.push(name);
    }
    preferred.extend_from_slice(PREFERRED_FAMILIES);

    let primary_family = pick_family(&mono_families, &preferred)
        .or_else(|| mono_families.first().cloned())
        .ok_or("no monospace font found on this system")?;

    let mut regular: Option<Font<'static>> = None;
    let mut bold = Vec::new();
    let mut italic = Vec::new();
    let mut bold_italic = Vec::new();
    let mut fallbacks = Vec::new();
    let mut seen_families: Vec<String> = Vec::new();

    // Pass 1: every face of the primary family, routed to its style slot.
    for info in db.faces() {
        let Some((family, _)) = info.families.first() else {
            continue;
        };
        if *family != primary_family {
            continue;
        }
        let Some(font) = load_face(&db, info.id) else {
            continue;
        };
        match style_slot(info.weight.0, info.style) {
            (false, false) => {
                if regular.is_none() {
                    regular = Some(font);
                } else {
                    fallbacks.push(font);
                }
            }
            (true, false) => bold.push(font),
            (false, true) => italic.push(font),
            (true, true) => bold_italic.push(font),
        }
    }

    // Pass 2: one regular face per remaining monospace family.
    for info in db.faces() {
        if !info.monospaced || info.index != 0 || info.style != Style::Normal {
            continue;
        }
        let Some((family, _)) = info.families.first() else {
            continue;
        };
        if *family == primary_family || seen_families.iter().any(|s| s == family) {
            continue;
        }
        if let Some(font) = load_face(&db, info.id) {
            fallbacks.push(font);
            seen_families.push(family.clone());
        }
    }

    // Pass 3: CJK / symbol / emoji fallbacks (not monospaced; shaping only).
    for want in EXTRA_FALLBACK_FAMILIES {
        let Some(info) = db.faces().find(|f| {
            f.index == 0
                && f.families
                    .first()
                    .is_some_and(|(name, _)| name.eq_ignore_ascii_case(want))
        }) else {
            continue;
        };
        if let Some(font) = load_face(&db, info.id) {
            fallbacks.push(font);
        }
    }

    let regular = regular
        .or_else(|| bold.pop())
        .ok_or(format!("failed to load face for '{primary_family}'"))?;
    Ok(FontSet {
        regular,
        bold,
        italic,
        bold_italic,
        fallbacks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn avail(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn picks_first_preferred_family_present() {
        let a = avail(&["Menlo", "SF Mono", "Courier"]);
        assert_eq!(
            pick_family(&a, PREFERRED_FAMILIES),
            Some("SF Mono".to_string())
        );
    }

    #[test]
    fn pick_is_case_insensitive() {
        let a = avail(&["jetbrains mono"]);
        assert_eq!(
            pick_family(&a, PREFERRED_FAMILIES),
            Some("jetbrains mono".to_string())
        );
    }

    #[test]
    fn returns_none_when_nothing_matches() {
        let a = avail(&["Comic Sans MS"]);
        assert_eq!(pick_family(&a, PREFERRED_FAMILIES), None);
    }

    #[test]
    fn env_override_wins_over_defaults() {
        let a = avail(&["Menlo", "Hack"]);
        let mut preferred = vec!["Hack"];
        preferred.extend_from_slice(PREFERRED_FAMILIES);
        assert_eq!(pick_family(&a, &preferred), Some("Hack".to_string()));
    }

    #[test]
    fn style_slots_route_weight_and_italic() {
        assert_eq!(style_slot(Weight::NORMAL.0, Style::Normal), (false, false));
        assert_eq!(style_slot(Weight::BOLD.0, Style::Normal), (true, false));
        assert_eq!(style_slot(Weight::NORMAL.0, Style::Italic), (false, true));
        assert_eq!(style_slot(Weight::BOLD.0, Style::Oblique), (true, true));
        assert_eq!(style_slot(Weight::SEMIBOLD.0, Style::Normal), (true, false));
        assert_eq!(style_slot(Weight::LIGHT.0, Style::Normal), (false, false));
    }

    #[test]
    fn system_load_produces_a_primary_and_fallbacks() {
        // Runs against the real system database (macOS always has Menlo).
        let set = load_fonts().expect("system fonts");
        assert!(!set.fallbacks.is_empty());
    }
}
