//! Cursor position: a logical `(line, char offset)` pair.
//!
//! `AGENTS.md` records the cursor as `(line, byte offset)`. The offset is kept in
//! chars and converted with [`TextBuffer::char_to_byte`] because ropey indexes
//! lines by char, and a stored byte offset would have to be snapped back to a
//! char boundary on every line change — O(line) on a 5 MB single line. The
//! invariant that matters is kept: **no visual column is ever stored**; it is
//! derived through [`crate::buffer::Width`].

use super::{TextBuffer, Width};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    line: usize,
    col: usize,
}

impl Cursor {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    /// Zero-based line.
    pub fn line(&self) -> usize {
        self.line
    }

    /// Zero-based char offset inside the line.
    pub fn col(&self) -> usize {
        self.col
    }

    /// Global char index, for undo ranges and search.
    pub fn char_index(&self, buffer: &TextBuffer) -> usize {
        buffer.position_to_char(self.line, self.col)
    }

    /// Pull the cursor inside the buffer, keeping the line when possible.
    pub fn clamp(&mut self, buffer: &TextBuffer) {
        self.line = self.line.min(buffer.line_count() - 1);
        self.col = self.col.min(buffer.line_len(self.line));
    }

    /// Column the cursor is drawn at, tabs and wide glyphs included.
    pub fn visual_column(&self, buffer: &TextBuffer, tab_width: usize) -> usize {
        let start = buffer.line_start(self.line.min(buffer.line_count() - 1));
        let mut width = Width::new(0, tab_width);
        for char_idx in start..start + self.col {
            if let Some(ch) = buffer.char_at(char_idx) {
                width.push(ch);
            }
        }
        width.column()
    }

    /// Position for the status bar, one-based as nano shows it: visual column,
    /// not the char offset.
    pub fn display_position(&self, buffer: &TextBuffer, tab_width: usize) -> (usize, usize) {
        (
            self.line.saturating_add(1),
            self.visual_column(buffer, tab_width).saturating_add(1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Cursor;
    use crate::buffer::{DEFAULT_TAB_WIDTH, TextBuffer};

    #[test]
    fn clamps_to_the_last_line_and_its_length() {
        let buffer = TextBuffer::from_text("abc\nde\n");
        let mut cursor = Cursor::new(99, 99);
        cursor.clamp(&buffer);
        assert_eq!((cursor.line(), cursor.col()), (2, 0));
    }

    #[test]
    fn char_index_uses_the_line_start() {
        let buffer = TextBuffer::from_text("abc\ndef");
        let cursor = Cursor::new(1, 2);
        assert_eq!(cursor.char_index(&buffer), 6);
    }

    #[test]
    fn visual_column_expands_tabs_and_wide_glyphs() {
        let buffer = TextBuffer::from_text("a\tb漢c");
        assert_eq!(Cursor::new(0, 1).visual_column(&buffer, 8), 1);
        assert_eq!(Cursor::new(0, 2).visual_column(&buffer, 8), 8);
        assert_eq!(Cursor::new(0, 3).visual_column(&buffer, 8), 9);
        assert_eq!(Cursor::new(0, 4).visual_column(&buffer, 8), 11);
        assert_eq!(Cursor::new(0, 5).visual_column(&buffer, 8), 12);
    }

    #[test]
    fn display_position_is_one_based_and_visual() {
        let buffer = TextBuffer::from_text("a\tb");
        let cursor = Cursor::new(0, 3);
        assert_eq!(cursor.display_position(&buffer, DEFAULT_TAB_WIDTH), (1, 10));
    }

    #[test]
    fn deep_position_in_a_million_lines_does_not_scan_the_file() {
        let buffer = TextBuffer::from_text(&"x\n".repeat(1_000_000));
        let mut cursor = Cursor::new(999_999, 5);
        cursor.clamp(&buffer);
        assert_eq!((cursor.line(), cursor.col()), (999_999, 1));
        assert_eq!(cursor.char_index(&buffer), 999_999 * 2 + 1);
    }
}
