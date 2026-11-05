//! Rendering (phases 1, 2, 5, 6).
//!
//! The body stays blank until the rope buffer lands (phase 2): only viewport
//! lines are read from it and only they are matched for syntax. Draw only happens
//! when the app is dirty, so this module never decides *when* to redraw.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Paragraph;

use crate::App;

/// Draw a whole frame: blank body above, one-row status bar below.
pub fn draw(frame: &mut Frame, app: &App) {
    let [_body, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    draw_status(frame, status, app);
}

/// Status bar: identity on the left, filename on the right, both reversed.
fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let style = Style::default().add_modifier(Modifier::REVERSED);
    let left = app.status_left();
    let right = app.status_right();
    let right_width = u16::try_from(right.chars().count())
        .unwrap_or(u16::MAX)
        .saturating_add(1)
        .min(area.width);
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).areas(area);

    frame.render_widget(Paragraph::new(left).style(style), left_area);
    frame.render_widget(
        Paragraph::new(right)
            .style(style)
            .alignment(Alignment::Right),
        right_area,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::draw;
    use crate::App;

    const WIDTH: u16 = 48;

    fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .skip(usize::from(y) * usize::from(WIDTH))
            .take(usize::from(WIDTH))
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn draws_version_and_file_name_in_the_last_row() {
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, 4)).unwrap();
        let app = App::new(Some("notes.txt".to_string()));

        terminal.draw(|frame| draw(frame, &app)).unwrap();

        let status = row(&terminal, 3);
        assert!(status.contains("nnano"), "{status:?}");
        assert!(status.contains("notes.txt"), "{status:?}");
        assert_eq!(row(&terminal, 0).trim(), "");
    }

    #[test]
    fn unnamed_buffer_is_labelled() {
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, 4)).unwrap();
        let app = App::new(None);

        terminal.draw(|frame| draw(frame, &app)).unwrap();

        assert!(row(&terminal, 3).contains("[No Name]"));
    }
}
