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
    char_offset_for_column, line_visual_column,
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
    /// `M-S`, or `^S` on a buffer with no name: ask for a path, then write.
    SaveAs,
    /// `^O`: ask for a path and open it in a new buffer.
    OpenPath,
    /// `^R`: re-read this buffer's file and replace its text.
    Reload,
    /// `M-I`: ask for a path and insert its text at the cursor.
    InsertFile,
    /// `^G` / `M-G`: ask for a line (and optional column) in this buffer.
    Goto,
    /// `M-X`: ask for a shell command, run it and insert its output.
    Execute,
    /// `^J`: re-wrap the paragraph, or the marked lines. The width is the
    /// viewport's, so the app loop resolves it and calls [`Editor::justify`].
    Justify,
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
    /// `^N`/`^T`: open a new empty buffer and make it active.
    NewBuffer,
    /// `^T`: same as `NewBuffer`; tabs are buffers in v1.
    NewTab,
    /// `^W`: close the active buffer (asks once when dirty).
    CloseBuffer,
    /// Switch to another open buffer (palette row, buffer-bar click).
    SwitchBuffer(usize),
    /// `M-R` / `^\\`: open the replace prompts.
    OpenReplace,
    /// `M-A`: start or clear the mark.
    StartMark,
    /// `^A`: select the whole buffer.
    SelectAll,
    /// `M-Z`: make the mark rectangular (or the next one, if there is none).
    ToggleBlockSelection,
    /// `M-Y`: turn syntax highlighting off or on for this session.
    ToggleSyntax,
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
    /// `Ctrl+←`: to the start of the previous word.
    MoveWordLeft,
    /// `Ctrl+→`: to the start of the next word.
    MoveWordRight,
    MoveUp,
    MoveDown,
    /// `Shift+↑`: extend the mark one line up, starting one if there is none.
    SelectUp,
    /// `Shift+↓`: extend the mark one line down, starting one if there is none.
    SelectDown,
    /// `Shift+←`: extend the mark one character left, starting one if there is none.
    SelectLeft,
    /// `Shift+→`: extend the mark one character right, starting one if there is none.
    SelectRight,
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

    /// Mark the buffer unsaved without editing it. Only the recovery restore uses
    /// this: the text came from an earlier session and is not on disk yet.
    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
        if dirty {
            self.history.break_group();
        }
    }

    /// Replace the whole text with what the disk holds now (`^R`). The old undo
    /// history refers to ranges of text that no longer exists, so it is dropped:
    /// a reload is a hard boundary, and the buffer is clean when it returns.
    pub fn load(&mut self, text: &str) {
        self.buffer = TextBuffer::from_text(text);
        self.cursor.clamp(&self.buffer);
        self.selection = None;
        self.goal_col = None;
        self.history.clear();
        self.dirty = false;
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
            | Action::SaveAs
            | Action::OpenPath
            // Reload and insert-file read the disk, so the app loop owns them;
            // `Editor::load` and `PasteText` are the halves it calls.
            | Action::Reload
            | Action::InsertFile
            | Action::Goto
            | Action::Execute
            | Action::Justify
            | Action::Redraw
            | Action::OpenFind
            | Action::OpenPalette
            | Action::OpenSettings
            | Action::OpenHelp
            | Action::OpenReplace
            | Action::Cancel
            | Action::ToggleCase
            | Action::ToggleRegexp
            | Action::ToggleBackwards
            | Action::FindNext
            | Action::FindPrevious
            | Action::NewBuffer
            | Action::NewTab
            | Action::CloseBuffer
            | Action::SwitchBuffer(_)
            | Action::ToggleSyntax
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
            Action::MoveWordLeft => {
                self.move_word_left();
                self.follow();
            }
            Action::MoveWordRight => {
                self.move_word_right();
                self.follow();
            }
            Action::MoveUp => self.move_vertical(-1),
            Action::MoveDown => self.move_vertical(1),
            Action::SelectUp => self.extend_vertical(-1),
            Action::SelectDown => self.extend_vertical(1),
            Action::SelectLeft => self.extend_horizontal(-1),
            Action::SelectRight => self.extend_horizontal(1),
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

    /// `Shift+↑`/`Shift+↓`: a mark starts at the cursor when there is none, then
    /// the cursor moves and [`Editor::follow`] drags the head with it, so the
    /// marked rows grow under the cursor. The visual column is kept, as in a
    /// plain vertical move.
    fn extend_vertical(&mut self, delta: isize) {
        if self.selection.is_none() {
            let mode = self.mark_mode();
            self.selection = Some(Selection::new(self.cursor, self.cursor, mode));
        }
        self.move_vertical(delta);
    }

    /// `Shift+←`/`Shift+→`: the horizontal twin of [`Editor::extend_vertical`].
    /// One character at a time, crossing the line break like a plain arrow, and
    /// the goal column is forgotten — a desktop editor behaves the same way.
    fn extend_horizontal(&mut self, delta: isize) {
        if self.selection.is_none() {
            let mode = self.mark_mode();
            self.selection = Some(Selection::new(self.cursor, self.cursor, mode));
        }
        if delta < 0 {
            self.move_left();
        } else {
            self.move_right();
        }
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

    /// Replace a char range in one undo group; used by the replace overlay.
    /// Returns whether anything changed.
    pub fn replace_range(&mut self, start: usize, end: usize, text: &str) -> bool {
        let start = start.min(self.buffer.len_chars());
        let end = end.clamp(start, self.buffer.len_chars());
        let removed: String = (start..end)
            .filter_map(|index| self.buffer.char_at(index))
            .collect();
        if removed.is_empty() && text.is_empty() {
            return false;
        }
        let before = self.cursor;
        if !removed.is_empty() {
            self.buffer.remove(start..end);
        }
        self.buffer.insert_str(start, text);
        let (line, col) = self.buffer.char_to_position(start + text.chars().count());
        self.cursor = Cursor::new(line, col);
        self.selection = None;
        self.touch();
        self.history.record_group(
            vec![Edit::replace(start, removed, text)],
            before,
            self.cursor,
        );
        true
    }

    /// Merge the newest `count` undo groups (replace-all is one step).
    pub fn merge_undo_groups(&mut self, count: usize) {
        self.history.merge_last(count);
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
                self.register = selection.text(&self.buffer);
                // The copy is done, so the mark ends here: without this, the next
                // arrow key would extend it (nano's mark semantics) and `^V`
                // would replace everything the cursor walked over — the copy
                // looked like it had failed. Same as leaving visual mode in vim.
                self.selection = None;
            }
            _ => self.register = self.line_with_terminator(self.cursor.line()),
        }
    }

    /// Mouse release: copy the marked region without ending the mark, so the
    /// highlight stays visible after the copy. Returns how many chars were taken,
    /// or `None` when there is no mark (a plain click must not touch the register).
    pub fn copy_marked(&mut self) -> Option<usize> {
        let selection = self.selection.filter(|selection| !selection.is_empty())?;
        let text = selection.text(&self.buffer);
        if text.is_empty() {
            return None;
        }
        let chars = text.chars().count();
        self.register = text;
        Some(chars)
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

    /// `Ctrl+←`: back over the gap, then to the first char of the word. Crossing
    /// a line break is a gap, as in nano.
    fn move_word_left(&mut self) {
        let mut index = self.cursor.char_index(&self.buffer);
        while index > 0 && !self.buffer.char_at(index - 1).is_some_and(is_word) {
            index -= 1;
        }
        while index > 0 && self.buffer.char_at(index - 1).is_some_and(is_word) {
            index -= 1;
        }
        self.cursor = self.position(index);
        self.goal_col = None;
    }

    /// `Ctrl+→`: out of the current word, then over the gap to the next word. A
    /// cursor already in a gap skips it and lands on the word after it.
    fn move_word_right(&mut self) {
        let total = self.buffer.len_chars();
        let mut index = self.cursor.char_index(&self.buffer);
        while index < total && self.buffer.char_at(index).is_some_and(is_word) {
            index += 1;
        }
        while index < total && !self.buffer.char_at(index).is_some_and(is_word) {
            index += 1;
        }
        self.cursor = self.position(index);
        self.goal_col = None;
    }

    /// `^J`: join the paragraph around the cursor (or the marked lines) and wrap it
    /// to `width` cells, keeping the first line's indent. One undo group, and the
    /// cursor lands at the start of the paragraph.
    ///
    /// Words are split on whitespace and rejoined with single spaces, so a wrapped
    /// paragraph never keeps the ragged joins it came with. Line breaks inside a
    /// paragraph are not data: that is what "justify" means here, as in nano.
    pub fn justify(&mut self, width: usize) {
        let (first, last) = self.paragraph_lines();
        let indent: String = self
            .buffer
            .line_chars(first)
            .take_while(|ch| ch.is_whitespace() && *ch != '\n' && *ch != '\r')
            .collect();
        let mut words: Vec<String> = Vec::new();
        for line in first..=last {
            words.extend(
                self.buffer
                    .line_chars(line)
                    .collect::<String>()
                    .split_whitespace()
                    .map(str::to_string),
            );
        }
        if words.is_empty() {
            return;
        }

        let start = self.buffer.line_start(first);
        let end = self.buffer.line_end(last);
        let removed: String = (start..end)
            .filter_map(|index| self.buffer.char_at(index))
            .collect();
        let wrapped = wrap(&indent, &words, width, self.tab_width);
        if wrapped == removed {
            return;
        }

        let before = self.cursor;
        self.buffer.remove(start..end);
        self.buffer.insert_str(start, &wrapped);
        self.cursor = Cursor::new(first.min(self.buffer.line_count() - 1), 0);
        self.cursor.clamp(&self.buffer);
        self.selection = None;
        self.touch();
        self.history.record_group(
            vec![Edit::replace(start, removed, &wrapped)],
            before,
            self.cursor,
        );
    }

    /// Lines `justify` owns: the marked ones when there is a mark, otherwise the
    /// contiguous non-blank block around the cursor.
    fn paragraph_lines(&self) -> (usize, usize) {
        if let Some(selection) = self.selection.filter(|selection| !selection.is_empty()) {
            let (start, end) = selection.char_range(&self.buffer);
            let (first, _) = self.buffer.char_to_position(start);
            let (last, col) = self.buffer.char_to_position(end);
            // A mark that ends at column 0 does not own that line.
            let last = if col == 0 && last > first {
                last - 1
            } else {
                last
            };
            return (first, last);
        }
        let cursor = self.cursor.line();
        let blank = |line: usize| self.buffer.line_len(line) == 0;
        let mut first = cursor;
        while first > 0 && !blank(first - 1) {
            first -= 1;
        }
        let mut last = cursor;
        while last + 1 < self.buffer.line_count() && !blank(last + 1) {
            last += 1;
        }
        (first, last)
    }

    /// Any change to the text marks the buffer dirty and forgets the goal column.
    fn touch(&mut self) {
        self.dirty = true;
        self.goal_col = None;
    }
}

/// nano's word character: alphanumeric or underscore. Unicode letters count, so a
/// word in Spanish or Japanese moves as one.
fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Re-wrap `words` under `indent` to `width` cells, one space between words.
///
/// Width is measured in cells (`line_visual_column`), so CJK does not overflow the
/// viewport, and tab stops use the editor's `tab_size`. A word wider than the
/// limit stays whole instead of being split: no hyphenation, as in nano.
fn wrap(indent: &str, words: &[String], width: usize, tab_width: usize) -> String {
    let indent_cells = line_visual_column(indent.chars(), tab_width);
    let limit = width.max(indent_cells + 1);
    let mut out = String::from(indent);
    let mut column = indent_cells;
    for (index, word) in words.iter().enumerate() {
        let cells = line_visual_column(word.chars(), tab_width);
        if index == 0 {
            out.push_str(word);
            column += cells;
        } else if column + 1 + cells > limit {
            out.push('\n');
            out.push_str(indent);
            out.push_str(word);
            column = indent_cells + cells;
        } else {
            out.push(' ');
            out.push_str(word);
            column += 1 + cells;
        }
    }
    out
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
    fn shift_down_selects_whole_rows_and_shift_up_shrinks_them() {
        let mut editor = editor("uno\ndos\ntres");
        editor.set_cursor(0, 1);
        editor.apply(Action::SelectDown);
        // The anchor stays at 0:1 and the head follows the cursor, as in a drag.
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "no\nd");
        editor.apply(Action::SelectDown);
        assert_eq!(
            editor.selection().unwrap().text(editor.buffer()),
            "no\ndos\nt"
        );
        editor.apply(Action::SelectUp);
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "no\nd");
        assert_eq!(at(&editor), (1, 1));
    }

    #[test]
    fn shift_up_selects_backwards_from_the_cursor() {
        let mut editor = editor("uno\ndos\ntres");
        editor.set_cursor(2, 2);
        editor.apply(Action::SelectUp);
        assert_eq!(at(&editor), (1, 2));
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "s\ntr");
    }

    #[test]
    fn shift_vertical_keeps_the_wanted_column_across_a_short_line() {
        let mut editor = editor("0123456789\nxyz\n0123456789");
        editor.set_cursor(0, 8);
        editor.apply(Action::SelectDown);
        assert_eq!(at(&editor), (1, 3));
        editor.apply(Action::SelectDown);
        assert_eq!(
            at(&editor),
            (2, 8),
            "the goal column survives the short row"
        );
        assert_eq!(
            editor.selection().unwrap().text(editor.buffer()),
            "89\nxyz\n01234567"
        );
    }

    #[test]
    fn shift_right_selects_characters_and_crosses_the_line_break() {
        let mut editor = editor("uno\ndos");
        editor.set_cursor(0, 1);
        editor.apply(Action::SelectRight);
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "n");
        editor.apply(Action::SelectRight);
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "no");
        // The line break is one character, so the mark reaches the next row.
        editor.apply(Action::SelectRight);
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "no\n");
        assert_eq!(at(&editor), (1, 0));
    }

    #[test]
    fn shift_left_selects_backwards_and_shrinks_with_the_cursor() {
        let mut editor = editor("uno\ndos");
        editor.set_cursor(1, 1);
        editor.apply(Action::SelectLeft);
        assert_eq!(at(&editor), (1, 0));
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "d");
        editor.apply(Action::SelectLeft);
        assert_eq!(at(&editor), (0, 3));
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "\nd");
        editor.apply(Action::SelectLeft);
        assert_eq!(at(&editor), (0, 2));
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "o\nd");
    }

    #[test]
    fn shift_arrows_share_one_mark_and_extend_it_in_both_axes() {
        let mut editor = editor("uno\ndos\ntres");
        editor.set_cursor(1, 1);
        editor.apply(Action::SelectRight);
        editor.apply(Action::SelectDown);
        editor.apply(Action::SelectLeft);
        // The anchor stays at the first Shift press; only the head follows.
        assert_eq!(at(&editor), (2, 1));
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "os\nt");
    }

    #[test]
    fn shift_vertical_extends_a_rectangular_mark() {
        let mut editor = editor("abc\ndef\nghi");
        editor.apply(Action::ToggleBlockSelection);
        editor.set_cursor(0, 0);
        editor.apply(Action::SelectDown);
        editor.apply(Action::SelectDown);
        editor.apply(Action::MoveRight);
        assert_eq!(editor.selection().unwrap().text(editor.buffer()), "a\nd\ng");
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
    fn a_mouse_copy_keeps_the_mark_and_fills_the_register() {
        let mut editor = editor("uno dos");
        select(&mut editor, 0, 0, 3);
        assert_eq!(editor.copy_marked(), Some(3));
        assert_eq!(editor.register(), "uno");
        assert!(editor.has_selection(), "the highlight survives the copy");
    }

    #[test]
    fn copy_ends_the_mark_so_the_next_paste_inserts_at_the_cursor() {
        let mut editor = editor("uno dos");
        select(&mut editor, 0, 0, 3);
        editor.apply(Action::Copy);
        assert_eq!(editor.register(), "uno");
        assert!(!editor.has_selection(), "copy must drop the mark");
        editor.apply(Action::MoveRight);
        editor.apply(Action::Paste);
        assert_eq!(plain(&editor), "uno unodos");
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
    fn replace_range_is_one_undoable_edit() {
        let mut editor = editor("uno dos uno");
        assert!(editor.replace_range(0, 3, "X"));
        assert_eq!(plain(&editor), "X dos uno");
        assert_eq!(at(&editor), (0, 1));
        editor.apply(Action::Undo);
        assert_eq!(plain(&editor), "uno dos uno");
    }

    #[test]
    fn merging_groups_makes_replace_all_one_undo_step() {
        let mut editor = editor("a a a");
        editor.replace_range(0, 1, "b");
        editor.replace_range(2, 3, "b");
        editor.replace_range(4, 5, "b");
        assert_eq!(plain(&editor), "b b b");
        editor.merge_undo_groups(3);
        editor.apply(Action::Undo);
        assert_eq!(plain(&editor), "a a a");
    }

    #[test]
    fn paste_without_a_register_does_nothing() {
        let mut editor = editor("x");
        editor.apply(Action::Paste);
        assert_eq!(plain(&editor), "x");
        assert!(!editor.is_dirty());
    }

    #[test]
    fn word_movement_stops_at_word_starts() {
        let mut editor = editor("uno dos,tres");
        editor.apply(Action::MoveWordRight);
        assert_eq!(at(&editor), (0, 4), "start of the second word");
        editor.apply(Action::MoveWordRight);
        assert_eq!(at(&editor), (0, 8), "past the comma, at the next word");
        editor.apply(Action::MoveWordRight);
        assert_eq!(at(&editor), (0, 12), "end of the buffer");
        editor.apply(Action::MoveWordLeft);
        assert_eq!(at(&editor), (0, 8));
        editor.apply(Action::MoveWordLeft);
        assert_eq!(at(&editor), (0, 4));
        editor.apply(Action::MoveWordLeft);
        assert_eq!(at(&editor), (0, 0));
    }

    #[test]
    fn word_movement_crosses_lines_and_ignores_punctuation() {
        let mut wrapped = editor("uno\n  dos");
        wrapped.set_cursor(0, 1);
        wrapped.apply(Action::MoveWordRight);
        assert_eq!(at(&wrapped), (1, 2), "the line break is a gap");
        wrapped.apply(Action::MoveWordLeft);
        assert_eq!(at(&wrapped), (0, 0));
        // From inside a run of punctuation, right lands on the next word.
        let mut punctuated = editor("a -- b");
        punctuated.set_cursor(0, 3);
        punctuated.apply(Action::MoveWordRight);
        assert_eq!(at(&punctuated), (0, 5));
    }

    #[test]
    fn word_movement_keeps_a_mark_and_handles_an_empty_buffer() {
        let mut empty = editor("");
        empty.apply(Action::MoveWordRight);
        empty.apply(Action::MoveWordLeft);
        assert_eq!(at(&empty), (0, 0));

        let mut marked = editor("uno dos");
        marked.set_cursor(0, 0);
        marked.apply(Action::StartMark);
        marked.apply(Action::MoveWordRight);
        // The cursor lands on the next word's first char, so the mark covers the
        // space it crossed: the range is half-open and honest about it.
        assert_eq!(marked.selection().unwrap().text(marked.buffer()), "uno ");
    }

    #[test]
    fn justify_joins_the_paragraph_around_the_cursor() {
        let mut editor = editor("uno\ndos tres\n\nsiguiente");
        editor.set_cursor(1, 4);
        editor.justify(80);
        assert_eq!(plain(&editor), "uno dos tres\n\nsiguiente");
        assert_eq!(at(&editor), (0, 0));
    }

    #[test]
    fn justify_wraps_at_the_given_width_and_keeps_the_indent() {
        let mut editor = editor("  uno dos tres cuatro\n");
        editor.justify(11);
        assert_eq!(plain(&editor), "  uno dos\n  tres\n  cuatro\n");
        assert_eq!(at(&editor), (0, 0));
    }

    #[test]
    fn justify_wraps_by_cells_not_chars() {
        let mut editor = editor("漢字 漢字 漢字");
        editor.justify(9);
        assert_eq!(plain(&editor), "漢字 漢字\n漢字");
    }

    #[test]
    fn justify_is_one_undo_step_and_keeps_a_blank_line_untouched() {
        let mut joined = editor("uno\ndos");
        joined.justify(80);
        assert_eq!(plain(&joined), "uno dos");
        joined.apply(Action::Undo);
        assert_eq!(plain(&joined), "uno\ndos");

        let mut blank = editor("\n");
        blank.justify(80);
        assert_eq!(plain(&blank), "\n");
        assert!(!blank.is_dirty());
    }

    #[test]
    fn justify_uses_the_marked_lines_when_there_is_a_mark() {
        // The mark is half-open, so ending at column 0 does not own that line.
        let mut one_line = editor("uno\ndos\ntres\n");
        one_line.set_cursor(0, 0);
        one_line.apply(Action::StartMark);
        one_line.apply(Action::MoveDown);
        one_line.justify(80);
        assert_eq!(plain(&one_line), "uno\ndos\ntres\n", "nothing to join");

        // Extending into the second line justifies both.
        let mut two_lines = editor("uno\ndos\ntres\n");
        two_lines.set_cursor(0, 0);
        two_lines.apply(Action::StartMark);
        two_lines.apply(Action::MoveDown);
        two_lines.apply(Action::MoveRight);
        two_lines.justify(80);
        assert_eq!(plain(&two_lines), "uno dos\ntres\n");
    }

    #[test]
    fn set_dirty_marks_a_restored_buffer_without_editing_it() {
        let mut editor = editor("recuperado\n");
        assert!(!editor.is_dirty());
        editor.set_dirty(true);
        assert!(editor.is_dirty());
        editor.set_dirty(false);
        assert!(!editor.is_dirty());
    }

    #[test]
    fn load_replaces_the_text_and_drops_the_history() {
        let mut editor = editor("uno\ndos\n3\n4\n5\n6\n7\n");
        editor.apply(Action::MoveFileEnd);
        editor.apply(Action::Insert('!'));
        assert!(editor.is_dirty());

        editor.load("nuevo\n");

        assert_eq!(plain(&editor), "nuevo\n");
        assert!(!editor.is_dirty());
        // The cursor is clamped into the shorter buffer, not left off the end.
        assert!(editor.cursor().line() < editor.buffer().line_count());
        assert!(editor.cursor().col() <= editor.buffer().line_len(editor.cursor().line()));
        // The edit before the reload cannot be undone into the new text.
        editor.apply(Action::Undo);
        assert_eq!(plain(&editor), "nuevo\n");
    }
}
