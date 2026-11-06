//! The command palette (`^P`).
//!
//! Rows are the editor commands followed by the open buffers. The typed filter
//! narrows both. The palette never walks the filesystem: the buffer list is what
//! `App` already has open.

use nnano_core::editor::Action;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::popup_frame;
use super::prompt::{Prompt, prompt_width};

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
        label: "New buffer",
        action: Action::NewBuffer,
    },
    Command {
        label: "Close buffer",
        action: Action::CloseBuffer,
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
    pub fn label(&self, buffers: &[String]) -> String {
        match self {
            Row::Command(command) => command.label.to_string(),
            Row::Buffer(index) => format!(
                "Buffer: {}",
                buffers.get(*index).cloned().unwrap_or_default()
            ),
        }
    }
}

/// Filter text, cursor and the highlighted row.
#[derive(Debug, Default)]
pub struct Palette {
    pub prompt: Prompt,
    selected: usize,
}

impl Palette {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Put the highlight on `index` (mouse click).
    pub fn select(&mut self, index: usize) {
        self.selected = index;
    }

    /// Commands matching the filter, then the matching buffers.
    pub fn rows(&self, buffers: &[String]) -> Vec<Row> {
        let filter = self.prompt.text().to_lowercase();
        let mut rows: Vec<Row> = COMMANDS
            .iter()
            .filter(|command| command.label.to_lowercase().contains(&filter))
            .map(Row::Command)
            .collect();
        for (index, name) in buffers.iter().enumerate() {
            if format!("buffer: {name}").contains(&filter) {
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

/// Floating palette: filter on top, matching rows below, cursor in the filter.
pub fn draw(frame: &mut Frame, area: Rect, palette: &Palette, buffers: &[String]) {
    let rows = palette.rows(buffers);
    let height = u16::try_from(rows.len() + 3).unwrap_or(u16::MAX);
    let inner = popup_frame(frame, area, 44, height, "Commands");

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
            Line::from(Span::styled(format!("  {}", row.label(buffers)), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

#[cfg(test)]
mod tests {
    use nnano_core::editor::Action;

    use super::{COMMANDS, Palette, Row};

    fn buffers() -> Vec<String> {
        vec!["uno.txt".to_string(), "dos.txt".to_string()]
    }

    #[test]
    fn the_filter_narrows_the_rows() {
        let mut palette = Palette::new();
        for ch in "fin".chars() {
            assert!(palette.edit(Action::Insert(ch)));
        }
        let labels: Vec<String> = palette
            .rows(&buffers())
            .iter()
            .map(|row| row.label(&buffers()))
            .collect();
        assert_eq!(labels, vec!["Find", "Find next", "Find previous"]);
    }

    #[test]
    fn the_filter_is_case_insensitive_and_empty_matches_everything() {
        let mut palette = Palette::new();
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
        let palette = Palette::new();
        let rows = palette.rows(&buffers());
        assert!(matches!(rows.last(), Some(Row::Buffer(1))));
        assert!(matches!(rows[rows.len() - 2], Row::Buffer(0)));
    }

    #[test]
    fn filtering_by_buffer_selects_it() {
        let mut palette = Palette::new();
        for ch in "dos.txt".chars() {
            palette.edit(Action::Insert(ch));
        }
        let row = palette.selected_row(&buffers()).unwrap();
        assert!(matches!(row, Row::Buffer(1)));
    }

    #[test]
    fn editing_resets_the_selection_but_movement_clamps() {
        let mut palette = Palette::new();
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
        let mut palette = Palette::new();
        for ch in "zzz".chars() {
            palette.edit(Action::Insert(ch));
        }
        assert!(palette.rows(&buffers()).is_empty());
        assert!(palette.selected_row(&buffers()).is_none());
    }
}
