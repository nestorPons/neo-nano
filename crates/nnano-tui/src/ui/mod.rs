//! Rendering (phases 1, 2, 5, 6).
//!
//! Only the visible window is read from the rope: `text::visible_line` builds one
//! line at a time, and nothing keeps a `Vec<Line>` mirror of the buffer. Drawing
//! never decides *when* to redraw; the app loop does.

pub mod text;
pub mod viewport;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Paragraph;

use crate::App;

/// Draw a whole frame: text area above, one-row status bar below.
pub fn draw(frame: &mut Frame, app: &App) {
    let [body, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    draw_body(frame, body, app);
    draw_status(frame, status, app);
}

/// Text area plus the terminal cursor.
fn draw_body(frame: &mut Frame, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let viewport = app.viewport();
    let buffer = app.buffer();
    let tab_width = app.tab_width();

    for row in 0..area.height {
        let line = viewport.top_line + usize::from(row);
        if line >= buffer.line_count() {
            break;
        }
        let rendered = text::visible_line(buffer, line, viewport.left_col, area.width, tab_width);
        frame.buffer_mut().set_stringn(
            area.x,
            area.y + row,
            &rendered.text,
            usize::from(area.width),
            Style::default(),
        );
    }

    let cursor = app.cursor();
    let row = cursor.line().saturating_sub(viewport.top_line);
    let col = cursor
        .visual_column(buffer, tab_width)
        .saturating_sub(viewport.left_col);
    if row < usize::from(area.height)
        && col < usize::from(area.width)
        && let (Ok(x), Ok(y)) = (u16::try_from(col), u16::try_from(row))
    {
        frame.set_cursor_position((area.x + x, area.y + y));
    }
}

/// Status bar: identity on the left, position and file name on the right.
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

/// Scroll so the cursor is visible in a body of this size. Called by the app
/// loop before every draw, using the same arithmetic as the layout above.
pub fn ensure_cursor_visible(app: &mut App, width: u16, height: u16) {
    let col = app.cursor().visual_column(app.buffer(), app.tab_width());
    let line = app.cursor().line();
    app.viewport_mut().ensure_visible(line, col, width, height);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::{draw, ensure_cursor_visible};
    use crate::App;

    const WIDTH: u16 = 24;
    const HEIGHT: u16 = 5;

    fn render(app: &App) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
    }

    fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .skip(usize::from(y) * usize::from(WIDTH))
            .take(usize::from(WIDTH))
            .map(|cell| cell.symbol())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn draws_text_and_the_status_bar() {
        let app = App::from_text("notes.txt", "hola\nmundo\n");
        let terminal = render(&app);
        assert_eq!(row(&terminal, 0), "hola");
        assert_eq!(row(&terminal, 1), "mundo");
        let status = row(&terminal, HEIGHT - 1);
        assert!(status.contains("nnano"), "{status:?}");
        assert!(status.contains("notes.txt"), "{status:?}");
        assert!(status.contains("1:1"), "{status:?}");
    }

    #[test]
    fn expands_tabs_and_keeps_crlf_out_of_the_screen() {
        let app = App::from_text("t.txt", "a\tb\r\nc");
        let terminal = render(&app);
        assert_eq!(row(&terminal, 0), "a       b");
        assert_eq!(row(&terminal, 1), "c");
    }

    #[test]
    fn unnamed_buffer_is_labelled_and_empty() {
        let app = App::from_text("", "");
        let terminal = render(&app);
        assert_eq!(row(&terminal, 0), "");
        assert!(row(&terminal, HEIGHT - 1).contains("[No Name]"));
    }

    #[test]
    fn scrolls_down_to_follow_the_cursor() {
        let text = (0..80).map(|n| format!("line {n}\n")).collect::<String>();
        let mut app = App::from_text("long.txt", &text);
        app.set_cursor(79, 0);
        ensure_cursor_visible(&mut app, WIDTH, HEIGHT - 1);
        assert!(app.viewport().top_line > 0);

        let terminal = render(&app);
        assert_eq!(row(&terminal, HEIGHT - 2), "line 79");
    }

    #[test]
    fn scrolls_right_to_follow_a_long_line() {
        let mut app = App::from_text("wide.txt", &"a".repeat(100));
        app.set_cursor(0, 60);
        ensure_cursor_visible(&mut app, WIDTH, HEIGHT - 1);
        assert!(app.viewport().left_col > 0);
    }
}
