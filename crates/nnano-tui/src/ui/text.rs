//! Visible text of one line: tab expansion, horizontal offset and wide glyphs.
//!
//! Only the window that fits on screen is built. A 5 MB single line therefore
//! costs one walk up to `left_col` plus the visible cells, never the line length.

use nnano_core::buffer::{TextBuffer, Width};

/// One line prepared for the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLine {
    pub text: String,
    /// Cells the text occupies, at most the requested width.
    pub width: u16,
}

impl RenderedLine {
    fn new(text: String, width: usize) -> Self {
        Self {
            text,
            width: u16::try_from(width).unwrap_or(u16::MAX),
        }
    }
}

/// Text of `line` starting at visual column `left_col`, at most `max_width`
/// cells. Tabs expand to spaces, `\r\n` is not drawn, and a wide glyph that would
/// straddle the right edge is dropped instead of split.
pub fn visible_line(
    buffer: &TextBuffer,
    line: usize,
    left_col: usize,
    max_width: u16,
    tab_width: usize,
) -> RenderedLine {
    let max_width = usize::from(max_width);
    let mut text = String::new();
    let mut width = Width::new(0, tab_width);
    let mut drawn = 0;

    if max_width == 0 {
        return RenderedLine::new(text, 0);
    }

    for char_idx in buffer.line_start(line)..buffer.line_end(line) {
        let Some(ch) = buffer.char_at(char_idx) else {
            break;
        };
        let before = width.column();
        let cells = width.push(ch);
        let after = width.column();

        // Still left of the window: skip without emitting anything.
        if after <= left_col {
            continue;
        }
        // Starts before the window: the left edge cuts through this glyph.
        if before < left_col {
            continue;
        }
        if drawn + cells > max_width {
            break;
        }
        match ch {
            '\t' => {
                // Pad up to the next stop, relative to the scroll offset.
                text.push_str(&" ".repeat(cells));
            }
            _ => text.push(ch),
        }
        drawn += cells;
        if drawn == max_width {
            break;
        }
    }

    // `next_tab_stop` is not needed here: `Width` already turns a tab into the
    // cells between the current column and the next stop.
    RenderedLine::new(text, drawn)
}

/// Visual column where char offset `col` of `line` starts. Used by the selection
/// highlight.
pub fn char_column(buffer: &TextBuffer, line: usize, col: usize, tab_width: usize) -> usize {
    nnano_core::buffer::line_visual_column(buffer.line_chars(line).take(col), tab_width)
}

/// Char offset whose cell contains visual column `target`, clamped to the line.
/// The inverse of [`char_column`] for the mouse; a click inside a wide glyph or a
/// tab puts the cursor before it, which is what the cursor can represent.
pub fn column_to_char(buffer: &TextBuffer, line: usize, target: usize, tab_width: usize) -> usize {
    let mut width = Width::new(0, tab_width);
    for (col, ch) in buffer.line_chars(line).enumerate() {
        width.push(ch);
        if target < width.column() {
            return col;
        }
    }
    buffer.line_len(line)
}

#[cfg(test)]
mod tests {
    use nnano_core::buffer::TextBuffer;

    use super::{char_column, column_to_char, visible_line};

    #[test]
    fn expands_tabs_into_spaces() {
        let buffer = TextBuffer::from_text("a\tb");
        let line = visible_line(&buffer, 0, 0, 40, 8);
        assert_eq!(line.text, "a       b");
        assert_eq!(line.width, 9);
    }

    #[test]
    fn drops_the_cr_of_crlf() {
        let buffer = TextBuffer::from_text("uno\r\ndos");
        assert_eq!(visible_line(&buffer, 0, 0, 40, 8).text, "uno");
        assert_eq!(visible_line(&buffer, 1, 0, 40, 8).text, "dos");
    }

    #[test]
    fn stops_before_a_wide_glyph_that_does_not_fit() {
        let buffer = TextBuffer::from_text("a漢");
        let line = visible_line(&buffer, 0, 0, 2, 8);
        assert_eq!(line.text, "a");
        assert_eq!(line.width, 1);
    }

    #[test]
    fn honours_the_horizontal_offset() {
        let buffer = TextBuffer::from_text("abcdef");
        let line = visible_line(&buffer, 0, 2, 3, 8);
        assert_eq!(line.text, "cde");
        assert_eq!(line.width, 3);
    }

    #[test]
    fn offset_inside_a_tab_skips_the_whole_tab() {
        let buffer = TextBuffer::from_text("\tx");
        let line = visible_line(&buffer, 0, 3, 20, 8);
        assert_eq!(line.text, "x");
        assert_eq!(line.width, 1);
    }

    #[test]
    fn zero_width_takes_nothing() {
        let buffer = TextBuffer::from_text("abc");
        assert_eq!(visible_line(&buffer, 0, 0, 0, 8).text, "");
    }

    #[test]
    fn columns_and_chars_round_trip() {
        let buffer = TextBuffer::from_text("a\tb漢c");
        for col in 0..=buffer.line_len(0) {
            let cell = char_column(&buffer, 0, col, 4);
            assert_eq!(column_to_char(&buffer, 0, cell, 4), col, "col {col}");
        }
        // A click inside the tab stops before it.
        assert_eq!(column_to_char(&buffer, 0, 2, 4), 1);
        // Past the end is the line end.
        assert_eq!(column_to_char(&buffer, 0, 99, 4), buffer.line_len(0));
    }

    #[test]
    fn big_line_only_builds_the_window() {
        let buffer = TextBuffer::from_text(&"a".repeat(5 * 1024 * 1024));
        let line = visible_line(&buffer, 0, 0, 80, 8);
        assert_eq!(line.text.len(), 80);
        assert_eq!(line.width, 80);
    }
}
