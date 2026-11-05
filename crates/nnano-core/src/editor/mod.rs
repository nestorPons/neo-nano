//! nano operations and the single mutation path (phase 3).
//!
//! [`Action`] lives here, not in the TUI: keyboard, mouse and the command palette
//! all produce it, so no surface can drift. [`Editor`] owns the buffer and the
//! cursor and applies those actions; it has no idea a terminal exists.
//!
//! Phase 3 covers typing, deletion and movement. Cut/paste (`^K`/`^U`), justify,
//! replace, goto and undo keep their own phases and are absent from `Action` until
//! they exist — no silent no-op variants.

use crate::buffer::{Cursor, DEFAULT_TAB_WIDTH, TextBuffer, char_offset_for_column};

/// Everything the editor can be told to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `^Q`. Confirmation belongs to the app loop.
    Quit,
    /// `^S`. The app loop owns the file: it has the path and the encoding.
    Save,
    /// `^L` or a resize: force a repaint, change nothing.
    Redraw,
    Insert(char),
    InsertTab,
    Newline,
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveLineStart,
    MoveLineEnd,
    MoveFileStart,
    MoveFileEnd,
    /// Needs the viewport height; the app loop turns it into
    /// [`Editor::move_vertical`].
    PageUp,
    PageDown,
}

/// Buffer + cursor + undo state (undo arrives in phase 7).
#[derive(Debug, Default)]
pub struct Editor {
    buffer: TextBuffer,
    cursor: Cursor,
    /// Visual column the cursor wants after a vertical move, so `Down` through a
    /// short line and back does not lose the column. Cleared by any edit or
    /// horizontal move.
    goal_col: Option<usize>,
    dirty: bool,
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            buffer: TextBuffer::from_text(text),
            ..Self::default()
        }
    }

    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// After a successful write: the buffer matches the file again.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Place the cursor somewhere (goto, tests, click). Clamped to the buffer.
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        self.cursor = Cursor::new(line, col);
        self.cursor.clamp(&self.buffer);
        self.goal_col = None;
    }

    /// Apply one action. Returns nothing: the caller repaints unconditionally.
    pub fn apply(&mut self, action: Action) {
        match action {
            // `Quit`/`Save` are owned by the app loop (it has the path and the
            // file), and pages need a viewport height.
            Action::Quit | Action::Save | Action::Redraw | Action::PageUp | Action::PageDown => {}
            Action::Insert(ch) => {
                let mut utf8 = [0; 4];
                let text = ch.encode_utf8(&mut utf8);
                self.insert(text);
            }
            Action::InsertTab => self.insert("\t"),
            Action::Newline => self.insert("\n"),
            Action::Backspace => self.backspace(),
            Action::Delete => self.delete_forward(),
            Action::MoveLeft => self.move_left(),
            Action::MoveRight => self.move_right(),
            Action::MoveUp => self.move_vertical(-1),
            Action::MoveDown => self.move_vertical(1),
            Action::MoveLineStart => {
                self.cursor = Cursor::new(self.cursor.line(), 0);
                self.goal_col = None;
            }
            Action::MoveLineEnd => {
                let line = self.cursor.line();
                self.cursor = Cursor::new(line, self.buffer.line_len(line));
                self.goal_col = None;
            }
            Action::MoveFileStart => {
                self.cursor = Cursor::new(0, 0);
                self.goal_col = None;
            }
            Action::MoveFileEnd => {
                let line = self.buffer.line_count() - 1;
                self.cursor = Cursor::new(line, self.buffer.line_len(line));
                self.goal_col = None;
            }
        }
    }

    /// Move `delta` lines, keeping the wanted visual column. Public because the
    /// app loop resolves `PageUp`/`PageDown` with the viewport height.
    pub fn move_vertical(&mut self, delta: isize) {
        let tab_width = DEFAULT_TAB_WIDTH;
        let goal = self
            .goal_col
            .unwrap_or_else(|| self.cursor.visual_column(&self.buffer, tab_width));
        let last_line = self.buffer.line_count() - 1;
        let line = self
            .cursor
            .line()
            .saturating_add_signed(delta)
            .min(last_line);
        let col = char_offset_for_column(self.buffer.line_chars(line), goal, tab_width);
        self.cursor = Cursor::new(line, col);
        self.goal_col = Some(goal);
    }

    fn insert(&mut self, text: &str) {
        let start = self.cursor.char_index(&self.buffer);
        self.buffer.insert_str(start, text);
        let (line, col) = self.buffer.char_to_position(start + text.chars().count());
        self.cursor = Cursor::new(line, col);
        self.touch();
    }

    /// Delete the char before the cursor, or join with the previous line.
    fn backspace(&mut self) {
        let (line, col) = (self.cursor.line(), self.cursor.col());
        if col > 0 {
            let index = self.cursor.char_index(&self.buffer);
            self.buffer.remove(index - 1..index);
            self.cursor = Cursor::new(line, col - 1);
        } else if line > 0 {
            let previous = line - 1;
            let previous_len = self.buffer.line_len(previous);
            // The terminator may be "\n", "\r\n" or any other rope line break.
            self.buffer
                .remove(self.buffer.line_end(previous)..self.buffer.line_start(line));
            self.cursor = Cursor::new(previous, previous_len);
        } else {
            return;
        }
        self.touch();
    }

    /// Delete the char under the cursor, or join with the next line.
    fn delete_forward(&mut self) {
        let line = self.cursor.line();
        let index = self.cursor.char_index(&self.buffer);
        if index < self.buffer.line_end(line) {
            self.buffer.remove(index..index + 1);
        } else if line + 1 < self.buffer.line_count() {
            self.buffer.remove(index..self.buffer.line_start(line + 1));
        } else {
            return;
        }
        self.touch();
    }

    fn move_left(&mut self) {
        let (line, col) = (self.cursor.line(), self.cursor.col());
        if col > 0 {
            self.cursor = Cursor::new(line, col - 1);
        } else if line > 0 {
            let previous = line - 1;
            self.cursor = Cursor::new(previous, self.buffer.line_len(previous));
        }
        self.goal_col = None;
    }

    fn move_right(&mut self) {
        let (line, col) = (self.cursor.line(), self.cursor.col());
        if col < self.buffer.line_len(line) {
            self.cursor = Cursor::new(line, col + 1);
        } else if line + 1 < self.buffer.line_count() {
            self.cursor = Cursor::new(line + 1, 0);
        }
        self.goal_col = None;
    }

    /// Any change to the text marks the buffer dirty and forgets the goal column.
    fn touch(&mut self) {
        self.dirty = true;
        self.goal_col = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Editor};

    fn editor(text: &str) -> Editor {
        Editor::from_text(text)
    }

    fn at(editor: &Editor) -> (usize, usize) {
        (editor.cursor().line(), editor.cursor().col())
    }

    fn text(editor: &Editor) -> String {
        (0..editor.buffer().line_count())
            .map(|line| editor.buffer().line_text(line))
            .collect::<Vec<_>>()
            .join("\\n")
    }

    #[test]
    fn typing_inserts_and_advances() {
        let mut editor = editor("");
        editor.apply(Action::Insert('h'));
        editor.apply(Action::Insert('i'));
        assert_eq!(text(&editor), "hi");
        assert_eq!(at(&editor), (0, 2));
        assert!(editor.is_dirty());
    }

    #[test]
    fn typing_in_the_middle_keeps_the_cursor_after_the_insert() {
        let mut editor = editor("ac");
        editor.set_cursor(0, 1);
        editor.apply(Action::Insert('b'));
        assert_eq!(text(&editor), "abc");
        assert_eq!(at(&editor), (0, 2));
    }

    #[test]
    fn newline_splits_the_line_and_moves_down() {
        let mut editor = editor("abc");
        editor.set_cursor(0, 1);
        editor.apply(Action::Newline);
        assert_eq!(text(&editor), "a\\nbc");
        assert_eq!(at(&editor), (1, 0));
    }

    #[test]
    fn tab_inserts_a_tab_character() {
        let mut editor = editor("");
        editor.apply(Action::InsertTab);
        assert_eq!(editor.buffer().line_text(0), "\t");
    }

    #[test]
    fn backspace_at_line_start_joins_lines_and_drops_the_terminator() {
        let mut editor = editor("uno\r\ndos");
        editor.set_cursor(1, 0);
        editor.apply(Action::Backspace);
        assert_eq!(text(&editor), "unodos");
        assert_eq!(at(&editor), (0, 3));
        assert_eq!(editor.buffer().line_count(), 1);
    }

    #[test]
    fn backspace_at_the_start_of_the_buffer_does_nothing() {
        let mut editor = editor("abc");
        editor.apply(Action::Backspace);
        assert_eq!(text(&editor), "abc");
        assert!(!editor.is_dirty());
    }

    #[test]
    fn delete_joins_with_the_next_line_and_handles_crlf() {
        let mut editor = editor("uno\r\ndos");
        editor.set_cursor(0, 3);
        editor.apply(Action::Delete);
        assert_eq!(text(&editor), "unodos");
        assert_eq!(at(&editor), (0, 3));
    }

    #[test]
    fn delete_at_the_end_of_the_buffer_does_nothing() {
        let mut editor = editor("abc");
        editor.set_cursor(0, 3);
        editor.apply(Action::Delete);
        assert_eq!(text(&editor), "abc");
        assert!(!editor.is_dirty());
    }

    #[test]
    fn horizontal_movement_crosses_line_boundaries() {
        let mut editor = editor("ab\ncd");
        editor.set_cursor(0, 2);
        editor.apply(Action::MoveRight);
        assert_eq!(at(&editor), (1, 0));
        editor.apply(Action::MoveLeft);
        assert_eq!(at(&editor), (0, 2));
    }

    #[test]
    fn home_and_end_stay_on_the_line() {
        let mut editor = editor("ab\ncd");
        editor.set_cursor(1, 1);
        editor.apply(Action::MoveLineStart);
        assert_eq!(at(&editor), (1, 0));
        editor.apply(Action::MoveLineEnd);
        assert_eq!(at(&editor), (1, 2));
    }

    #[test]
    fn file_start_and_end_jump_to_the_edges() {
        let mut editor = editor("ab\ncdef\ngh");
        editor.set_cursor(1, 2);
        editor.apply(Action::MoveFileEnd);
        assert_eq!(at(&editor), (2, 2));
        editor.apply(Action::MoveFileStart);
        assert_eq!(at(&editor), (0, 0));
    }

    #[test]
    fn vertical_movement_keeps_the_wanted_visual_column() {
        // 'a' at col 0, tab up to col 8, so col 2 sits at visual 8.
        let mut editor = editor("a\tbcdef\n0123456789\nxyz");
        editor.set_cursor(0, 2);
        editor.apply(Action::MoveDown);
        assert_eq!(at(&editor), (1, 8));
        editor.apply(Action::MoveDown);
        assert_eq!(at(&editor), (2, 3));
        editor.apply(Action::MoveUp);
        assert_eq!(at(&editor), (1, 8));
        editor.apply(Action::MoveUp);
        assert_eq!(at(&editor), (0, 2));
    }

    #[test]
    fn horizontal_movement_forgets_the_goal_column() {
        let mut editor = editor("0123456789\nxyz");
        editor.set_cursor(0, 8);
        editor.apply(Action::MoveDown);
        assert_eq!(at(&editor), (1, 3));
        editor.apply(Action::MoveDown);
        assert_eq!(at(&editor), (1, 3));
    }

    #[test]
    fn vertical_movement_clamps_at_the_edges() {
        let mut editor = editor("uno\ndos");
        editor.set_cursor(0, 2);
        editor.move_vertical(-50);
        assert_eq!(at(&editor), (0, 2));
        editor.move_vertical(50);
        assert_eq!(at(&editor), (1, 2));
        assert_eq!(editor.cursor().line(), 1);
    }

    #[test]
    fn saved_buffer_is_clean_until_the_next_edit() {
        let mut editor = editor("");
        assert!(!editor.is_dirty());
        editor.apply(Action::Insert('x'));
        assert!(editor.is_dirty());
        editor.mark_saved();
        assert!(!editor.is_dirty());
        editor.apply(Action::MoveLeft);
        assert!(!editor.is_dirty());
        editor.apply(Action::Delete);
        assert!(editor.is_dirty());
    }
}
