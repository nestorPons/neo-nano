//! The command palette (`^P`).
//!
//! Rows are the editor commands; the typed filter narrows them. The buffer list
//! joins this palette when tabs land (phase 7) — the palette still never walks the
//! filesystem, by decision.

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
#[derive(Debug, Clone, Copy)]
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

    /// Commands matching the filter, in registry order.
    pub fn rows(&self) -> Vec<&'static Command> {
        let filter = self.prompt.text().to_lowercase();
        COMMANDS
            .iter()
            .filter(|command| command.label.to_lowercase().contains(&filter))
            .collect()
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

    pub fn select_next(&mut self) {
        let last = self.rows().len().saturating_sub(1);
        self.selected = (self.selected + 1).min(last);
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_action(&self) -> Option<Action> {
        self.rows().get(self.selected).map(|command| command.action)
    }
}

/// Floating palette: filter on top, matching commands below, cursor in the filter.
pub fn draw(frame: &mut Frame, area: Rect, palette: &Palette) {
    let rows = palette.rows();
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
        .map(|(index, command)| {
            let style = if index == palette.selected() {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Line::from(Span::styled(format!("  {}", command.label), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

#[cfg(test)]
mod tests {
    use nnano_core::editor::Action;

    use super::{COMMANDS, Palette};

    #[test]
    fn the_filter_narrows_the_rows() {
        let mut palette = Palette::new();
        for ch in "fin".chars() {
            assert!(palette.edit(Action::Insert(ch)));
        }
        let labels: Vec<&str> = palette.rows().iter().map(|row| row.label).collect();
        assert_eq!(labels, vec!["Find", "Find next", "Find previous"]);
    }

    #[test]
    fn the_filter_is_case_insensitive_and_empty_matches_everything() {
        let mut palette = Palette::new();
        assert_eq!(palette.rows().len(), COMMANDS.len());
        for ch in "QUIT".chars() {
            palette.edit(Action::Insert(ch));
        }
        assert_eq!(palette.rows().len(), 1);
        assert_eq!(palette.selected_action(), Some(Action::Quit));
    }

    #[test]
    fn editing_resets_the_selection_but_movement_clamps() {
        let mut palette = Palette::new();
        palette.select_next();
        palette.select_next();
        assert_eq!(palette.selected(), 2);
        palette.edit(Action::Insert('s'));
        assert_eq!(palette.selected(), 0);
        palette.select_previous();
        assert_eq!(palette.selected(), 0);
        for _ in 0..20 {
            palette.select_next();
        }
        assert!(palette.selected() < palette.rows().len());
    }

    #[test]
    fn a_filter_with_no_match_has_no_action() {
        let mut palette = Palette::new();
        for ch in "zzz".chars() {
            palette.edit(Action::Insert(ch));
        }
        assert!(palette.rows().is_empty());
        assert_eq!(palette.selected_action(), None);
    }
}
