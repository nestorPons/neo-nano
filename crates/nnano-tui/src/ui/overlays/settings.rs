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
use super::prompt::prompt_width;
use crate::App;
use crate::i18n::{Lang, t};

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
    /// inside their registry bounds, an enum cycles through its declared values
    /// (wrapping, so the picker is one key in each direction).
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
                    (Kind::Enum(values), Action::MoveLeft) => {
                        Value::Text(step_enum(values, config, row.id, -1))
                    }
                    (Kind::Enum(values), _) => Value::Text(step_enum(values, config, row.id, 1)),
                };
                config.set(row.id, next).ok().map(|()| row.id)
            }
            _ => None,
        }
    }
}

/// Next value of an enum, wrapping. An unrecognised current value behaves like
/// the first one, so a hand-edited file cannot strand the picker.
fn step_enum(
    values: &'static [&'static str],
    config: &nnano_core::config::Config,
    id: Opt,
    step: isize,
) -> &'static str {
    let current = config.get(id).as_text().unwrap_or_default();
    let index = values
        .iter()
        .position(|value| value.eq_ignore_ascii_case(current))
        .unwrap_or(0);
    let len = values.len() as isize;
    let next = (index as isize + step).rem_euclid(len);
    values[usize::try_from(next).unwrap_or(0)]
}

/// Height of the panel: options, group headings, the help line, the key line and
/// the two border rows.
fn height() -> u16 {
    u16::try_from(OPTIONS.len() + headings() + 4).unwrap_or(u16::MAX)
}

/// Headings actually drawn: one **each time the group changes**, which is not the
/// number of distinct groups, because the registry interleaves Editor/Search/
/// Editor. Counting distinct names here used to clip the footer by a row.
fn headings() -> usize {
    let mut group = "";
    let mut count = 0;
    for row in OPTIONS {
        if row.group != group {
            group = row.group;
            count += 1;
        }
    }
    count
}

/// Panel row of each option, with the group headings counted in. Shared by the
/// renderer and the mouse hit test so a click lands on what is drawn.
fn row_offsets() -> Vec<usize> {
    let mut offsets = Vec::with_capacity(OPTIONS.len());
    let mut line = 0;
    let mut group = "";
    for row in OPTIONS {
        if row.group != group {
            group = row.group;
            line += 1;
        }
        offsets.push(line);
        line += 1;
    }
    offsets
}

/// Option under a screen row, for a click inside the panel.
pub fn row_at(area: Rect, y: u16) -> Option<usize> {
    let inner = super::panel_inner(area, PANEL_WIDTH, height());
    let offset = usize::from(y.checked_sub(inner.y)?);
    row_offsets().iter().position(|line| *line == offset)
}

impl Settings {
    /// Put the highlight on `index` (mouse click).
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(OPTIONS.len() - 1);
    }
}

/// Floating panel with the option rows and the selected row's help text.
pub fn draw(frame: &mut Frame, area: Rect, app: &App, settings: &Settings) {
    let lang = app.lang();
    let height = height();
    let inner = popup_frame(frame, area, PANEL_WIDTH, height, t(lang, "Settings"));

    let [list, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas::<2>(inner);

    let mut lines: Vec<Line> = Vec::new();
    let mut group = "";
    for (index, row) in OPTIONS.iter().enumerate() {
        if row.group != group {
            group = row.group;
            lines.push(Line::from(Span::styled(
                format!(" {}", t(lang, group)),
                Style::default().add_modifier(Modifier::DIM | Modifier::BOLD),
            )));
        }
        let text = format!(
            " {}{}",
            pad(t(lang, row.label), LABEL_WIDTH),
            value_text(app, row)
        );
        let style = if index == settings.selected() {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    frame.render_widget(Paragraph::new(lines), list);

    let help = format!(" {}", t(lang, settings.selected_row().help));
    let keys = t(lang, " ↑↓ select   ←→ change   Enter toggle   Esc close");
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

/// Panel width and label column, in cells: wide enough for the longest
/// translated label and the longest language name (`Bahasa Indonesia (auto)`).
const PANEL_WIDTH: u16 = 62;
const LABEL_WIDTH: usize = 34;

/// Cell-exact padding: a translated label wider than the column is clipped in
/// cells, not in chars, so CJK and Arabic rows stay inside the panel.
fn pad(text: &str, width: usize) -> String {
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
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}

/// The right column: `on`/`off` translated, the number as typed, and for the
/// language row the language written in itself. `auto` also names what it
/// currently resolves to, because "auto" alone does not tell the user much.
fn value_text(app: &App, row: &OptRow) -> String {
    let lang = app.lang();
    match app.config().get(row.id) {
        Value::Bool(true) => t(lang, "on").to_string(),
        Value::Bool(false) => t(lang, "off").to_string(),
        Value::Int(value) => value.to_string(),
        Value::Text(code) if code.eq_ignore_ascii_case("auto") => {
            format!("{} ({})", Lang::detect().native_name(), t(lang, "auto"))
        }
        Value::Text(code) => Lang::resolve(code).native_name().to_string(),
    }
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
        use nnano_core::config::OPTIONS;

        let mut settings = Settings::new();
        let mut config = config();
        settings.handle(Action::MoveUp, &mut config);
        assert_eq!(settings.selected(), 0);
        for _ in 0..OPTIONS.len() + 3 {
            settings.handle(Action::MoveDown, &mut config);
        }
        assert_eq!(settings.selected(), OPTIONS.len() - 1);
        settings.handle(Action::MoveUp, &mut config);
        assert_eq!(settings.selected(), OPTIONS.len() - 2);
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

    #[test]
    fn an_enum_cycles_through_its_values_and_wraps() {
        use nnano_core::config::OPTIONS;

        let mut settings = Settings::new();
        let mut config = config();
        for _ in 0..OPTIONS.len() {
            settings.handle(Action::MoveDown, &mut config);
        }
        assert_eq!(settings.selected_row().id, Opt::Language);
        assert_eq!(config.get(Opt::Language), Value::Text("en"));

        assert_eq!(
            settings.handle(Action::MoveRight, &mut config),
            Some(Opt::Language)
        );
        assert_eq!(config.get(Opt::Language), Value::Text("zh-Hans"));

        // Left goes back to the beginning of the picker.
        settings.handle(Action::MoveLeft, &mut config);
        settings.handle(Action::MoveLeft, &mut config);
        assert_eq!(config.get(Opt::Language), Value::Text("auto"));

        // Enter steps forward like `→`, and wraps at the end.
        settings.handle(Action::Newline, &mut config);
        assert_eq!(config.get(Opt::Language), Value::Text("en"));
        settings.handle(Action::MoveLeft, &mut config);
        assert_eq!(config.get(Opt::Language), Value::Text("auto"));
    }

    #[test]
    fn a_language_change_is_live_and_translated_immediately() {
        use nnano_core::config::OPTIONS;

        let mut app = crate::App::from_text("s.txt", "");
        app.apply(Action::OpenSettings);
        for _ in 0..OPTIONS.len() {
            app.apply(Action::MoveDown);
        }
        // `en` -> `zh-Hans`.
        app.apply(Action::MoveRight);
        assert_eq!(app.lang(), crate::i18n::Lang::ZhHans);
        assert_eq!(app.config().language(), crate::i18n::Lang::ZhHans);
    }
}
