//! Key decoding: one crossterm event -> one [`Action`], or nothing.
//!
//! This is the only place a key is named. Rows of the v1 keymap that have no
//! feature yet (`^S` save, `^K` cut, `^O` open, …) deliberately decode to `None`
//! instead of a no-op action, and the deferred list below is asserted by a test so
//! that adding one is a deliberate change, not an accident.
//!
//! `^H` is Backspace, never Replace: Linux console and some `TERM`s send `^H` for
//! backspace, so it is accepted both as `KeyCode::Backspace` and as Ctrl+H.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nnano_core::editor::Action;

/// Decode a press event; releases, repeats and unknown keys yield `None`.
pub fn decode(key: &KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    if ctrl
        && !alt
        && let KeyCode::Char(raw) = key.code
    {
        return match raw.to_ascii_lowercase() {
            'q' => Some(Action::Quit),
            's' => Some(Action::Save),
            'h' => Some(Action::Backspace),
            'e' => Some(Action::MoveLineEnd),
            'b' => Some(Action::MoveLeft),
            'd' => Some(Action::Delete),
            'l' => Some(Action::Redraw),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Enter => Some(Action::Newline),
        KeyCode::Tab => Some(Action::InsertTab),
        KeyCode::Backspace => Some(Action::Backspace),
        KeyCode::Delete => Some(Action::Delete),
        KeyCode::Left => Some(Action::MoveLeft),
        KeyCode::Right => Some(Action::MoveRight),
        // nano's page keys moved to PgUp/PgDn; Alt+arrows are the aliases.
        KeyCode::Up if alt => Some(Action::PageUp),
        KeyCode::Down if alt => Some(Action::PageDown),
        KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Home => Some(Action::MoveLineStart),
        KeyCode::End => Some(Action::MoveLineEnd),
        KeyCode::PageUp => Some(Action::PageUp),
        KeyCode::PageDown => Some(Action::PageDown),
        KeyCode::Char(ch) if !ctrl && !alt => Some(Action::Insert(ch)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use nnano_core::editor::Action;

    use super::decode;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn plain(code: KeyCode) -> KeyEvent {
        key(code, KeyModifiers::NONE)
    }

    #[test]
    fn implemented_keymap_rows_decode() {
        let table: &[(KeyEvent, Action)] = &[
            (key(KeyCode::Char('q'), KeyModifiers::CONTROL), Action::Quit),
            (key(KeyCode::Char('s'), KeyModifiers::CONTROL), Action::Save),
            (
                key(KeyCode::Char('l'), KeyModifiers::CONTROL),
                Action::Redraw,
            ),
            (
                key(KeyCode::Char('b'), KeyModifiers::CONTROL),
                Action::MoveLeft,
            ),
            (
                key(KeyCode::Char('d'), KeyModifiers::CONTROL),
                Action::Delete,
            ),
            (
                key(KeyCode::Char('e'), KeyModifiers::CONTROL),
                Action::MoveLineEnd,
            ),
            (plain(KeyCode::Left), Action::MoveLeft),
            (plain(KeyCode::Right), Action::MoveRight),
            (plain(KeyCode::Up), Action::MoveUp),
            (plain(KeyCode::Down), Action::MoveDown),
            (plain(KeyCode::Home), Action::MoveLineStart),
            (plain(KeyCode::End), Action::MoveLineEnd),
            (plain(KeyCode::PageUp), Action::PageUp),
            (plain(KeyCode::PageDown), Action::PageDown),
            (key(KeyCode::Up, KeyModifiers::ALT), Action::PageUp),
            (key(KeyCode::Down, KeyModifiers::ALT), Action::PageDown),
            (plain(KeyCode::Enter), Action::Newline),
            (plain(KeyCode::Tab), Action::InsertTab),
            (plain(KeyCode::Backspace), Action::Backspace),
            (plain(KeyCode::Delete), Action::Delete),
            (plain(KeyCode::Char('a')), Action::Insert('a')),
            (
                key(KeyCode::Char('A'), KeyModifiers::SHIFT),
                Action::Insert('A'),
            ),
            (plain(KeyCode::Char('漢')), Action::Insert('漢')),
        ];

        for (event, expected) in table {
            assert_eq!(decode(event).as_ref(), Some(expected), "{event:?}");
        }
    }

    #[test]
    fn caret_h_is_backspace_not_replace() {
        assert_eq!(
            decode(&key(KeyCode::Char('h'), KeyModifiers::CONTROL)),
            Some(Action::Backspace)
        );
        // Shift+Ctrl+H arrives upper-cased on some terminals.
        assert_eq!(
            decode(&key(KeyCode::Char('H'), KeyModifiers::CONTROL)),
            Some(Action::Backspace)
        );
    }

    #[test]
    fn deferred_rows_decode_to_nothing() {
        // Each of these gains an action in its own phase; until then the key is
        // inert rather than a silent no-op variant.
        for (code, modifiers) in [
            (KeyCode::Char('o'), KeyModifiers::CONTROL), // open, phase 4
            (KeyCode::Char('k'), KeyModifiers::CONTROL), // cut line, phase 7
            (KeyCode::Char('u'), KeyModifiers::CONTROL), // paste line, phase 7
            (KeyCode::Char('z'), KeyModifiers::CONTROL), // undo, phase 7
            (KeyCode::Char('f'), KeyModifiers::CONTROL), // find, phase 7
            (KeyCode::Char('p'), KeyModifiers::CONTROL), // palette, phase 5
            (KeyCode::Esc, KeyModifiers::NONE),          // overlays, phase 5
            (KeyCode::Char('q'), KeyModifiers::ALT),     // not a binding
        ] {
            assert_eq!(
                decode(&key(code, modifiers)),
                None,
                "{code:?}/{modifiers:?}"
            );
        }
    }

    #[test]
    fn release_and_repeat_events_are_ignored() {
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            let event = KeyEvent::new_with_kind(KeyCode::Char('a'), KeyModifiers::NONE, kind);
            assert_eq!(decode(&event), None, "{kind:?}");
        }
    }
}
