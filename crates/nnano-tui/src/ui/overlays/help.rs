//! The help overlay (`^H` or `F1`): every shortcut that works today.
//!
//! The binding list is **not** written here: it is rendered from
//! [`crate::term::input::KEYMAP`], the same table `decode` reads, so the help
//! cannot advertise a key that does nothing and a new binding shows up on its
//! own. Only what the table cannot describe — the replace prompt's answers and
//! the mouse — is written by hand, and those rows carry no chord of their own.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::popup_frame;
use crate::i18n::{Lang, t};
use crate::term::input::{GROUPS, bindings};

/// Rows that are not a single chord: the answers inside the replace prompt and
/// the mouse gestures. They change nothing in `decode`, so they stay text.
const NOTES: &[(&str, &[(&str, &str)])] = &[
    (
        "Prompts",
        &[
            ("Enter", "Accept the find or replace prompt"),
            ("y / n", "Replace or skip this match"),
            ("a / q", "Replace all or stop the run"),
        ],
    ),
    (
        "Mouse",
        &[
            ("Click / drag", "Place the cursor / start the mark"),
            ("Release", "Copy the mark to the clipboard"),
            ("Alt+drag", "Rectangular mark"),
            ("Wheel", "Scroll the view"),
            ("Click bar", "Jump to that page"),
        ],
    ),
];

/// Column where the description starts; the chord column is padded to it.
const KEYS_WIDTH: usize = 26;

fn heading(title: &str) -> Line<'static> {
    Line::from(format!(" {title}"))
        .style(Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
}

/// `keys` in bold on the left, `label` in the body colour; one row per line.
fn row(keys: &str, label: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("   {keys:<KEYS_WIDTH$}"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(label.to_string()),
    ])
}

/// The hand-written rows, for the translation coverage test: every label here
/// has to be in `crate::i18n`.
pub fn notes() -> &'static [(&'static str, &'static [(&'static str, &'static str)])] {
    NOTES
}

/// Text of every line in the panel: headings plus one row per binding, with a
/// blank line before each section but the first. The chord column stays as
/// typed (`Ctrl+S`); only the descriptions are translated.
fn lines(lang: Lang) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, group) in GROUPS.iter().enumerate() {
        if index > 0 {
            lines.push(Line::from(""));
        }
        lines.push(heading(t(lang, group.title())));
        for binding in bindings(*group) {
            lines.push(row(&binding.chord.label(), t(lang, binding.label)));
        }
    }
    for (title, rows) in NOTES {
        lines.push(Line::from(""));
        lines.push(heading(t(lang, title)));
        for (keys, label) in *rows {
            lines.push(row(keys, t(lang, label)));
        }
    }
    lines
}

/// How many lines the panel would need in total: the same in every language,
/// because translation changes text, never the row count.
pub fn content_len() -> usize {
    lines(Lang::En).len()
}

/// Scroll offset of the reference.
#[derive(Debug, Default)]
pub struct Help {
    scroll: usize,
}

impl Help {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Move `delta` lines inside the content; `viewport` is how many rows fit.
    pub fn scroll_by(&mut self, delta: isize, viewport: usize) {
        let max = content_len().saturating_sub(viewport);
        self.scroll = (self.scroll.min(max) as isize + delta).clamp(0, max as isize) as usize;
    }

    /// Jump to the top or the bottom of the reference.
    pub fn scroll_to(&mut self, end: bool, viewport: usize) {
        self.scroll = if end {
            content_len().saturating_sub(viewport)
        } else {
            0
        };
    }
}

/// Floating panel with the reference and a scroll hint.
pub fn draw(frame: &mut Frame, area: Rect, help: &Help, lang: Lang) {
    let lines = lines(lang);
    let width = 72.min(area.width);
    let height = u16::try_from(lines.len() + 3)
        .unwrap_or(u16::MAX)
        .min(area.height);
    let inner = popup_frame(frame, area, width, height, t(lang, "Help"));
    if inner.height == 0 {
        return;
    }
    let [body, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas::<2>(inner);

    frame.render_widget(
        Paragraph::new(lines).scroll((u16::try_from(help.scroll()).unwrap_or(0), 0)),
        body,
    );
    frame.render_widget(
        Paragraph::new(t(lang, " ↑↓ PgUp PgDn scroll · Esc closes"))
            .style(Style::default().add_modifier(Modifier::DIM)),
        footer,
    );
}

#[cfg(test)]
mod tests {
    use super::{Help, content_len, lines};
    use crate::term::input::KEYMAP;

    #[test]
    fn the_help_lists_every_binding_in_the_keymap() {
        let screen: Vec<String> = lines(crate::i18n::Lang::En)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect()
            })
            .collect();
        let text = screen.join("\n");
        for binding in KEYMAP {
            let chord = binding.chord.label();
            assert!(
                text.contains(&format!("{chord} ")),
                "{chord} is in the keymap but not in the help"
            );
            assert!(
                text.contains(binding.label),
                "{} is missing its description",
                binding.chord.label()
            );
        }
    }

    #[test]
    fn the_content_has_every_section_and_binding() {
        assert!(content_len() > KEYMAP.len(), "headings and notes too");
        assert!(content_len() > 55, "the reference should cover the keymap");
    }

    #[test]
    fn scrolling_clamps_to_the_content() {
        let mut help = Help::new();
        help.scroll_by(-5, 20);
        assert_eq!(help.scroll(), 0);
        help.scroll_by(10_000, 20);
        assert_eq!(help.scroll(), content_len() - 20);
        help.scroll_by(10, 20);
        assert_eq!(help.scroll(), content_len() - 20, "no scroll past the end");
    }

    #[test]
    fn home_and_end_jump() {
        let mut help = Help::new();
        help.scroll_to(true, 10);
        assert_eq!(help.scroll(), content_len() - 10);
        help.scroll_to(false, 10);
        assert_eq!(help.scroll(), 0);
    }

    #[test]
    fn a_viewport_larger_than_the_content_cannot_scroll() {
        let mut help = Help::new();
        help.scroll_by(5, content_len() + 5);
        assert_eq!(help.scroll(), 0);
    }
}
