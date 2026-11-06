//! The settings overlay: a live mirror of the option registry.
//!
//! Rows are built from [`OPTIONS`], values are read and written through `Config`,
//! so a new registry row appears here with no code change. Nothing is written to
//! disk: the overlay edits the session, matching "the commented TOML template is
//! written on demand".

use nnano_core::config::{Kind, OPTIONS, Opt, OptRow, Value};
use nnano_core::editor::Action;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::popup_frame;
use crate::App;

/// Selection index into `OPTIONS`, plus nothing else: every value lives in
/// `Config`, so the overlay cannot drift from what the editor uses.
#[derive(Debug, Default)]
pub struct Settings {
    selected: usize,
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_row(&self) -> &'static OptRow {
        &OPTIONS[self.selected.min(OPTIONS.len() - 1)]
    }

    /// Handle one key. Returns the option that changed, so the app loop can push
    /// it into the editor (tab width); `None` means nothing changed.
    ///
    /// `↑`/`↓` move, `←`/`→` and `Enter` change: booleans toggle, integers step
    /// inside their registry bounds.
    pub fn handle(
        &mut self,
        action: Action,
        config: &mut nnano_core::config::Config,
    ) -> Option<Opt> {
        match action {
            Action::MoveUp => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            Action::MoveDown => {
                self.selected = (self.selected + 1).min(OPTIONS.len() - 1);
                None
            }
            Action::Newline | Action::MoveLeft | Action::MoveRight => {
                let row = self.selected_row();
                let next = match (row.kind, action) {
                    (Kind::Bool, _) => Value::Bool(!config.bool(row.id)),
                    (Kind::Int { min, .. }, Action::MoveLeft) => {
                        Value::Int((config.int(row.id) - 1).max(min))
                    }
                    (Kind::Int { max, .. }, _) => Value::Int((config.int(row.id) + 1).min(max)),
                };
                config.set(row.id, next).ok().map(|()| row.id)
            }
            _ => None,
        }
    }
}

/// Floating panel with the option rows and the selected row's help text.
pub fn draw(frame: &mut Frame, area: Rect, app: &App, settings: &Settings) {
    let mut groups: Vec<&str> = Vec::new();
    for row in OPTIONS {
        if !groups.contains(&row.group) {
            groups.push(row.group);
        }
    }
    let height = u16::try_from(OPTIONS.len() + groups.len() + 4).unwrap_or(u16::MAX);
    let inner = popup_frame(frame, area, 52, height, "Settings");

    let [list, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas::<2>(inner);

    let mut lines: Vec<Line> = Vec::new();
    let mut group = "";
    for (index, row) in OPTIONS.iter().enumerate() {
        if row.group != group {
            group = row.group;
            lines.push(Line::from(Span::styled(
                format!(" {group}"),
                Style::default().add_modifier(Modifier::DIM | Modifier::BOLD),
            )));
        }
        let value = app.config().get(row.id).label();
        let text = format!(" {:<28}{value:>8}", row.label);
        let style = if index == settings.selected() {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    frame.render_widget(Paragraph::new(lines), list);

    let help = format!(" {}", settings.selected_row().help);
    let keys = " \u{2191}\u{2193} select   \u{2190}\u{2192} change   Enter toggle   Esc close";
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                help,
                Style::default().add_modifier(Modifier::DIM),
            )),
            Line::from(Span::styled(
                keys,
                Style::default().add_modifier(Modifier::DIM),
            )),
        ]),
        footer,
    );
}

#[cfg(test)]
mod tests {
    use nnano_core::config::{Config, Opt, Value};
    use nnano_core::editor::Action;

    use super::Settings;

    fn config() -> Config {
        Config::defaults()
    }

    #[test]
    fn arrows_move_the_selection_within_bounds() {
        let mut settings = Settings::new();
        let mut config = config();
        settings.handle(Action::MoveUp, &mut config);
        assert_eq!(settings.selected(), 0);
        settings.handle(Action::MoveDown, &mut config);
        settings.handle(Action::MoveDown, &mut config);
        settings.handle(Action::MoveDown, &mut config);
        assert_eq!(settings.selected(), 1);
        settings.handle(Action::MoveUp, &mut config);
        assert_eq!(settings.selected(), 0);
    }

    #[test]
    fn integers_step_inside_the_registry_bounds() {
        let mut settings = Settings::new();
        let mut config = config();
        assert_eq!(config.int(Opt::TabSize), 8);
        assert_eq!(
            settings.handle(Action::MoveRight, &mut config),
            Some(Opt::TabSize)
        );
        assert_eq!(config.int(Opt::TabSize), 9);
        for _ in 0..20 {
            settings.handle(Action::MoveLeft, &mut config);
        }
        assert_eq!(config.int(Opt::TabSize), 1, "must stop at the minimum");
    }

    #[test]
    fn booleans_toggle_from_any_change_key() {
        let mut settings = Settings::new();
        let mut config = config();
        // Move to the search row first.
        settings.handle(Action::MoveDown, &mut config);
        assert_eq!(settings.selected_row().id, Opt::CaseSensitive);
        assert_eq!(
            settings.handle(Action::Newline, &mut config),
            Some(Opt::CaseSensitive)
        );
        assert!(config.bool(Opt::CaseSensitive));
        settings.handle(Action::MoveLeft, &mut config);
        assert!(!config.bool(Opt::CaseSensitive));
    }

    #[test]
    fn other_keys_change_nothing() {
        let mut settings = Settings::new();
        let mut config = config();
        assert_eq!(settings.handle(Action::Insert('x'), &mut config), None);
        assert_eq!(config.int(Opt::TabSize), 8);
        assert_eq!(settings.handle(Action::Cancel, &mut config), None);
        assert_eq!(settings.selected_row().id, Opt::TabSize);
        assert_eq!(config.get(Opt::TabSize), Value::Int(8));
    }
}
