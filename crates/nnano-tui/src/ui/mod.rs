//! Rendering (phases 1, 2, 5, 6).
//!
//! Only the visible window is read from the rope: `text::visible_line` builds one
//! line at a time, and nothing keeps a `Vec<Line>` mirror of the buffer. Drawing
//! never decides *when* to redraw; the app loop does.
//!
//! The find prompt replaces the status row (nano's bottom line); settings, the
//! palette and help are floating panels over the text area, so the text never
//! reflows because an overlay opened.

pub mod overlays;
pub mod style;
pub mod text;
pub mod viewport;

use nnano_core::buffer::{Selection, line_visual_column};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Paragraph;

use crate::App;
use crate::ui::overlays::Overlay;

/// The screen split: an optional one-row buffer bar, the text area and the status
/// row. Shared by the renderer and the mouse hit test so clicks land where things
/// are drawn.
pub fn areas(area: Rect, buffers: usize) -> (Option<Rect>, Rect, Rect) {
    let tab_rows = u16::from(buffers > 1);
    let [tabs, body, status] = Layout::vertical([
        Constraint::Length(tab_rows),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    (if tab_rows > 0 { Some(tabs) } else { None }, body, status)
}

/// X offset and width of each buffer in the one-row bar, starting at column 1.
pub(crate) fn tab_layout(names: &[String]) -> Vec<(u16, u16)> {
    let mut layout = Vec::with_capacity(names.len());
    let mut x = 1u16;
    for name in names {
        let width = u16::try_from(line_visual_column(name.chars(), 1) + 2).unwrap_or(u16::MAX);
        layout.push((x, width));
        x = x.saturating_add(width).saturating_add(1);
    }
    layout
}

/// Draw a whole frame: buffer bar (when several buffers are open), text area,
/// one-row status bar, plus whatever overlay currently owns the keys.
pub fn draw(frame: &mut Frame, app: &App) {
    let (tabs, body, status) = areas(frame.area(), app.buffer_count());
    if let Some(tabs) = tabs {
        draw_tabs(frame, tabs, app);
    }
    let overlay = app.overlay();
    draw_body(frame, body, app, overlay.is_none());

    match overlay {
        Some(Overlay::Find(prompt)) => {
            overlays::find::draw_prompt(frame, status, app.find_settings(), prompt);
        }
        Some(Overlay::Replace(replace)) => {
            overlays::replace::draw_status(frame, status, replace);
        }
        Some(overlay) => {
            draw_status(frame, status, app);
            overlay.draw(frame, body, app);
        }
        None => draw_status(frame, status, app),
    }
}

/// One row listing every open buffer; the active one is reversed.
fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let names = app.buffer_names();
    let layout = tab_layout(&names);
    let active = app.active_buffer();

    for (index, (left, width)) in layout.iter().enumerate() {
        let Some(name) = names.get(index) else {
            break;
        };
        let style = if index == active {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let text = format!(" {name} ");
        frame
            .buffer_mut()
            .set_stringn(area.x + left, area.y, &text, usize::from(*width), style);
        if index + 1 < names.len() {
            frame.buffer_mut().set_stringn(
                area.x + left + width,
                area.y,
                "\u{2502}",
                1,
                Style::default().add_modifier(Modifier::DIM),
            );
        }
    }
}

/// Text area, plus the terminal cursor only when the editor owns it.
fn draw_body(frame: &mut Frame, area: Rect, app: &App, show_cursor: bool) {
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
        draw_syntax(frame, area, app, line, row);
    }

    if let Some(selection) = app.selection() {
        draw_selection(frame, area, app, selection);
    }

    if !show_cursor {
        return;
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

/// Colour the highlighted spans of one line. Only viewport lines reach here, and a
/// line longer than the core cap yields no spans at all.
fn draw_syntax(frame: &mut Frame, area: Rect, app: &App, line: usize, row: u16) {
    let spans = app.syntax_spans(line);
    if spans.is_empty() {
        return;
    }
    let buffer = app.editor().buffer();
    let tab_width = app.tab_width();
    let left_col = app.viewport().left_col;
    let width = usize::from(area.width);

    for span in spans {
        let start = text::char_column(buffer, line, span.start, tab_width).saturating_sub(left_col);
        let end = text::char_column(buffer, line, span.end, tab_width).saturating_sub(left_col);
        let left = start.min(width);
        let right = end.min(width);
        if right <= left {
            continue;
        }
        frame.buffer_mut().set_style(
            Rect::new(
                area.x + u16::try_from(left).unwrap_or(0),
                area.y + row,
                u16::try_from(right - left).unwrap_or(u16::MAX),
                1,
            ),
            style::span_style(&span.style),
        );
    }
}

/// Reverse video over the marked cells, block or stream. Applied after the text and
/// after the syntax colours, so the mark always wins.
fn draw_selection(frame: &mut Frame, area: Rect, app: &App, selection: &Selection) {
    let viewport = app.viewport();
    let buffer = app.editor().buffer();
    let tab_width = app.tab_width();
    let style = Style::default().add_modifier(Modifier::REVERSED);

    for row in 0..area.height {
        let line = viewport.top_line + usize::from(row);
        if line >= buffer.line_count() {
            break;
        }
        let Some((from, to)) = selection.line_span(buffer, line) else {
            continue;
        };
        let start =
            text::char_column(buffer, line, from, tab_width).saturating_sub(viewport.left_col);
        let end = text::char_column(buffer, line, to, tab_width).saturating_sub(viewport.left_col);
        let left = start.min(usize::from(area.width));
        let right = end.min(usize::from(area.width));
        if right <= left {
            continue;
        }
        frame.buffer_mut().set_style(
            Rect::new(
                area.x + u16::try_from(left).unwrap_or(0),
                area.y + row,
                u16::try_from(right - left).unwrap_or(u16::MAX),
                1,
            ),
            style,
        );
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
    compose_line(&app.status_left(), &app.status_right(), width)
}

/// Left text wins the left side, right text is kept whole when it fits, and the
/// result is exactly `width` cells. Shared by the status bar and the find prompt.
pub(crate) fn compose_line(left: &str, right: &str, width: u16) -> String {
    let width = usize::from(width);
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

    use super::{compose_line, draw, ensure_cursor_visible, status_text};
    use crate::App;

    const WIDTH: u16 = 24;
    const HEIGHT: u16 = 5;
    /// Floating panels need room for a border and their rows.
    const PANEL_WIDTH: u16 = 60;
    const PANEL_HEIGHT: u16 = 20;

    fn render(app: &App) -> Terminal<TestBackend> {
        render_sized(app, WIDTH, HEIGHT)
    }

    fn render_sized(app: &App, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
    }

    fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
        row_of(terminal, y, WIDTH)
    }

    fn row_of(terminal: &Terminal<TestBackend>, y: u16, width: u16) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .skip(usize::from(y) * usize::from(width))
            .take(usize::from(width))
            .map(|cell| cell.symbol())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    /// Whole screen as text, for "this panel is on screen" assertions.
    fn screen(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> String {
        (0..height)
            .map(|y| row_of(terminal, y, width))
            .collect::<Vec<_>>()
            .join("\n")
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

    #[test]
    fn compose_line_pads_clips_and_measures_cells() {
        assert_eq!(compose_line("ab", "cd", 7), "ab   cd");
        assert_eq!(compose_line("abcd", "cd", 6), "abcdcd");
        assert_eq!(compose_line("漢", "", 2), "漢");
        assert_eq!(compose_line("", "x", 0), "");
    }

    fn rust_syntax() -> nnano_core::syntax::SyntaxSet {
        nnano_core::syntax::SyntaxSet::from_grammars(nnano_core::syntax::parse(
            "syntax \"rust\" \"\\\\.rs$\"\ncolor red \"fn\"\n",
        ))
    }

    #[test]
    fn syntax_colours_reach_the_cells() {
        let mut app = App::from_text("main.rs", "fn x\n");
        app.apply_syntax(rust_syntax());
        let terminal = render(&app);
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].fg, ratatui::style::Color::Red);
        assert_eq!(buffer[(1, 0)].fg, ratatui::style::Color::Red);
        assert_eq!(buffer[(3, 0)].fg, ratatui::style::Color::Reset);
    }

    #[test]
    fn the_mark_wins_over_syntax_colour() {
        let mut app = App::from_text("main.rs", "fn x\n");
        app.apply_syntax(rust_syntax());
        app.set_cursor(0, 0);
        app.apply(Action::StartMark);
        app.apply(Action::MoveRight);
        app.apply(Action::MoveRight);
        let terminal = render(&app);
        let modifier = terminal.backend().buffer()[(0, 0)].modifier;
        assert!(modifier.contains(ratatui::style::Modifier::REVERSED));
    }

    #[test]
    fn the_buffer_bar_appears_only_with_several_buffers() {
        let single = App::from_text("uno.txt", "uno\n");
        let terminal = render_sized(&single, PANEL_WIDTH, 6);
        assert_eq!(row_of(&terminal, 0, PANEL_WIDTH), "uno");

        let mut app = App::from_text("uno.txt", "uno\n");
        app.apply(Action::NewBuffer);
        app.apply(Action::SwitchBuffer(0));
        let terminal = render_sized(&app, PANEL_WIDTH, 6);
        let bar = row_of(&terminal, 0, PANEL_WIDTH);
        assert!(bar.contains("uno.txt"), "{bar:?}");
        assert!(bar.contains("[No Name]"), "{bar:?}");
        assert_eq!(row_of(&terminal, 1, PANEL_WIDTH), "uno");
    }

    #[test]
    fn the_marked_region_is_rendered_in_reverse_video() {
        let mut app = App::from_text("notes.txt", "uno\ndos\n");
        app.set_cursor(0, 0);
        app.apply(Action::StartMark);
        app.apply(Action::MoveRight);
        app.apply(Action::MoveRight);
        let terminal = render(&app);
        let buffer = terminal.backend().buffer();
        for x in 0..2 {
            assert!(
                buffer[(x, 0)]
                    .modifier
                    .contains(ratatui::style::Modifier::REVERSED),
                "cell {x} should be marked"
            );
        }
        assert!(
            !buffer[(2, 0)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );
    }

    #[test]
    fn the_find_prompt_takes_over_the_status_row() {
        let mut app = App::from_text("notes.txt", "uno\ndos\n");
        app.apply(Action::OpenFind);
        for ch in "dos".chars() {
            app.apply(Action::Insert(ch));
        }
        let terminal = render_sized(&app, PANEL_WIDTH, PANEL_HEIGHT);
        let status = row_of(&terminal, PANEL_HEIGHT - 1, PANEL_WIDTH);
        assert!(status.contains("Find: dos"), "{status:?}");
        assert!(!status.contains("nnano"), "{status:?}");
    }

    #[test]
    fn the_settings_panel_lists_registry_rows() {
        let mut app = App::from_text("notes.txt", "");
        app.apply(Action::OpenSettings);
        let terminal = render_sized(&app, PANEL_WIDTH, PANEL_HEIGHT);
        let screen = screen(&terminal, PANEL_WIDTH, PANEL_HEIGHT);
        assert!(screen.contains("Settings"), "{screen}");
        assert!(screen.contains("Tab size"), "{screen}");
        assert!(screen.contains("Case-sensitive search"), "{screen}");
        assert!(screen.contains('8'), "{screen}");
    }

    #[test]
    fn the_palette_panel_draws_its_filter_and_commands() {
        let mut app = App::from_text("notes.txt", "");
        app.apply(Action::OpenPalette);
        let terminal = render_sized(&app, PANEL_WIDTH, PANEL_HEIGHT);
        let screen = screen(&terminal, PANEL_WIDTH, PANEL_HEIGHT);
        assert!(screen.contains("Commands"), "{screen}");
        assert!(screen.contains("Save file"), "{screen}");
        assert!(screen.contains("Settings"), "{screen}");
    }

    #[test]
    fn the_help_panel_draws_and_stays_inside_the_area() {
        let mut app = App::from_text("notes.txt", "");
        app.apply(Action::OpenHelp);
        let terminal = render_sized(&app, PANEL_WIDTH, PANEL_HEIGHT);
        let screen = screen(&terminal, PANEL_WIDTH, PANEL_HEIGHT);
        assert!(screen.contains("Help"), "{screen}");
        assert!(screen.contains("save in place"), "{screen}");
        // A tiny terminal must not panic or draw outside the frame.
        let _ = render_sized(&app, 8, 3);
    }
}
