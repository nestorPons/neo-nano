//! Rendering (phases 1, 2, 5, 6).
//!
//! Only the visible window is read from the rope: `text::visible_line` builds one
//! line at a time, and nothing keeps a `Vec<Line>` mirror of the buffer. Drawing
//! never decides *when* to redraw; the app loop does.

pub mod text;
pub mod viewport;

use nnano_core::buffer::line_visual_column;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
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
    let buffer = app.editor().buffer();
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

    let cursor = app.editor().cursor();
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
    frame.render_widget(
        Paragraph::new(status_text(app, area.width)).style(style),
        area,
    );
}

/// One exactly-wide line: identity on the left, `line:column  file` on the right.
///
/// The right side (position, then file name) wins the space budget, so a narrow
/// terminal clips `[Modified]` and the title rather than the cursor position.
/// Width is measured cell-wise, not by `chars().count()`, so CJK file names do
/// not overlap the title.
fn status_text(app: &App, width: u16) -> String {
    let width = usize::from(width);
    let left = app.status_left();
    let right = app.status_right();
    let left_width = line_visual_column(left.chars(), 1);
    let right_width = line_visual_column(right.chars(), 1);

    if right_width >= width {
        return right.chars().take(width).collect();
    }
    if left_width + right_width < width {
        let padding = width - left_width - right_width;
        return format!("{left}{}{right}", " ".repeat(padding));
    }
    let keep = width - right_width;
    let clipped: String = left.chars().take(keep).collect();
    format!("{clipped}{right}")
}

/// Scroll so the cursor is visible in a body of this size. Called by the app
/// loop before every draw, using the same arithmetic as the layout above.
pub fn ensure_cursor_visible(app: &mut App, width: u16, height: u16) {
    let col = app
        .editor()
        .cursor()
        .visual_column(app.editor().buffer(), app.tab_width());
    let line = app.editor().cursor().line();
    app.viewport_mut().ensure_visible(line, col, width, height);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use nnano_core::editor::Action;

    use super::{draw, ensure_cursor_visible, status_text};
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
    fn status_shows_the_modified_marker_and_keeps_the_position_when_narrow() {
        let mut app = App::from_text("notes.txt", "");
        app.apply(Action::Insert('x'));

        let wide = status_text(&app, 80);
        assert!(wide.contains("[Modified]"), "{wide:?}");
        assert_eq!(wide.chars().count(), 80);

        let narrow = status_text(&app, 20);
        assert_eq!(narrow.chars().count(), 20);
        assert!(narrow.contains("1:2"), "{narrow:?}");
        assert_eq!(status_text(&app, 0), "");
    }

    #[test]
    fn scrolls_right_to_follow_a_long_line() {
        let mut app = App::from_text("wide.txt", &"a".repeat(100));
        app.set_cursor(0, 60);
        ensure_cursor_visible(&mut app, WIDTH, HEIGHT - 1);
        assert!(app.viewport().left_col > 0);
    }
}
