//! The keymap: one table, three consumers.
//!
//! [`KEYMAP`] is the only place a key is named. [`decode`] is a lookup over it,
//! the help overlay renders it, and the command palette asks it for the chord of
//! an action. A key that is not in the table does not exist, so the help cannot
//! advertise a binding that does not work and the palette cannot show a stale
//! one — `every_table_row_decodes_to_its_own_action` asserts that every row
//! decodes to its own action.
//!
//! Rows are matched in order and a row's modifiers are a *subset* of the event's,
//! so `Alt+Up` must sit before `Up` and `Shift+Up` before `Up`; `Shift` is
//! significant only for function keys (`Shift+F3` is not `F3`), while
//! `Ctrl+Shift+H` is still `^H`.
//!
//! Keys that are modal in nano decode to one `Action` and are interpreted by the
//! focus: `M-B` is "backwards" everywhere (repeat the search outside a prompt,
//! toggle the direction inside one), and `M-R` toggles regexp inside a prompt
//! while outside it opens Replace. `Enter` (`Newline`) confirms inside a prompt
//! or a panel; `Esc` (`Cancel`) closes an overlay and clears the mark.
//!
//! `^H` opens the help reference; Backspace is the Backspace key (0x7F). A
//! terminal that sends 0x08 for Backspace therefore opens help instead of
//! deleting, which is documented in `AGENTS.md` and `docs/*/shortcuts.md`.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nnano_core::editor::Action;

/// Section of the reference. The order of [`GROUPS`] is the order on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Files,
    Editing,
    Move,
    Search,
    Interface,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Files => "Files",
            Group::Editing => "Editing",
            Group::Move => "Move",
            Group::Search => "Search",
            Group::Interface => "Interface",
        }
    }
}

/// Headings of the help overlay, in order.
pub const GROUPS: &[Group] = &[
    Group::Files,
    Group::Editing,
    Group::Move,
    Group::Search,
    Group::Interface,
];

/// One chord: the key plus the modifiers that have to be held for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl Chord {
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    /// As the user types it: `Ctrl+S`, `Alt+,`, `Shift+F3`, `Backspace`.
    pub fn label(self) -> String {
        let mut label = String::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            label.push_str("Ctrl+");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            label.push_str("Alt+");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            label.push_str("Shift+");
        }
        match self.code {
            // Modifier chords arrive lower-case (`Ctrl+s`); the docs and the
            // help write them upper-case (`Ctrl+S`).
            KeyCode::Char(ch) => label.extend(ch.to_uppercase()),
            KeyCode::F(n) => label.push_str(&format!("F{n}")),
            KeyCode::Enter => label.push_str("Enter"),
            KeyCode::Tab => label.push_str("Tab"),
            KeyCode::Backspace => label.push_str("Backspace"),
            KeyCode::Delete => label.push_str("Delete"),
            KeyCode::Esc => label.push_str("Esc"),
            KeyCode::Home => label.push_str("Home"),
            KeyCode::End => label.push_str("End"),
            KeyCode::PageUp => label.push_str("PgUp"),
            KeyCode::PageDown => label.push_str("PgDn"),
            KeyCode::Up => label.push_str("Up"),
            KeyCode::Down => label.push_str("Down"),
            KeyCode::Left => label.push_str("Left"),
            KeyCode::Right => label.push_str("Right"),
            _ => {}
        }
        label
    }

    /// True when `key` is this chord: the row's modifiers are a subset of the
    /// event's, and `Shift` only counts apart on function keys.
    fn matches(self, key: &KeyEvent) -> bool {
        let same_key = match (self.code, key.code) {
            (KeyCode::Char(row), KeyCode::Char(event)) => row.eq_ignore_ascii_case(&event),
            (row, event) => row == event,
        };
        if !same_key || !key.modifiers.contains(self.modifiers) {
            return false;
        }
        if matches!(self.code, KeyCode::F(_)) {
            return key.modifiers.contains(KeyModifiers::SHIFT)
                == self.modifiers.contains(KeyModifiers::SHIFT);
        }
        true
    }
}

/// One row of the keymap. `label` is what the help overlay prints, so a binding
/// is documented where it is declared.
#[derive(Debug)]
pub struct Binding {
    pub chord: Chord,
    pub action: Action,
    pub group: Group,
    pub label: &'static str,
}

const fn bind(
    code: KeyCode,
    modifiers: KeyModifiers,
    action: Action,
    group: Group,
    label: &'static str,
) -> Binding {
    Binding {
        chord: Chord::new(code, modifiers),
        action,
        group,
        label,
    }
}

/// Every binding the editor has today. The order inside a group is the order on
/// screen, and the order across groups only matters where two chords overlap
/// (`Alt+Up` before `Up`).
pub const KEYMAP: &[Binding] = &[
    // Files
    bind(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        Action::Save,
        Group::Files,
        "Save in place",
    ),
    bind(
        KeyCode::Char('q'),
        KeyModifiers::CONTROL,
        Action::Quit,
        Group::Files,
        "Quit (asks once if modified)",
    ),
    bind(
        KeyCode::Char('n'),
        KeyModifiers::CONTROL,
        Action::NewBuffer,
        Group::Files,
        "New buffer",
    ),
    bind(
        KeyCode::Char('t'),
        KeyModifiers::CONTROL,
        Action::NewTab,
        Group::Files,
        "New tab (a buffer)",
    ),
    bind(
        KeyCode::Char('w'),
        KeyModifiers::CONTROL,
        Action::CloseBuffer,
        Group::Files,
        "Close buffer (asks once if modified)",
    ),
    bind(
        KeyCode::Char('s'),
        KeyModifiers::ALT,
        Action::SaveAs,
        Group::Files,
        "Save as: ask for a path, then write",
    ),
    bind(
        KeyCode::Char('o'),
        KeyModifiers::CONTROL,
        Action::OpenPath,
        Group::Files,
        "Open a file by path (Tab completes)",
    ),
    bind(
        KeyCode::Char('r'),
        KeyModifiers::CONTROL,
        Action::Reload,
        Group::Files,
        "Reload from disk (asks once if modified)",
    ),
    bind(
        KeyCode::Char('i'),
        KeyModifiers::ALT,
        Action::InsertFile,
        Group::Files,
        "Insert a file at the cursor (Tab completes)",
    ),
    // Editing
    bind(
        KeyCode::Char('z'),
        KeyModifiers::CONTROL,
        Action::Undo,
        Group::Editing,
        "Undo",
    ),
    bind(
        KeyCode::Char('u'),
        KeyModifiers::ALT,
        Action::Undo,
        Group::Editing,
        "Undo (nano alias)",
    ),
    bind(
        KeyCode::Char('y'),
        KeyModifiers::CONTROL,
        Action::Redo,
        Group::Editing,
        "Redo",
    ),
    bind(
        KeyCode::Char('e'),
        KeyModifiers::ALT,
        Action::Redo,
        Group::Editing,
        "Redo (nano alias)",
    ),
    bind(
        KeyCode::Char('k'),
        KeyModifiers::CONTROL,
        Action::CutLine,
        Group::Editing,
        "Cut the line (repeats accumulate)",
    ),
    bind(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
        Action::Paste,
        Group::Editing,
        "Paste the register (nano alias)",
    ),
    bind(
        KeyCode::Char('v'),
        KeyModifiers::CONTROL,
        Action::Paste,
        Group::Editing,
        "Paste the register",
    ),
    bind(
        KeyCode::Char('x'),
        KeyModifiers::CONTROL,
        Action::Cut,
        Group::Editing,
        "Cut the mark, or the line",
    ),
    bind(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
        Action::Copy,
        Group::Editing,
        "Copy the mark, or the line",
    ),
    bind(
        KeyCode::Char('6'),
        KeyModifiers::ALT,
        Action::Copy,
        Group::Editing,
        "Copy the mark",
    ),
    bind(
        KeyCode::Char('a'),
        KeyModifiers::CONTROL,
        Action::SelectAll,
        Group::Editing,
        "Select the whole buffer",
    ),
    bind(
        KeyCode::Char('a'),
        KeyModifiers::ALT,
        Action::StartMark,
        Group::Editing,
        "Start or clear the mark",
    ),
    bind(
        KeyCode::Char('z'),
        KeyModifiers::ALT,
        Action::ToggleBlockSelection,
        Group::Editing,
        "Rectangular mark (Alt+drag too)",
    ),
    bind(
        KeyCode::Char('d'),
        KeyModifiers::CONTROL,
        Action::Delete,
        Group::Editing,
        "Delete the character under the cursor",
    ),
    bind(
        KeyCode::Char('\\'),
        KeyModifiers::CONTROL,
        Action::OpenReplace,
        Group::Editing,
        "Replace",
    ),
    bind(
        KeyCode::Char('j'),
        KeyModifiers::CONTROL,
        Action::Justify,
        Group::Editing,
        "Justify the paragraph, or the marked lines",
    ),
    bind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        Action::Newline,
        Group::Editing,
        "New line; accepts a prompt or a row",
    ),
    bind(
        KeyCode::Tab,
        KeyModifiers::NONE,
        Action::InsertTab,
        Group::Editing,
        "Insert a tab",
    ),
    bind(
        KeyCode::Backspace,
        KeyModifiers::NONE,
        Action::Backspace,
        Group::Editing,
        "Delete the character before the cursor",
    ),
    bind(
        KeyCode::Delete,
        KeyModifiers::NONE,
        Action::Delete,
        Group::Editing,
        "Delete the character under the cursor",
    ),
    // Move. `Alt+Up`/`Alt+Down` are nano's page keys and must precede the plain
    // arrows: a row matches when its modifiers are a subset of the event's.
    bind(
        KeyCode::Up,
        KeyModifiers::ALT,
        Action::PageUp,
        Group::Move,
        "One page up",
    ),
    bind(
        KeyCode::Down,
        KeyModifiers::ALT,
        Action::PageDown,
        Group::Move,
        "One page down",
    ),
    // Word movement must precede the plain arrows for the same reason: a row
    // matches when its modifiers are a subset of the event's. `Ctrl+Shift+←`
    // therefore stays a word move instead of becoming a character selection.
    bind(
        KeyCode::Left,
        KeyModifiers::CONTROL,
        Action::MoveWordLeft,
        Group::Move,
        "One word left",
    ),
    bind(
        KeyCode::Right,
        KeyModifiers::CONTROL,
        Action::MoveWordRight,
        Group::Move,
        "One word right",
    ),
    bind(
        KeyCode::Char('g'),
        KeyModifiers::CONTROL,
        Action::Goto,
        Group::Move,
        "Go to line (and optional column)",
    ),
    bind(
        KeyCode::Char('g'),
        KeyModifiers::ALT,
        Action::Goto,
        Group::Move,
        "Go to line (nano)",
    ),
    // Shift+arrows start the mark and extend it, as in a desktop editor. They
    // must precede the plain arrows: a row matches when its modifiers are a
    // subset of the event's, so `Shift+Left` would otherwise land on
    // `MoveLeft`.
    bind(
        KeyCode::Left,
        KeyModifiers::SHIFT,
        Action::SelectLeft,
        Group::Move,
        "Extend the mark one character left",
    ),
    bind(
        KeyCode::Right,
        KeyModifiers::SHIFT,
        Action::SelectRight,
        Group::Move,
        "Extend the mark one character right",
    ),
    bind(
        KeyCode::Left,
        KeyModifiers::NONE,
        Action::MoveLeft,
        Group::Move,
        "One character left",
    ),
    bind(
        KeyCode::Right,
        KeyModifiers::NONE,
        Action::MoveRight,
        Group::Move,
        "One character right",
    ),
    // `Shift+↑`/`Shift+↓` extend the mark by whole lines, keeping the wanted
    // visual column; they need the same precedence for the same reason.
    bind(
        KeyCode::Up,
        KeyModifiers::SHIFT,
        Action::SelectUp,
        Group::Move,
        "Extend the mark one line up",
    ),
    bind(
        KeyCode::Down,
        KeyModifiers::SHIFT,
        Action::SelectDown,
        Group::Move,
        "Extend the mark one line down",
    ),
    bind(
        KeyCode::Up,
        KeyModifiers::NONE,
        Action::MoveUp,
        Group::Move,
        "One line up",
    ),
    bind(
        KeyCode::Down,
        KeyModifiers::NONE,
        Action::MoveDown,
        Group::Move,
        "One line down",
    ),
    bind(
        KeyCode::Home,
        KeyModifiers::NONE,
        Action::MoveLineStart,
        Group::Move,
        "Start of line",
    ),
    bind(
        KeyCode::End,
        KeyModifiers::NONE,
        Action::MoveLineEnd,
        Group::Move,
        "End of line",
    ),
    bind(
        KeyCode::Char('b'),
        KeyModifiers::CONTROL,
        Action::MoveLeft,
        Group::Move,
        "Back one character (nano)",
    ),
    bind(
        KeyCode::Char('e'),
        KeyModifiers::CONTROL,
        Action::MoveLineEnd,
        Group::Move,
        "End of line (nano)",
    ),
    bind(
        KeyCode::PageUp,
        KeyModifiers::NONE,
        Action::PageUp,
        Group::Move,
        "One page up",
    ),
    bind(
        KeyCode::PageDown,
        KeyModifiers::NONE,
        Action::PageDown,
        Group::Move,
        "One page down",
    ),
    // Search
    bind(
        KeyCode::Char('f'),
        KeyModifiers::CONTROL,
        Action::OpenFind,
        Group::Search,
        "Find",
    ),
    bind(
        KeyCode::F(3),
        KeyModifiers::NONE,
        Action::FindNext,
        Group::Search,
        "Find next",
    ),
    bind(
        KeyCode::Char('w'),
        KeyModifiers::ALT,
        Action::FindNext,
        Group::Search,
        "Find next (nano)",
    ),
    bind(
        KeyCode::F(3),
        KeyModifiers::SHIFT,
        Action::FindPrevious,
        Group::Search,
        "Find previous",
    ),
    bind(
        KeyCode::Char('b'),
        KeyModifiers::ALT,
        Action::ToggleBackwards,
        Group::Search,
        "Find backwards; direction in a prompt",
    ),
    bind(
        KeyCode::Char('c'),
        KeyModifiers::ALT,
        Action::ToggleCase,
        Group::Search,
        "Case sensitivity (in the prompt)",
    ),
    bind(
        KeyCode::Char('r'),
        KeyModifiers::ALT,
        Action::ToggleRegexp,
        Group::Search,
        "Regexp (in a prompt) / replace (outside)",
    ),
    // Interface
    bind(
        KeyCode::Char('h'),
        KeyModifiers::CONTROL,
        Action::OpenHelp,
        Group::Interface,
        "This help",
    ),
    bind(
        KeyCode::F(1),
        KeyModifiers::NONE,
        Action::OpenHelp,
        Group::Interface,
        "This help",
    ),
    bind(
        KeyCode::Char('p'),
        KeyModifiers::CONTROL,
        Action::OpenPalette,
        Group::Interface,
        "Command palette",
    ),
    bind(
        KeyCode::Char(','),
        KeyModifiers::ALT,
        Action::OpenSettings,
        Group::Interface,
        "Settings",
    ),
    bind(
        KeyCode::Char('y'),
        KeyModifiers::ALT,
        Action::ToggleSyntax,
        Group::Interface,
        "Syntax highlighting on/off",
    ),
    bind(
        KeyCode::Char('x'),
        KeyModifiers::ALT,
        Action::Execute,
        Group::Interface,
        "Run a shell command and insert its output",
    ),
    bind(
        KeyCode::Char('l'),
        KeyModifiers::CONTROL,
        Action::Redraw,
        Group::Interface,
        "Repaint the screen",
    ),
    bind(
        KeyCode::Esc,
        KeyModifiers::NONE,
        Action::Cancel,
        Group::Interface,
        "Close a prompt or panel; clear the mark",
    ),
];

/// Decode a press event; releases, repeats and unknown keys yield `None`.
pub fn decode(key: &KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    if let Some(binding) = KEYMAP.iter().find(|binding| binding.chord.matches(key)) {
        return Some(binding.action.clone());
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char(ch) if !ctrl && !alt => Some(Action::Insert(ch)),
        _ => None,
    }
}

/// The chord a surface should advertise for `action` (first row wins). The
/// command palette uses it instead of carrying its own copy of the keymap.
pub fn chord_for(action: &Action) -> Option<Chord> {
    KEYMAP
        .iter()
        .find(|binding| &binding.action == action)
        .map(|binding| binding.chord)
}

/// Rows of one section, in table order.
pub fn bindings(group: Group) -> impl Iterator<Item = &'static Binding> {
    KEYMAP.iter().filter(move |binding| binding.group == group)
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use nnano_core::editor::Action;

    use super::{Chord, GROUPS, KEYMAP, bindings, chord_for, decode};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn plain(code: KeyCode) -> KeyEvent {
        key(code, KeyModifiers::NONE)
    }

    #[test]
    fn every_table_row_decodes_to_its_own_action() {
        // The drift test: a row on screen that does not work is impossible.
        for binding in KEYMAP {
            let event = KeyEvent::new(binding.chord.code, binding.chord.modifiers);
            assert_eq!(
                decode(&event).as_ref(),
                Some(&binding.action),
                "{} should be {:?}",
                binding.chord.label(),
                binding.action
            );
        }
    }

    #[test]
    fn no_chord_is_declared_twice() {
        for (index, binding) in KEYMAP.iter().enumerate() {
            for other in &KEYMAP[index + 1..] {
                assert_ne!(
                    binding.chord,
                    other.chord,
                    "{} is declared twice",
                    binding.chord.label()
                );
            }
        }
    }

    #[test]
    fn every_binding_has_a_section() {
        let counted: usize = GROUPS.iter().map(|group| bindings(*group).count()).sum();
        assert_eq!(counted, KEYMAP.len());
    }

    #[test]
    fn the_table_covers_the_original_keymap() {
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
            (key(KeyCode::Up, KeyModifiers::SHIFT), Action::SelectUp),
            (key(KeyCode::Down, KeyModifiers::SHIFT), Action::SelectDown),
            (key(KeyCode::Left, KeyModifiers::SHIFT), Action::SelectLeft),
            (
                key(KeyCode::Right, KeyModifiers::SHIFT),
                Action::SelectRight,
            ),
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
            (
                key(KeyCode::Char('f'), KeyModifiers::CONTROL),
                Action::OpenFind,
            ),
            (
                key(KeyCode::Char('p'), KeyModifiers::CONTROL),
                Action::OpenPalette,
            ),
            (
                key(KeyCode::Char(','), KeyModifiers::ALT),
                Action::OpenSettings,
            ),
            (key(KeyCode::Char('w'), KeyModifiers::ALT), Action::FindNext),
            (
                key(KeyCode::Char('b'), KeyModifiers::ALT),
                Action::ToggleBackwards,
            ),
            (
                key(KeyCode::Char('c'), KeyModifiers::ALT),
                Action::ToggleCase,
            ),
            (
                key(KeyCode::Char('r'), KeyModifiers::ALT),
                Action::ToggleRegexp,
            ),
            (plain(KeyCode::F(1)), Action::OpenHelp),
            (plain(KeyCode::F(3)), Action::FindNext),
            (
                key(KeyCode::F(3), KeyModifiers::SHIFT),
                Action::FindPrevious,
            ),
            (plain(KeyCode::Esc), Action::Cancel),
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
    fn caret_h_opens_help_and_the_backspace_key_still_deletes() {
        assert_eq!(
            decode(&key(KeyCode::Char('h'), KeyModifiers::CONTROL)),
            Some(Action::OpenHelp)
        );
        // Shift+Ctrl+H arrives upper-cased on some terminals.
        assert_eq!(
            decode(&key(KeyCode::Char('H'), KeyModifiers::CONTROL)),
            Some(Action::OpenHelp)
        );
        // The Backspace key (0x7F) is the delete path now.
        assert_eq!(decode(&plain(KeyCode::Backspace)), Some(Action::Backspace));
    }

    #[test]
    fn shift_still_inserts_upper_case() {
        assert_eq!(
            decode(&key(KeyCode::Char('S'), KeyModifiers::SHIFT)),
            Some(Action::Insert('S'))
        );
    }

    #[test]
    fn deferred_rows_decode_to_nothing() {
        // A key that is not in the table is inert rather than a silent no-op
        // variant, so this list only shrinks when a feature lands.
        let event = key(KeyCode::Char('q'), KeyModifiers::ALT);
        assert_eq!(decode(&event), None, "{event:?}");
    }

    #[test]
    fn the_file_goto_and_word_rows_are_live() {
        for (event, expected) in [
            (key(KeyCode::Char('s'), KeyModifiers::ALT), Action::SaveAs),
            (
                key(KeyCode::Char('o'), KeyModifiers::CONTROL),
                Action::OpenPath,
            ),
            (
                key(KeyCode::Char('r'), KeyModifiers::CONTROL),
                Action::Reload,
            ),
            (
                key(KeyCode::Char('i'), KeyModifiers::ALT),
                Action::InsertFile,
            ),
            (key(KeyCode::Char('g'), KeyModifiers::CONTROL), Action::Goto),
            (key(KeyCode::Char('g'), KeyModifiers::ALT), Action::Goto),
            (
                key(KeyCode::Char('j'), KeyModifiers::CONTROL),
                Action::Justify,
            ),
            (key(KeyCode::Char('x'), KeyModifiers::ALT), Action::Execute),
            (
                key(KeyCode::Left, KeyModifiers::CONTROL),
                Action::MoveWordLeft,
            ),
            (
                key(KeyCode::Right, KeyModifiers::CONTROL),
                Action::MoveWordRight,
            ),
        ] {
            assert_eq!(decode(&event).as_ref(), Some(&expected), "{event:?}");
        }
        // The plain arrows still move one char and one line.
        assert_eq!(decode(&plain(KeyCode::Left)), Some(Action::MoveLeft));
        assert_eq!(decode(&plain(KeyCode::Right)), Some(Action::MoveRight));
        // Ctrl+Shift+Left is still a word move (modifiers are a subset rule).
        assert_eq!(
            decode(&key(
                KeyCode::Left,
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )),
            Some(Action::MoveWordLeft)
        );
    }

    #[test]
    fn release_and_repeat_events_are_ignored() {
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            let event = KeyEvent::new_with_kind(KeyCode::Char('a'), KeyModifiers::NONE, kind);
            assert_eq!(decode(&event), None, "{kind:?}");
        }
    }

    #[test]
    fn labels_read_like_the_docs() {
        assert_eq!(
            Chord::new(KeyCode::Char('s'), KeyModifiers::CONTROL).label(),
            "Ctrl+S"
        );
        assert_eq!(
            Chord::new(KeyCode::Char(','), KeyModifiers::ALT).label(),
            "Alt+,"
        );
        assert_eq!(
            Chord::new(KeyCode::F(3), KeyModifiers::SHIFT).label(),
            "Shift+F3"
        );
        assert_eq!(Chord::new(KeyCode::F(1), KeyModifiers::NONE).label(), "F1");
        assert_eq!(
            Chord::new(KeyCode::Enter, KeyModifiers::NONE).label(),
            "Enter"
        );
    }

    #[test]
    fn the_palette_chords_come_from_the_table() {
        // An action with two chords advertises the one declared first.
        assert_eq!(
            chord_for(&Action::Undo).map(|chord| chord.label()),
            Some("Ctrl+Z".to_string())
        );
        assert_eq!(
            chord_for(&Action::OpenHelp).map(|chord| chord.label()),
            Some("Ctrl+H".to_string())
        );
        // Actions that have no chord (mouse, palette rows) answer `None`.
        assert_eq!(chord_for(&Action::SwitchBuffer(0)), None);
    }
}
