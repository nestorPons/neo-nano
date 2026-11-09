//! The command palette (`^P`).
//!
//! Rows are the editor commands followed by the open buffers. The typed filter
//! narrows both. The palette never walks the filesystem: the buffer list is what
//! `App` already has open.
//!
//! The chord on the right of each command is not stored here: it comes from
//! [`crate::term::input::chord_for`], the same table `decode` reads, so the
//! palette cannot advertise a key that does nothing. Typing a chord also filters
//! (`ctrl+q` finds Quit).

use nnano_core::editor::Action;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::popup_frame;
use super::prompt::{Prompt, prompt_width};
use crate::i18n::{Lang, t};
use crate::term::input::chord_for;

/// One selectable command. `Action` is the only mutation path, so the palette
/// cannot do anything the keymap cannot.
#[derive(Debug, Clone)]
pub struct Command {
    pub label: &'static str,
    pub action: Action,
}

pub const COMMANDS: &[Command] = &[
    Command {
        label: "Save file",
        action: Action::Save,
    },
    Command {
        label: "Save as",
        action: Action::SaveAs,
    },
    Command {
        label: "Open file",
        action: Action::OpenPath,
    },
    Command {
        label: "Reload from disk",
        action: Action::Reload,
    },
    Command {
        label: "Insert file",
        action: Action::InsertFile,
    },
    Command {
        label: "Find",
        action: Action::OpenFind,
    },
    Command {
        label: "Find next",
        action: Action::FindNext,
    },
    Command {
        label: "Find previous",
        action: Action::FindPrevious,
    },
    Command {
        label: "Replace",
        action: Action::OpenReplace,
    },
    Command {
        label: "Go to line",
        action: Action::Goto,
    },
    Command {
        label: "Justify paragraph",
        action: Action::Justify,
    },
    Command {
        label: "Execute command",
        action: Action::Execute,
    },
    Command {
        label: "New buffer",
        action: Action::NewBuffer,
    },
    Command {
        label: "Close buffer",
        action: Action::CloseBuffer,
    },
    Command {
        label: "Toggle syntax",
        action: Action::ToggleSyntax,
    },
    Command {
        label: "Settings",
        action: Action::OpenSettings,
    },
    Command {
        label: "Help",
        action: Action::OpenHelp,
    },
    Command {
        label: "Quit",
        action: Action::Quit,
    },
];

/// A selectable row: a command, or one of the open buffers.
#[derive(Debug, Clone)]
pub enum Row {
    Command(&'static Command),
    Buffer(usize),
}

impl Row {
    /// Text of the row. Command names and the `Buffer:` prefix are translated;
    /// the buffer's file name is not, because it is a path.
    pub fn label(&self, buffers: &[String], lang: Lang) -> String {
        match self {
            Row::Command(command) => t(lang, command.label).to_string(),
            Row::Buffer(index) => {
                let name = buffers.get(*index).cloned().unwrap_or_default();
                crate::i18n::tf(lang, "Buffer: {}", &[&name])
            }
        }
    }
}

/// Filter text, cursor, the highlighted row and the language to print the
/// command names in. The language is captured when the palette opens, which is
/// enough: changing it happens in the settings overlay, never at the same time.
#[derive(Debug, Default)]
pub struct Palette {
    pub prompt: Prompt,
    selected: usize,
    lang: Lang,
}

/// The chord assigned to a row, if it has one: commands do, buffers do not
/// (they are reached from the palette itself or the buffer bar).
fn chord_label(row: &Row) -> Option<String> {
    match row {
        Row::Command(command) => chord_for(&command.action).map(|chord| chord.label()),
        Row::Buffer(_) => None,
    }
}

impl Palette {
    pub fn new(lang: Lang) -> Self {
        Self {
            lang,
            ..Self::default()
        }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Put the highlight on `index` (mouse click).
    pub fn select(&mut self, index: usize) {
        self.selected = index;
    }

    /// Commands matching the filter, then the matching buffers. A command also
    /// matches its chord, so `ctrl+s` finds Save.
    pub fn rows(&self, buffers: &[String]) -> Vec<Row> {
        let filter = self.prompt.text().to_lowercase();
        let mut rows: Vec<Row> = COMMANDS
            .iter()
            .filter(|command| {
                // Both spellings match, so the palette is usable in another
                // language without hiding its own key names.
                t(self.lang, command.label).to_lowercase().contains(&filter)
                    || command.label.to_lowercase().contains(&filter)
                    || chord_for(&command.action)
                        .is_some_and(|chord| chord.label().to_lowercase().contains(&filter))
            })
            .map(Row::Command)
            .collect();
        for (index, name) in buffers.iter().enumerate() {
            if crate::i18n::tf(self.lang, "Buffer: {}", &[name])
                .to_lowercase()
                .contains(&filter)
                || format!("buffer: {name}").contains(&filter)
            {
                rows.push(Row::Buffer(index));
            }
        }
        rows
    }

    /// Text keys edit the filter and reset the selection, so the highlight never
    /// points at a row that the new filter removed.
    pub fn edit(&mut self, action: Action) -> bool {
        if !self.prompt.apply(action) {
            return false;
        }
        self.selected = 0;
        true
    }

    pub fn select_next(&mut self, buffers: &[String]) {
        let last = self.rows(buffers).len().saturating_sub(1);
        self.selected = (self.selected + 1).min(last);
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_row(&self, buffers: &[String]) -> Option<Row> {
        self.rows(buffers).get(self.selected).cloned()
    }
}

/// Panel inner area for the current row count; the mouse hit test needs the same
/// geometry the renderer used.
fn panel(area: Rect, rows: usize) -> Rect {
    let height = u16::try_from(rows + 3).unwrap_or(u16::MAX);
    super::panel_inner(area, 44, height)
}

/// Row under a screen row, for a click inside the panel.
pub fn row_at(area: Rect, y: u16, palette: &Palette, buffers: &[String]) -> Option<usize> {
    let rows = palette.rows(buffers).len();
    let inner = panel(area, rows);
    let first = inner.y.saturating_add(1);
    let offset = usize::from(y.checked_sub(first)?);
    (offset < rows).then_some(offset)
}

/// Left text clipped to `width` cells, so a long row never wraps and pushes the
/// rows under the mouse out of place.
fn clip(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let cells = prompt_width(&ch.to_string());
        if used + cells > width {
            break;
        }
        used += cells;
        out.push(ch);
    }
    out
}

/// One row: label on the left, its chord on the right, exactly `width` cells.
fn line_for(row: &Row, buffers: &[String], width: u16, style: Style, lang: Lang) -> Line<'static> {
    let label = format!("  {}", row.label(buffers, lang));
    let Some(keys) = chord_label(row) else {
        return Line::from(Span::styled(clip(&label, usize::from(width)), style));
    };
    let keys_width = prompt_width(&keys);
    let budget = usize::from(width).saturating_sub(keys_width + 1);
    let label = clip(&label, budget);
    let padding = usize::from(width)
        .saturating_sub(prompt_width(&label) + keys_width)
        .max(1);
    Line::from(vec![
        Span::styled(label, style),
        Span::raw(" ".repeat(padding)),
        Span::styled(keys, style.add_modifier(Modifier::DIM)),
    ])
}

/// Floating palette: filter on top, matching rows below with their chord on the
/// right, cursor in the filter.
pub fn draw(frame: &mut Frame, area: Rect, palette: &Palette, buffers: &[String]) {
    let rows = palette.rows(buffers);
    let height = u16::try_from(rows.len() + 3).unwrap_or(u16::MAX);
    let inner = popup_frame(frame, area, 44, height, t(palette.lang(), "Commands"));

    let [filter, list] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas::<2>(inner);

    let prefix = " > ";
    frame.render_widget(
        Paragraph::new(format!("{prefix}{}", palette.prompt.text()))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        filter,
    );
    let offset = prompt_width(prefix) + prompt_width(palette.prompt.before());
    let limit = usize::from(filter.width).saturating_sub(1);
    frame.set_cursor_position((
        filter.x + u16::try_from(offset.min(limit)).unwrap_or(0),
        filter.y,
    ));

    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let style = if index == palette.selected() {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            line_for(row, buffers, list.width, style, palette.lang())
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyEvent;
    use nnano_core::editor::Action;

    use super::{COMMANDS, Palette, Row, line_for};
    use crate::i18n::Lang;
    use crate::term::input::{chord_for, decode};

    fn buffers() -> Vec<String> {
        vec!["uno.txt".to_string(), "dos.txt".to_string()]
    }

    #[test]
    fn every_command_advertises_a_live_chord() {
        // The palette cannot show a key that does not run the command.
        for command in COMMANDS {
            let chord = chord_for(&command.action)
                .unwrap_or_else(|| panic!("{} has no chord in the keymap", command.label));
            assert_eq!(
                decode(&KeyEvent::new(chord.code, chord.modifiers)),
                Some(command.action.clone()),
                "{} advertises {}",
                command.label,
                chord.label()
            );
        }
    }

    #[test]
    fn the_filter_matches_a_chord() {
        let mut palette = Palette::new(Lang::En);
        for ch in "ctrl+q".chars() {
            assert!(palette.edit(Action::Insert(ch)));
        }
        let rows = palette.rows(&buffers());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label(&buffers(), Lang::En), "Quit");
    }

    #[test]
    fn the_chord_sits_on_the_right_edge_of_the_row() {
        let row = Row::Command(&COMMANDS[0]);
        let line = line_for(
            &row,
            &buffers(),
            40,
            ratatui::style::Style::default(),
            Lang::En,
        );
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.starts_with("  Save file"), "{text:?}");
        assert!(text.ends_with("Ctrl+S"), "{text:?}");
        assert_eq!(crate::ui::overlays::prompt::prompt_width(&text), 40);
    }

    #[test]
    fn buffer_rows_have_no_chord() {
        let row = Row::Buffer(0);
        let line = line_for(
            &row,
            &buffers(),
            40,
            ratatui::style::Style::default(),
            Lang::En,
        );
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "  Buffer: uno.txt");
    }

    #[test]
    fn the_filter_narrows_the_rows() {
        let mut palette = Palette::new(Lang::En);
        for ch in "fin".chars() {
            assert!(palette.edit(Action::Insert(ch)));
        }
        let labels: Vec<String> = palette
            .rows(&buffers())
            .iter()
            .map(|row| row.label(&buffers(), Lang::En))
            .collect();
        assert_eq!(labels, vec!["Find", "Find next", "Find previous"]);
    }

    #[test]
    fn the_filter_is_case_insensitive_and_empty_matches_everything() {
        let mut palette = Palette::new(Lang::En);
        assert_eq!(palette.rows(&buffers()).len(), COMMANDS.len() + 2);
        for ch in "QUIT".chars() {
            palette.edit(Action::Insert(ch));
        }
        assert_eq!(palette.rows(&buffers()).len(), 1);
        assert!(matches!(
            palette.selected_row(&buffers()),
            Some(Row::Command(_))
        ));
    }

    #[test]
    fn buffers_are_listed_after_the_commands() {
        let palette = Palette::new(Lang::En);
        let rows = palette.rows(&buffers());
        assert!(matches!(rows.last(), Some(Row::Buffer(1))));
        assert!(matches!(rows[rows.len() - 2], Row::Buffer(0)));
    }

    #[test]
    fn filtering_by_buffer_selects_it() {
        let mut palette = Palette::new(Lang::En);
        for ch in "dos.txt".chars() {
            palette.edit(Action::Insert(ch));
        }
        let row = palette.selected_row(&buffers()).unwrap();
        assert!(matches!(row, Row::Buffer(1)));
    }

    #[test]
    fn editing_resets_the_selection_but_movement_clamps() {
        let mut palette = Palette::new(Lang::En);
        palette.select_next(&buffers());
        palette.select_next(&buffers());
        assert_eq!(palette.selected(), 2);
        palette.edit(Action::Insert('s'));
        assert_eq!(palette.selected(), 0);
        palette.select_previous();
        assert_eq!(palette.selected(), 0);
        for _ in 0..50 {
            palette.select_next(&buffers());
        }
        assert!(palette.selected() < palette.rows(&buffers()).len());
    }

    #[test]
    fn a_filter_with_no_match_has_no_action() {
        let mut palette = Palette::new(Lang::En);
        for ch in "zzz".chars() {
            palette.edit(Action::Insert(ch));
        }
        assert!(palette.rows(&buffers()).is_empty());
        assert!(palette.selected_row(&buffers()).is_none());
    }
}
