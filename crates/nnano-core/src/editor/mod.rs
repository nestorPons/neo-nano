//! nano operations and the single mutation path (phase 3).
//!
//! [`Action`] lives here, not in the TUI: keyboard, mouse and the command palette
//! all produce it, so no surface can drift. [`Editor`] owns the buffer and the
//! cursor and applies those actions; it has no idea a terminal exists.
//!
//! Phase 3 covers typing, deletion and movement; phase 5 adds the overlay and
//! search actions. Cut/paste (`^K`/`^U`), justify, replace, goto and undo keep
//! their own phases and are absent from `Action` until they exist — no silent
//! no-op variants.

use std::time::Instant;

use crate::buffer::{
    Cursor, DEFAULT_TAB_WIDTH, Edit, History, Selection, SelectionMode, TextBuffer,
    char_offset_for_column,
};

/// Everything the editor can be told to do.
///
/// Not `Copy`: a bracketed paste carries its text. Everything else is a plain
/// enum variant, and the payload actions are the only ones that allocate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `^Q`. Confirmation belongs to the app loop.
    Quit,
    /// `^S`. The app loop owns the file: it has the path and the encoding.
    Save,
    /// `^L` or a resize: force a repaint, change nothing.
    Redraw,
    /// `^F`: open the find prompt.
    OpenFind,
    /// `^P`: open the command palette.
    OpenPalette,
    /// `M-,`: open the settings overlay.
    OpenSettings,
    /// `F1`: open the help overlay.
    OpenHelp,
    /// `Esc`: close the overlay, or cancel a pending confirmation.
    Cancel,
    /// `M-C` inside the find prompt: toggle case sensitivity.
    ToggleCase,
    /// `M-R` inside the find prompt: toggle regexp mode.
    ToggleRegexp,
    /// `M-B` inside the find prompt: toggle the search direction.
    ToggleBackwards,
    /// `M-W`/`F3`: repeat the last search forwards.
    FindNext,
    /// `M-B`/`Shift+F3`: repeat the last search backwards.
    FindPrevious,
    /// `M-A`: start or clear the mark.
    StartMark,
    /// `^A`: select the whole buffer.
    SelectAll,
    /// `M-Z`: make the mark rectangular (or the next one, if there is none).
    ToggleBlockSelection,
    /// `^C` / `M-6`: copy the marked region, or the current line.
    Copy,
    /// `^X`: cut the marked region, or the current line.
    Cut,
    /// `^K`: cut the mark, or the current line (consecutive cuts accumulate).
    CutLine,
    /// `^V` / `^U`: paste the internal register.
    Paste,
    /// Bracketed paste from the terminal: insert this text as one undo step.
    PasteText(String),
    /// Mouse press: place the cursor and drop the mark.
    ClickAt(usize, usize),
    /// Mouse drag: extend the mark to this position.
    ExtendTo(usize, usize),
    /// Mouse wheel: scroll the viewport without moving the cursor.
    ScrollView(isize),
    /// `^Z` / `M-U`: undo the last group.
    Undo,
    /// `^Y` / `M-E`: redo the last undone group.
    Redo,
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
#[derive(Debug)]
pub struct Editor {
    buffer: TextBuffer,
    cursor: Cursor,
    /// Visual column the cursor wants after a vertical move, so `Down` through a
    /// short line and back does not lose the column. Cleared by any edit or
    /// horizontal move.
    goal_col: Option<usize>,
    dirty: bool,
    /// `tab_size` from the registry, kept here so vertical movement and the goal
    /// column use the same arithmetic as the renderer.
    tab_width: usize,
    /// The mark, when one is active.
    selection: Option<Selection>,
    /// Whether the next mark is rectangular.
    block_mode: bool,
    /// Internal clipboard. Always available; OSC 52 is an opt-in copy *out*, not
    /// the storage.
    register: String,
    /// Undo/redo stacks and their grouping clock.
    history: History,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            buffer: TextBuffer::new(),
            cursor: Cursor::default(),
            goal_col: None,
            dirty: false,
            tab_width: DEFAULT_TAB_WIDTH,
            selection: None,
            block_mode: false,
            register: String::new(),
            history: History::default(),
        }
    }
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

    /// Follow the `tab_size` option. The caller re-renders; nothing here is
    /// cached against the old width beyond the next vertical move.
    pub fn set_tab_width(&mut self, tab_width: usize) {
        self.tab_width = tab_width.max(1);
    }

    pub fn tab_width(&self) -> usize {
        self.tab_width
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

    /// After a successful write: the buffer matches the file again, and saving is
    /// a hard undo boundary.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.history.break_group();
    }

    /// Place the cursor somewhere (goto, tests, click). Clamped to the buffer,
    /// and it drops the mark.
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        self.cursor = Cursor::new(line, col);
        self.cursor.clamp(&self.buffer);
        self.selection = None;
        self.goal_col = None;
    }

    /// Apply one action. Returns nothing: the caller repaints unconditionally.
    pub fn apply(&mut self, action: Action) {
        match action {
            // `Quit`/`Save` are owned by the app loop (it has the path and the
            // file), pages and scrolling need viewport geometry, and the rest are
            // overlay or search actions owned by `App`.
            Action::Quit
            | Action::Save
            | Action::Redraw
            | Action::OpenFind
            | Action::OpenPalette
            | Action::OpenSettings
            | Action::OpenHelp
            | Action::Cancel
            | Action::ToggleCase
            | Action::ToggleRegexp
            | Action::ToggleBackwards
            | Action::FindNext
            | Action::FindPrevious
            | Action::ScrollView(_)
            | Action::PageUp
            | Action::PageDown => {}
            Action::Insert(ch) => {
                let mut utf8 = [0; 4];
                let text = ch.encode_utf8(&mut utf8);
                self.insert(text);
            }
            Action::InsertTab => self.insert("\t"),
            Action::Newline => self.insert("\n"),
            Action::Backspace => self.backspace(),
            Action::Delete => self.delete_forward(),
            Action::MoveLeft => {
                self.move_left();
                self.follow();
            }
            Action::MoveRight => {
                self.move_right();
                self.follow();
            }
            Action::MoveUp => self.move_vertical(-1),
            Action::MoveDown => self.move_vertical(1),
            Action::MoveLineStart => {
                self.cursor = Cursor::new(self.cursor.line(), 0);
                self.goal_col = None;
                self.follow();
            }
            Action::MoveLineEnd => {
                let line = self.cursor.line();
                self.cursor = Cursor::new(line, self.buffer.line_len(line));
                self.goal_col = None;
                self.follow();
            }
            Action::MoveFileStart => {
                self.cursor = Cursor::new(0, 0);
                self.goal_col = None;
                self.follow();
            }
            Action::MoveFileEnd => {
                let line = self.buffer.line_count() - 1;
                self.cursor = Cursor::new(line, self.buffer.line_len(line));
                self.goal_col = None;
                self.follow();
            }
            Action::StartMark => self.toggle_mark(),
            Action::SelectAll => self.select_all(),
            Action::ToggleBlockSelection => self.toggle_block(),
            Action::Copy => self.copy(),
            Action::Cut => self.cut(),
            Action::CutLine => self.cut_line(),
            Action::Paste => self.paste(),
            Action::PasteText(text) => self.paste_text(&text),
            Action::ClickAt(line, col) => self.set_cursor(line, col),
            Action::ExtendTo(line, col) => self.extend_to(line, col),
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
        }
    }

    /// Move `delta` lines, keeping the wanted visual column. Public because the
    /// app loop resolves `PageUp`/`PageDown` with the viewport height.
    pub fn move_vertical(&mut self, delta: isize) {
        let tab_width = self.tab_width;
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
        self.follow();
    }

    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    pub fn has_selection(&self) -> bool {
        self.selection
            .is_some_and(|selection| !selection.is_empty())
    }

    pub fn block_mode(&self) -> bool {
        self.block_mode
    }

    /// Internal clipboard, for the status bar and tests.
    pub fn register(&self) -> &str {
        &self.register
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// A new mark starts at the cursor; `M-A` again clears it.
    fn toggle_mark(&mut self) {
        if self.selection.is_some() {
            self.selection = None;
        } else {
            let mode = self.mark_mode();
            self.selection = Some(Selection::new(self.cursor, self.cursor, mode));
        }
    }

    fn select_all(&mut self) {
        let line = self.buffer.line_count() - 1;
        let end = Cursor::new(line, self.buffer.line_len(line));
        self.selection = Some(Selection::new(Cursor::new(0, 0), end, self.mark_mode()));
        self.cursor = end;
        self.goal_col = None;
    }

    /// `M-Z`: rectangular marks from now on, and the active one switches too.
    fn toggle_block(&mut self) {
        self.block_mode = !self.block_mode;
        let mode = self.mark_mode();
        if let Some(mut selection) = self.selection {
            selection.set_mode(mode);
            self.selection = Some(selection);
        }
    }

    fn mark_mode(&self) -> SelectionMode {
        if self.block_mode {
            SelectionMode::Block
        } else {
            SelectionMode::Stream
        }
    }

    /// After a movement while marking, the head follows the cursor.
    fn follow(&mut self) {
        if let Some(mut selection) = self.selection {
            selection.set_head(self.cursor);
            self.selection = Some(selection);
        }
    }

    /// Mouse drag: start a mark at the press position if needed, then extend it.
    fn extend_to(&mut self, line: usize, col: usize) {
        let mut head = Cursor::new(line, col);
        head.clamp(&self.buffer);
        match self.selection {
            Some(mut selection) => {
                selection.set_head(head);
                self.selection = Some(selection);
            }
            None => {
                let mode = self.mark_mode();
                self.selection = Some(Selection::new(self.cursor, head, mode));
            }
        }
        self.cursor = head;
        self.goal_col = None;
    }

    fn copy(&mut self) {
        match self.selection {
            Some(selection) if !selection.is_empty() => {
                self.register = selection.text(&self.buffer)
            }
            _ => self.register = self.line_with_terminator(self.cursor.line()),
        }
    }

    /// `^X`: the mark when there is one, otherwise the current line.
    fn cut(&mut self) {
        if let Some(selection) = self.selection.filter(|selection| !selection.is_empty()) {
            let text = selection.text(&self.buffer);
            if !text.is_empty() {
                self.register = text;
            }
            let before = self.cursor;
            let edits = self.delete_selection_now();
            self.history.record_group(edits, before, self.cursor);
            return;
        }
        self.cut_current_line(false);
    }

    /// `^K`: the mark when there is one, otherwise the line; consecutive line
    /// cuts accumulate in the register, as in nano.
    fn cut_line(&mut self) {
        if self.has_selection() {
            self.cut();
            return;
        }
        self.cut_current_line(true);
    }

    /// Cut the whole line including its terminator, and leave the cursor at its
    /// start. `append` is the nano `^K` behaviour.
    fn cut_current_line(&mut self, append: bool) {
        let before = self.cursor;
        let line = self.cursor.line();
        let start = self.buffer.line_start(line);
        let end = if line + 1 < self.buffer.line_count() {
            self.buffer.line_start(line + 1)
        } else {
            self.buffer.line_end(line)
        };
        let text: String = (start..end)
            .filter_map(|index| self.buffer.char_at(index))
            .collect();
        if text.is_empty() {
            return;
        }
        if append {
            self.register.push_str(&text);
        } else {
            self.register = text.clone();
        }
        self.buffer.remove(start..end);
        let line = line.min(self.buffer.line_count() - 1);
        self.cursor = Cursor::new(line, 0);
        self.selection = None;
        self.touch();
        self.history
            .record_group(vec![Edit::remove(start, text)], before, self.cursor);
    }

    /// The line plus its terminator, for a copy with no mark.
    fn line_with_terminator(&self, line: usize) -> String {
        let start = self.buffer.line_start(line);
        let end = if line + 1 < self.buffer.line_count() {
            self.buffer.line_start(line + 1)
        } else {
            self.buffer.line_end(line)
        };
        (start..end)
            .filter_map(|index| self.buffer.char_at(index))
            .collect()
    }

    fn paste(&mut self) {
        if self.register.is_empty() {
            return;
        }
        let text = self.register.clone();
        self.insert(&text);
    }

    /// A bracketed paste is one undo step, never merged with typing.
    fn paste_text(&mut self, text: &str) {
        self.history.break_group();
        self.insert(text);
        self.history.break_group();
    }

    /// Undo one group; a no-op at the start of history.
    fn undo(&mut self) {
        if let Some(cursor) = self.history.undo(&mut self.buffer) {
            self.cursor = cursor;
            self.cursor.clamp(&self.buffer);
            self.selection = None;
            self.goal_col = None;
            self.dirty = true;
        }
    }

    fn redo(&mut self) {
        if let Some(cursor) = self.history.redo(&mut self.buffer) {
            self.cursor = cursor;
            self.cursor.clamp(&self.buffer);
            self.selection = None;
            self.goal_col = None;
            self.dirty = true;
        }
    }

    fn insert(&mut self, text: &str) {
        let before = self.cursor;
        let removed = if self.has_selection() {
            self.delete_selection_now()
        } else {
            Vec::new()
        };
        let start = self.cursor.char_index(&self.buffer);
        self.buffer.insert_str(start, text);
        let (line, col) = self.buffer.char_to_position(start + text.chars().count());
        self.cursor = Cursor::new(line, col);
        self.touch();
        let insert = Edit::insert(start, text);
        if removed.is_empty() {
            self.history
                .record(insert, before, self.cursor, Instant::now());
        } else {
            let mut edits = removed;
            edits.push(insert);
            self.history.record_group(edits, before, self.cursor);
        }
    }

    /// Remove the marked region, returning the edits for the history. The cursor
    /// lands at the start of what was removed.
    fn delete_selection_now(&mut self) -> Vec<Edit> {
        let Some(selection) = self.selection.take() else {
            return Vec::new();
        };
        match selection.mode() {
            SelectionMode::Stream => {
                let (start, end) = selection.char_range(&self.buffer);
                let removed = selection.text(&self.buffer);
                if removed.is_empty() {
                    return Vec::new();
                }
                self.buffer.remove(start..end);
                self.cursor = self.position(start);
                self.touch();
                vec![Edit::remove(start, removed)]
            }
            SelectionMode::Block => {
                let (first, _, left, _) = selection.block_bounds();
                let mut edits = Vec::new();
                for line in (first..=selection.block_bounds().1).rev() {
                    let Some((from, to)) = selection.line_span(&self.buffer, line) else {
                        continue;
                    };
                    let start = self.buffer.line_start(line) + from;
                    let removed: String = (start..start + (to - from))
                        .filter_map(|index| self.buffer.char_at(index))
                        .collect();
                    self.buffer.remove(start..start + (to - from));
                    edits.push(Edit::remove(start, removed));
                }
                self.cursor = Cursor::new(first, left);
                self.cursor.clamp(&self.buffer);
                self.touch();
                edits
            }
        }
    }

    fn position(&self, char_index: usize) -> Cursor {
        let (line, col) = self.buffer.char_to_position(char_index);
        Cursor::new(line, col)
    }

    /// Delete the char before the cursor, or join with the previous line.
    fn backspace(&mut self) {
        if self.has_selection() {
            let before = self.cursor;
            let edits = self.delete_selection_now();
            self.history.record_group(edits, before, self.cursor);
            return;
        }
        let (line, col) = (self.cursor.line(), self.cursor.col());
        let before = self.cursor;
        if col > 0 {
            let index = self.cursor.char_index(&self.buffer);
            let removed: String = (index - 1..index)
                .filter_map(|at| self.buffer.char_at(at))
                .collect();
            self.buffer.remove(index - 1..index);
            self.cursor = Cursor::new(line, col - 1);
            self.touch();
            self.history.record(
                Edit::remove(index - 1, removed),
                before,
                self.cursor,
                Instant::now(),
            );
        } else if line > 0 {
            let previous = line - 1;
            let previous_len = self.buffer.line_len(previous);
            let start = self.buffer.line_end(previous);
            let end = self.buffer.line_start(line);
            let removed: String = (start..end)
                .filter_map(|at| self.buffer.char_at(at))
                .collect();
            // The terminator may be "\n", "\r\n" or any other rope line break.
            self.buffer.remove(start..end);
            self.cursor = Cursor::new(previous, previous_len);
            self.touch();
            self.history.record(
                Edit::remove(start, removed),
                before,
                self.cursor,
                Instant::now(),
            );
        }
    }

    /// Delete the char under the cursor, or join with the next line.
    fn delete_forward(&mut self) {
        if self.has_selection() {
            let before = self.cursor;
            let edits = self.delete_selection_now();
            self.history.record_group(edits, before, self.cursor);
            return;
        }
        let line = self.cursor.line();
        let index = self.cursor.char_index(&self.buffer);
        let before = self.cursor;
        let (start, end) = if index < self.buffer.line_end(line) {
            (index, index + 1)
        } else if line + 1 < self.buffer.line_count() {
            (index, self.buffer.line_start(line + 1))
        } else {
            return;
        };
        let removed: String = (start..end)
            .filter_map(|at| self.buffer.char_at(at))
            .collect();
        self.buffer.remove(start..end);
        self.touch();
        self.history.record(
            Edit::remove(start, removed),
            before,
            self.cursor,
            Instant::now(),
        );
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

    /// Multi-line text with real newlines; `text()` above keeps its escaped
    /// separators for legacy assertions.
    fn plain(editor: &Editor) -> String {
        (0..editor.buffer().line_count())
            .map(|line| editor.buffer().line_text(line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn select(editor: &mut Editor, line: usize, col: usize, steps: usize) {
        editor.set_cursor(line, col);
        editor.apply(Action::StartMark);
        for _ in 0..steps {
            editor.apply(Action::MoveRight);
        }
    }

    #[test]
    fn movement_with_a_mark_extends_the_selection() {
        let mut editor = editor("uno dos");
        select(&mut editor, 0, 0, 3);
        let selection = editor.selection().unwrap();
        assert_eq!(selection.text(editor.buffer()), "uno");
        assert_eq!(at(&editor), (0, 3));
    }

    #[test]
    fn mark_again_clears_the_selection() {
        let mut editor = editor("uno");
        select(&mut editor, 0, 0, 2);
        assert!(editor.selection().is_some());
        editor.apply(Action::StartMark);
        assert!(editor.selection().is_none());
    }

    #[test]
    fn select_all_and_copy_takes_the_whole_buffer() {
        let mut editor = editor("uno\ndos\n");
        editor.apply(Action::SelectAll);
        editor.apply(Action::Copy);
        assert_eq!(editor.register(), "uno\ndos\n");
    }

    #[test]
    fn copy_without_a_mark_takes_the_line_and_its_terminator() {
        let mut editor = editor("uno\ndos\n");
        editor.set_cursor(0, 1);
        editor.apply(Action::Copy);
        assert_eq!(editor.register(), "uno\n");
    }

    #[test]
    fn cut_removes_the_mark_and_paste_puts_it_back() {
        let mut editor = editor("uno dos");
        select(&mut editor, 0, 0, 3);
        editor.apply(Action::Cut);
        assert_eq!(plain(&editor), " dos");
        assert_eq!(editor.register(), "uno");
        assert_eq!(at(&editor), (0, 0));
        editor.apply(Action::Paste);
        assert_eq!(plain(&editor), "uno dos");
        assert_eq!(at(&editor), (0, 3));
    }

    #[test]
    fn typing_over_a_selection_replaces_it() {
        let mut editor = editor("uno dos");
        select(&mut editor, 0, 0, 3);
        editor.apply(Action::Insert('X'));
        assert_eq!(plain(&editor), "X dos");
        assert!(editor.selection().is_none());
    }

    #[test]
    fn cut_line_accumulates_like_nano() {
        let mut editor = editor("uno\ndos\ntres\n");
        editor.set_cursor(0, 0);
        editor.apply(Action::CutLine);
        editor.apply(Action::CutLine);
        assert_eq!(plain(&editor), "tres\n");
        assert_eq!(editor.register(), "uno\ndos\n");
        editor.apply(Action::Paste);
        assert_eq!(plain(&editor), "uno\ndos\ntres\n");
    }

    #[test]
    fn cutting_the_last_line_without_a_terminator_works() {
        let mut editor = editor("uno\ndos");
        editor.set_cursor(1, 0);
        editor.apply(Action::CutLine);
        assert_eq!(plain(&editor), "uno\n");
        assert_eq!(editor.register(), "dos");
    }

    #[test]
    fn a_block_selection_is_rectangular() {
        let mut editor = editor("uno\ndos\ntres");
        editor.apply(Action::ToggleBlockSelection);
        editor.set_cursor(0, 0);
        editor.apply(Action::StartMark);
        editor.apply(Action::MoveRight);
        editor.apply(Action::MoveRight);
        editor.apply(Action::MoveDown);
        let selection = editor.selection().unwrap();
        assert_eq!(selection.text(editor.buffer()), "un\ndo");
        editor.apply(Action::Copy);
        assert_eq!(editor.register(), "un\ndo");
    }

    #[test]
    fn block_cut_removes_a_rectangle_from_every_line() {
        let mut editor = editor("abc\ndef\nghi");
        editor.apply(Action::ToggleBlockSelection);
        editor.set_cursor(0, 1);
        editor.apply(Action::StartMark);
        editor.apply(Action::MoveRight);
        editor.apply(Action::MoveRight);
        editor.apply(Action::MoveDown);
        editor.apply(Action::MoveDown);
        editor.apply(Action::Cut);
        assert_eq!(plain(&editor), "a\nd\ng");
        assert_eq!(editor.register(), "bc\nef\nhi");
    }

    #[test]
    fn undo_and_redo_walk_the_grouped_history() {
        let mut editor = editor("");
        editor.apply(Action::Insert('h'));
        editor.apply(Action::Insert('i'));
        assert!(editor.can_undo());
        editor.apply(Action::Undo);
        assert_eq!(plain(&editor), "");
        assert!(editor.can_redo());
        editor.apply(Action::Redo);
        assert_eq!(plain(&editor), "hi");
    }

    #[test]
    fn undo_restores_a_cut_line() {
        let mut editor = editor("uno\ndos\n");
        editor.apply(Action::CutLine);
        assert_eq!(plain(&editor), "dos\n");
        editor.apply(Action::Undo);
        assert_eq!(plain(&editor), "uno\ndos\n");
    }

    #[test]
    fn undo_after_save_does_not_merge_into_the_previous_group() {
        let mut editor = editor("");
        editor.apply(Action::Insert('a'));
        editor.mark_saved();
        editor.apply(Action::Insert('b'));
        editor.apply(Action::Undo);
        assert_eq!(plain(&editor), "a");
    }

    #[test]
    fn a_bracketed_paste_is_one_undo_step() {
        let mut editor = editor("");
        editor.apply(Action::PasteText("uno\ndos".to_string()));
        assert_eq!(plain(&editor), "uno\ndos");
        editor.apply(Action::Undo);
        assert_eq!(plain(&editor), "");
    }

    #[test]
    fn paste_without_a_register_does_nothing() {
        let mut editor = editor("x");
        editor.apply(Action::Paste);
        assert_eq!(plain(&editor), "x");
        assert!(!editor.is_dirty());
    }
}
