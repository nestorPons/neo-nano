//! The help overlay (`^H` or `F1`): every shortcut that works today.
//!
//! Content is grouped by area, one line per binding, and scrolls when the terminal
//! cannot show it all. Only implemented keys are listed: a help screen that
//! advertises phase-7 bindings that do not work would be the same lie as a key
//! that decodes to a silent no-op.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use super::popup_frame;

/// One binding: the keys as the user types them, and what they do.
struct Entry {
    keys: &'static str,
    action: &'static str,
}

const fn entry(keys: &'static str, action: &'static str) -> Entry {
    Entry { keys, action }
}

/// The whole reference. Order matters: it is the order on screen.
const SECTIONS: &[(&str, &[Entry])] = &[
    (
        "Files",
        &[
            entry("Ctrl+S", "Save in place"),
            entry("Ctrl+Q", "Quit (confirms if modified)"),
            entry("Ctrl+N / Ctrl+T", "New buffer"),
            entry("Ctrl+W", "Close buffer (confirms)"),
        ],
    ),
    (
        "Editing",
        &[
            entry("Ctrl+Z / Ctrl+Y", "Undo / redo"),
            entry("Alt+U / Alt+E", "Undo / redo"),
            entry("Ctrl+X / Ctrl+C / Ctrl+V", "Cut / copy / paste"),
            entry("Ctrl+K / Ctrl+U", "Cut line / paste"),
            entry("Ctrl+A / Alt+A", "Select all / start mark"),
            entry("Alt+6", "Copy the mark"),
            entry("Alt+Z", "Rectangular mark (Alt+drag too)"),
            entry("Ctrl+D", "Delete character"),
            entry("Ctrl+\\ / Alt+R", "Replace"),
        ],
    ),
    (
        "Move",
        &[
            entry("Arrows", "One character / one line"),
            entry("Home / End", "Start / end of line"),
            entry("PgUp / PgDn", "One page"),
            entry("Alt+Up / Alt+Down", "One page"),
            entry("Ctrl+B / Ctrl+E", "Back one char / end of line"),
            entry("Ctrl+L", "Repaint the screen"),
        ],
    ),
    (
        "Search",
        &[
            entry("Ctrl+F", "Find"),
            entry("F3 / Alt+W", "Find next"),
            entry("Shift+F3 / Alt+B", "Find previous"),
            entry("Alt+C", "Case sensitivity (in the prompt)"),
            entry("Alt+R", "Regexp (in the prompt) / replace (outside)"),
            entry("Alt+B", "Direction (in the find prompt)"),
        ],
    ),
    (
        "Replace",
        &[
            entry("Enter", "Accept the prompt"),
            entry("y / n", "Replace / skip this match"),
            entry("a / q", "Replace all / stop"),
        ],
    ),
    (
        "Interface",
        &[
            entry("Ctrl+H / F1", "This help"),
            entry("Ctrl+P", "Command palette"),
            entry("Alt+,", "Settings"),
            entry("Alt+Y", "Syntax highlighting on/off"),
            entry("Esc", "Close overlay / clear the mark"),
        ],
    ),
    (
        "Mouse",
        &[
            entry("Click / drag", "Place the cursor / mark"),
            entry("Alt+drag", "Rectangular mark"),
            entry("Wheel", "Scroll the view"),
        ],
    ),
];

/// Text of every line in the panel: headings plus one row per binding, with a
/// blank line before each section but the first.
fn lines() -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, (heading, entries)) in SECTIONS.iter().enumerate() {
        if index > 0 {
            lines.push(Line::from(""));
        }
        lines.push(
            Line::from(format!(" {heading}"))
                .style(Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
        );
        for entry in *entries {
            lines.push(Line::from(vec![
                ratatui::text::Span::styled(
                    format!("   {:<28}", entry.keys),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::raw(entry.action),
            ]));
        }
    }
    lines
}

/// How many lines the panel would need in total.
pub fn content_len() -> usize {
    // `lines()` builds owned text; counting here keeps it a pure read.
    SECTIONS.len()
        + SECTIONS
            .iter()
            .map(|(_, entries)| entries.len())
            .sum::<usize>()
        + SECTIONS.len().saturating_sub(1)
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
pub fn draw(frame: &mut Frame, area: Rect, help: &Help) {
    let lines = lines();
    let width = 72.min(area.width);
    let height = u16::try_from(lines.len() + 3)
        .unwrap_or(u16::MAX)
        .min(area.height);
    let inner = popup_frame(frame, area, width, height, "Help");
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
        Paragraph::new(" \u{2191}\u{2193} PgUp PgDn scroll \u{00b7} Esc closes")
            .style(Style::default().add_modifier(Modifier::DIM)),
        footer,
    );
}

#[cfg(test)]
mod tests {
    use super::{Help, content_len};

    #[test]
    fn the_content_has_every_section_and_binding() {
        assert!(content_len() > 30, "the reference should cover the keymap");
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
