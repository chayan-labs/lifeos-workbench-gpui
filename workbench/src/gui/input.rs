//! winit → crossterm event translation. The whole app (shell keymap, panes)
//! consumes crossterm-shaped events, so the window host speaks that dialect
//! too and everything above this seam runs unmodified.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Map winit modifier state onto crossterm's bitflags.
pub fn to_modifiers(mods: ModifiersState) -> KeyModifiers {
    let mut out = KeyModifiers::NONE;
    if mods.shift_key() {
        out |= KeyModifiers::SHIFT;
    }
    if mods.control_key() {
        out |= KeyModifiers::CONTROL;
    }
    if mods.alt_key() {
        out |= KeyModifiers::ALT;
    }
    if mods.super_key() {
        out |= KeyModifiers::SUPER;
    }
    out
}

/// Translate a pressed winit logical key into a crossterm key event.
/// Returns `None` for keys with no terminal-world equivalent (bare
/// modifiers, media keys, ...).
pub fn translate_key(logical: &Key, mods: ModifiersState) -> Option<Event> {
    let m = to_modifiers(mods);
    let code = match logical {
        Key::Character(s) => KeyCode::Char(s.chars().next()?),
        Key::Named(named) => match named {
            NamedKey::Enter => KeyCode::Enter,
            NamedKey::Tab => {
                if mods.shift_key() {
                    KeyCode::BackTab
                } else {
                    KeyCode::Tab
                }
            }
            NamedKey::Space => KeyCode::Char(' '),
            NamedKey::Backspace => KeyCode::Backspace,
            NamedKey::Escape => KeyCode::Esc,
            NamedKey::ArrowUp => KeyCode::Up,
            NamedKey::ArrowDown => KeyCode::Down,
            NamedKey::ArrowLeft => KeyCode::Left,
            NamedKey::ArrowRight => KeyCode::Right,
            NamedKey::Home => KeyCode::Home,
            NamedKey::End => KeyCode::End,
            NamedKey::PageUp => KeyCode::PageUp,
            NamedKey::PageDown => KeyCode::PageDown,
            NamedKey::Delete => KeyCode::Delete,
            NamedKey::Insert => KeyCode::Insert,
            NamedKey::F1 => KeyCode::F(1),
            NamedKey::F2 => KeyCode::F(2),
            NamedKey::F3 => KeyCode::F(3),
            NamedKey::F4 => KeyCode::F(4),
            NamedKey::F5 => KeyCode::F(5),
            NamedKey::F6 => KeyCode::F(6),
            NamedKey::F7 => KeyCode::F(7),
            NamedKey::F8 => KeyCode::F(8),
            NamedKey::F9 => KeyCode::F(9),
            NamedKey::F10 => KeyCode::F(10),
            NamedKey::F11 => KeyCode::F(11),
            NamedKey::F12 => KeyCode::F(12),
            _ => return None,
        },
        _ => return None,
    };
    Some(Event::Key(KeyEvent::new(code, m)))
}

/// Turn pasted / IME-committed text into key events for the focused pane
/// (issue #28). Newlines become Enter, tabs become Tab; carriage returns in
/// CRLF pairs collapse into a single Enter.
pub fn text_to_events(text: &str) -> Vec<Event> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        let code = match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                KeyCode::Enter
            }
            '\n' => KeyCode::Enter,
            '\t' => KeyCode::Tab,
            c => KeyCode::Char(c),
        };
        out.push(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::SmolStr;

    fn key(ev: Option<Event>) -> KeyEvent {
        match ev {
            Some(Event::Key(k)) => k,
            other => panic!("expected key event, got {other:?}"),
        }
    }

    #[test]
    fn translates_plain_character() {
        let k = key(translate_key(
            &Key::Character(SmolStr::new("a")),
            ModifiersState::empty(),
        ));
        assert_eq!(k.code, KeyCode::Char('a'));
        assert_eq!(k.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn carries_control_and_super_modifiers() {
        let k = key(translate_key(
            &Key::Character(SmolStr::new("s")),
            ModifiersState::CONTROL | ModifiersState::SUPER,
        ));
        assert_eq!(k.code, KeyCode::Char('s'));
        assert!(k.modifiers.contains(KeyModifiers::CONTROL));
        assert!(k.modifiers.contains(KeyModifiers::SUPER));
    }

    #[test]
    fn shift_tab_becomes_backtab() {
        let k = key(translate_key(
            &Key::Named(NamedKey::Tab),
            ModifiersState::SHIFT,
        ));
        assert_eq!(k.code, KeyCode::BackTab);
        assert!(k.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn named_keys_map_to_terminal_equivalents() {
        for (named, code) in [
            (NamedKey::Enter, KeyCode::Enter),
            (NamedKey::Escape, KeyCode::Esc),
            (NamedKey::Space, KeyCode::Char(' ')),
            (NamedKey::ArrowLeft, KeyCode::Left),
            (NamedKey::PageDown, KeyCode::PageDown),
            (NamedKey::F5, KeyCode::F(5)),
        ] {
            let k = key(translate_key(&Key::Named(named), ModifiersState::empty()));
            assert_eq!(k.code, code);
        }
    }

    #[test]
    fn text_to_events_maps_newlines_tabs_and_crlf() {
        let evs = text_to_events("ab\r\nc\td\n");
        let codes: Vec<KeyCode> = evs
            .iter()
            .map(|e| match e {
                Event::Key(k) => k.code,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            codes,
            vec![
                KeyCode::Char('a'),
                KeyCode::Char('b'),
                KeyCode::Enter,
                KeyCode::Char('c'),
                KeyCode::Tab,
                KeyCode::Char('d'),
                KeyCode::Enter,
            ]
        );
    }

    #[test]
    fn returns_none_for_bare_modifier_keys() {
        assert!(translate_key(&Key::Named(NamedKey::Shift), ModifiersState::SHIFT).is_none());
        assert!(translate_key(&Key::Named(NamedKey::Control), ModifiersState::CONTROL).is_none());
    }
}
