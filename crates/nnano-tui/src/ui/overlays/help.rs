//! The help overlay (`F1`).
//!
//! Static text, and deliberately only the keys that work today: a help screen
//! that lists phase-7 bindings would be the same kind of lie as a key that
//! decodes to a silent no-op. It grows one line per landed feature.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use super::popup_frame;

/// `("", heading)`, `("  ", entry)`; headings are dimmed and entries plain.
const HELP: &[&str] = &[
    " nnano 0.1 \u{2014} keys that work today",
    "",
    "Files",
    "  ^S save in place",
    "  ^Q quit (asks again if modified)",
    "",
    "Move",
    "  arrows, Home, End",
    "  PgUp PgDn, M-\u{2191} M-\u{2193}",
    "  ^B ^D ^E",
    "",
    "Search",
    "  ^F find, Enter accepts, Esc closes",
    "  M-W / F3 next, M-B / Shift+F3 previous",
    "  in the prompt: M-C case, M-R regexp, M-B direction",
    "",
    "Interface",
    "  ^P commands",
    "  M-, settings",
    "  F1 help",
];

/// Keys that are documented but not implemented yet are simply absent.
pub fn draw(frame: &mut Frame, area: Rect) {
    let height = u16::try_from(HELP.len() + 2).unwrap_or(u16::MAX);
    let inner = popup_frame(frame, area, 58, height, "Help");

    let lines: Vec<Line> = HELP
        .iter()
        .map(|line| {
            let style = if line.is_empty() || line.starts_with("  ") {
                Style::default()
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            };
            Line::from(*line).style(style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
