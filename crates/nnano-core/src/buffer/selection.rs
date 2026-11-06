//! Selection: a stream (char run) or a rectangular block.
//!
//! The selection is **half-open** — `[anchor, head)` in char indices — so
//! "select all" is exactly the buffer, and a cursor sitting at a line end (after
//! the last char) does not select a phantom character. The anchor stays put while
//! the head moves, which is what marking means.
//!
//! Copy/extraction works on chars, the same currency as the cursor; `\r\n` is
//! copied as stored, because saving never transcodes.

use super::{Cursor, TextBuffer};

/// Stream selection follows the text; block selection is a rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Stream,
    Block,
}

/// Anchor (fixed end) plus head (moves with the cursor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    anchor: Cursor,
    head: Cursor,
    mode: Mode,
}

impl Selection {
    pub fn new(anchor: Cursor, head: Cursor, mode: Mode) -> Self {
        Self { anchor, head, mode }
    }

    pub fn anchor(&self) -> Cursor {
        self.anchor
    }

    pub fn head(&self) -> Cursor {
        self.head
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_head(&mut self, head: Cursor) {
        self.head = head;
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Nothing selected: anchor and head are the same cursor.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Char range `[start, end)` of a stream selection, normalized.
    pub fn char_range(&self, buffer: &TextBuffer) -> (usize, usize) {
        let anchor = self.anchor.char_index(buffer);
        let head = self.head.char_index(buffer);
        if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        }
    }

    /// Normalized block bounds: `(first_line, last_line, first_col, last_col)`,
    /// columns half-open.
    pub fn block_bounds(&self) -> (usize, usize, usize, usize) {
        let (first_line, last_line) = if self.anchor.line() <= self.head.line() {
            (self.anchor.line(), self.head.line())
        } else {
            (self.head.line(), self.anchor.line())
        };
        let (first_col, last_col) = if self.anchor.col() <= self.head.col() {
            (self.anchor.col(), self.head.col())
        } else {
            (self.head.col(), self.anchor.col())
        };
        (first_line, last_line, first_col, last_col)
    }

    /// Char columns to highlight on `line`, half-open. `None` when the line is
    /// outside the selection.
    pub fn line_span(&self, buffer: &TextBuffer, line: usize) -> Option<(usize, usize)> {
        match self.mode {
            Mode::Stream => {
                let (start, end) = self.char_range(buffer);
                let line_start = buffer.line_start(line);
                let line_end = buffer.line_end(line);
                let from = start.max(line_start);
                let to = end.min(line_end);
                (from < to).then(|| (from - line_start, to - line_start))
            }
            Mode::Block => {
                let (first, last, left, right) = self.block_bounds();
                if line < first || line > last {
                    return None;
                }
                let len = buffer.line_len(line);
                let from = left.min(len);
                let to = right.min(len);
                (from < to).then_some((from, to))
            }
        }
    }

    /// Selected text. Stream copies the chars as stored; a block copies one slice
    /// per line, joined with `\n` (a block has no line terminators of its own).
    pub fn text(&self, buffer: &TextBuffer) -> String {
        match self.mode {
            Mode::Stream => {
                let (start, end) = self.char_range(buffer);
                (start..end)
                    .filter_map(|index| buffer.char_at(index))
                    .collect()
            }
            Mode::Block => {
                let (first, last, _, _) = self.block_bounds();
                (first..=last)
                    .map(|line| {
                        let Some((from, to)) = self.line_span(buffer, line) else {
                            return String::new();
                        };
                        let base = buffer.line_start(line);
                        (base + from..base + to)
                            .filter_map(|index| buffer.char_at(index))
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Mode, Selection};
    use crate::buffer::{Cursor, TextBuffer};

    fn buffer() -> TextBuffer {
        TextBuffer::from_text("uno\ndos tres\ncuatro\n")
    }

    #[test]
    fn a_stream_selection_is_half_open_and_normalized() {
        let buffer = buffer();
        let selection = Selection::new(Cursor::new(1, 0), Cursor::new(0, 0), Mode::Stream);
        assert_eq!(selection.char_range(&buffer), (0, 4));
        assert_eq!(selection.text(&buffer), "uno\n");
    }

    #[test]
    fn a_stream_selection_covers_partial_lines() {
        let buffer = buffer();
        let selection = Selection::new(Cursor::new(0, 2), Cursor::new(1, 3), Mode::Stream);
        assert_eq!(selection.line_span(&buffer, 0), Some((2, 3)));
        assert_eq!(selection.line_span(&buffer, 1), Some((0, 3)));
        assert_eq!(selection.line_span(&buffer, 2), None);
        assert_eq!(selection.text(&buffer), "o\ndos");
    }

    #[test]
    fn selecting_everything_is_exactly_the_buffer() {
        let buffer = buffer();
        let selection = Selection::new(Cursor::new(0, 0), Cursor::new(3, 0), Mode::Stream);
        assert_eq!(selection.text(&buffer), "uno\ndos tres\ncuatro\n");
    }

    #[test]
    fn a_block_selection_is_a_rectangle_over_whole_lines() {
        let buffer = buffer();
        let selection = Selection::new(Cursor::new(0, 1), Cursor::new(2, 3), Mode::Block);
        assert_eq!(selection.line_span(&buffer, 0), Some((1, 3)));
        assert_eq!(selection.line_span(&buffer, 1), Some((1, 3)));
        assert_eq!(selection.line_span(&buffer, 2), Some((1, 3)));
        assert_eq!(selection.text(&buffer), "no\nos\nua");
    }

    #[test]
    fn a_block_clamps_at_short_lines() {
        let buffer = TextBuffer::from_text("abc\nx\nabcdef\n");
        let selection = Selection::new(Cursor::new(0, 1), Cursor::new(2, 4), Mode::Block);
        assert_eq!(
            selection.line_span(&buffer, 1),
            None,
            "x has nothing at col 1"
        );
        assert_eq!(selection.text(&buffer), "bc\n\nbcd");
    }

    #[test]
    fn an_empty_selection_selects_nothing() {
        let buffer = buffer();
        let selection = Selection::new(Cursor::new(1, 2), Cursor::new(1, 2), Mode::Stream);
        assert!(selection.is_empty());
        assert_eq!(selection.text(&buffer), "");
        assert_eq!(selection.line_span(&buffer, 1), None);
    }
}
