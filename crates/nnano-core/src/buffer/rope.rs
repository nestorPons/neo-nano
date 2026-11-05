//! The rope wrapper: line-oriented lookups on top of `ropey`.

use ropey::{Rope, RopeSlice};

use std::ops::Range;

/// Text of one buffer.
///
/// Lines are zero-indexed and a trailing newline opens an empty final line, as
/// in nano. Line breaks follow ropey's Unicode set (`\n`, `\r`, `\r\n`, VT, FF,
/// NEL, LS, PS); `\r\n` is kept in the buffer and [`TextBuffer::line_len`] /
/// [`TextBuffer::line_text`] treat the two bytes as one terminator, so the cursor
/// never lands between `\r` and `\n`.
#[derive(Debug, Default, Clone)]
pub struct TextBuffer {
    rope: Rope,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// One O(n) pass: the rope is built from the whole text at once.
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    /// Number of lines, always at least 1.
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Number of chars in the buffer.
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// True when there is nothing to show: no chars at all.
    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Char index where `line` starts. Clamped to the end of the buffer.
    pub fn line_start(&self, line: usize) -> usize {
        self.rope.line_to_char(line.min(self.line_count()))
    }

    /// Char index of the terminator that ends `line`, or of the end of the
    /// buffer. `\n` and `\r\n` are excluded.
    pub fn line_end(&self, line: usize) -> usize {
        let mut end = self.rope.line_to_char((line + 1).min(self.line_count()));
        if end > 0 && self.rope.char(end - 1) == '\n' {
            end -= 1;
        }
        if end > 0 && self.rope.char(end - 1) == '\r' {
            end -= 1;
        }
        end
    }

    /// Chars in `line`, terminator excluded.
    pub fn line_len(&self, line: usize) -> usize {
        self.line_end(line).saturating_sub(self.line_start(line))
    }

    /// The raw line, terminator included. Clamped to the last line.
    pub fn line_slice(&self, line: usize) -> RopeSlice<'_> {
        self.rope.line(line.min(self.line_count() - 1))
    }

    /// Chars of `line` without its terminator, as an iterator.
    pub fn line_chars(&self, line: usize) -> impl Iterator<Item = char> + '_ {
        let len = self.line_len(line);
        self.line_slice(line).chars().take(len)
    }

    /// Insert `text` at a char index; O(log n) wherever it lands.
    pub fn insert_str(&mut self, char_idx: usize, text: &str) {
        let idx = char_idx.min(self.len_chars());
        self.rope.insert(idx, text);
    }

    /// Remove `range` of chars; an empty or inverted range does nothing.
    pub fn remove(&mut self, range: Range<usize>) {
        let start = range.start.min(self.len_chars());
        let end = range.end.min(self.len_chars()).max(start);
        if start < end {
            self.rope.remove(start..end);
        }
    }

    /// The line as a string, terminator excluded. Meant for status, tests and
    /// save-as-text; rendering reads the slice directly.
    pub fn line_text(&self, line: usize) -> String {
        let mut text: String = self.line_slice(line).chars().collect();
        if text.ends_with('\n') {
            text.pop();
        }
        if text.ends_with('\r') {
            text.pop();
        }
        text
    }

    /// Char index -> `(line, char offset in line)`.
    pub fn char_to_position(&self, char_idx: usize) -> (usize, usize) {
        let idx = char_idx.min(self.len_chars());
        let line = self.rope.char_to_line(idx);
        (line, idx - self.line_start(line))
    }

    /// `(line, char offset in line)` -> char index, clamped to the line and to
    /// the buffer.
    pub fn position_to_char(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.line_count() - 1);
        self.line_start(line) + col.min(self.line_len(line))
    }

    /// Char at a char index, `None` past the end.
    pub fn char_at(&self, char_idx: usize) -> Option<char> {
        self.rope.get_char(char_idx)
    }

    /// Byte index of a char index; the save/render edges convert here.
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx.min(self.len_chars()))
    }
}

#[cfg(test)]
mod tests {
    use super::TextBuffer;

    #[test]
    fn empty_buffer_has_one_empty_line() {
        let buffer = TextBuffer::new();
        assert_eq!(buffer.line_count(), 1);
        assert!(buffer.is_empty());
        assert_eq!(buffer.line_len(0), 0);
        assert_eq!(buffer.char_to_position(0), (0, 0));
    }

    #[test]
    fn trailing_newline_opens_an_empty_line() {
        let buffer = TextBuffer::from_text("a\nb\n");
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line_text(0), "a");
        assert_eq!(buffer.line_text(1), "b");
        assert_eq!(buffer.line_text(2), "");
        assert_eq!(buffer.line_len(1), 1);
    }

    #[test]
    fn crlf_counts_as_one_terminator() {
        let buffer = TextBuffer::from_text("uno\r\ndos\r\n");
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line_len(0), 3);
        assert_eq!(buffer.line_text(0), "uno");
        assert_eq!(buffer.line_text(1), "dos");
        // The cursor stops before the terminator, not between \r and \n.
        assert_eq!(buffer.position_to_char(1, 99), buffer.line_start(1) + 3);
    }

    #[test]
    fn lone_cr_is_a_line_break_not_a_visible_char() {
        // ropey follows the Unicode line-break set (\n, \r, \r\n, VT, FF, NEL,
        // LS, PS), so an old-Mac file is two lines and the terminator is stripped
        // exactly like CRLF's. Phase 4 must save terminators, not re-emit '\n'.
        let buffer = TextBuffer::from_text("a\rb");
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.line_text(0), "a");
        assert_eq!(buffer.line_text(1), "b");
        assert_eq!(buffer.line_len(0), 1);
    }

    #[test]
    fn positions_and_chars_round_trip() {
        let buffer = TextBuffer::from_text("ñ\n漢字\n");
        for char_idx in 0..=buffer.len_chars() {
            let (line, col) = buffer.char_to_position(char_idx);
            assert_eq!(buffer.position_to_char(line, col), char_idx);
        }
        assert_eq!(buffer.char_to_position(2), (1, 0));
        assert_eq!(buffer.char_to_byte(2), 3); // "ñ\n" is 3 bytes
    }

    #[test]
    fn positions_clamp_past_the_end() {
        let buffer = TextBuffer::from_text("abc");
        assert_eq!(buffer.position_to_char(9, 9), 3);
        assert_eq!(buffer.char_to_position(9), (0, 3));
        assert_eq!(buffer.line_text(9), "abc");
        assert_eq!(buffer.char_at(3), None);
    }

    #[test]
    fn single_line_of_five_megabytes_is_indexed_in_one_pass() {
        let text = "a".repeat(5 * 1024 * 1024);
        let buffer = TextBuffer::from_text(&text);
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.line_len(0), text.chars().count());
        assert_eq!(
            buffer.position_to_char(0, buffer.line_len(0)),
            buffer.len_chars()
        );
        assert_eq!(buffer.char_to_position(buffer.len_chars()), (0, text.len()));
    }

    #[test]
    fn one_million_lines_lookup_without_scanning() {
        let text = "x\n".repeat(1_000_000);
        let buffer = TextBuffer::from_text(&text);
        assert_eq!(buffer.line_count(), 1_000_001);
        assert_eq!(buffer.line_text(999_999), "x");
        assert_eq!(buffer.line_start(1_000_000), buffer.len_chars());
        assert_eq!(
            buffer.char_to_position(buffer.line_start(500_000)),
            (500_000, 0)
        );
    }
}
