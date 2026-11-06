//! Grouped undo/redo as an operation log — never a snapshot of the text.
//!
//! One [`Edit`] is "replace the char range starting at `start`, which held
//! `removed`, with `inserted`". Undo applies the inverse in reverse order; redo
//! replays the group. A group closes on:
//!
//! - a 400 ms pause between edits (v1 constant, decided 2026-11-04);
//! - a word boundary while typing (a run of words is one group each);
//! - a hard boundary: cut, paste, save, buffer switch.
//!
//! Memory is bounded by the number of groups (the text of each edit, not of the
//! buffer) and the stack is capped at [`LIMIT`] groups.

use std::time::{Duration, Instant};

use super::{Cursor, TextBuffer};

/// Idle time that closes a group.
const PAUSE: Duration = Duration::from_millis(400);
/// Hard cap on remembered groups; the oldest is dropped first.
const LIMIT: usize = 200;

/// One replacement at a char index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub removed: String,
    pub inserted: String,
}

impl Edit {
    pub fn insert(start: usize, text: impl Into<String>) -> Self {
        Self {
            start,
            removed: String::new(),
            inserted: text.into(),
        }
    }

    pub fn remove(start: usize, text: impl Into<String>) -> Self {
        Self {
            start,
            removed: text.into(),
            inserted: String::new(),
        }
    }

    pub fn replace(start: usize, removed: impl Into<String>, inserted: impl Into<String>) -> Self {
        Self {
            start,
            removed: removed.into(),
            inserted: inserted.into(),
        }
    }

    fn apply(&self, buffer: &mut TextBuffer) {
        let removed = self.removed.chars().count();
        buffer.remove(self.start..self.start + removed);
        buffer.insert_str(self.start, &self.inserted);
    }

    fn revert(&self, buffer: &mut TextBuffer) {
        let inserted = self.inserted.chars().count();
        buffer.remove(self.start..self.start + inserted);
        buffer.insert_str(self.start, &self.removed);
    }
}

/// A contiguous run of edits undone together.
#[derive(Debug, Clone)]
struct Group {
    edits: Vec<Edit>,
    cursor_before: Cursor,
    cursor_after: Cursor,
}

/// Two stacks plus the open group.
#[derive(Debug)]
pub struct History {
    done: Vec<Group>,
    undone: Vec<Group>,
    open: Option<Group>,
    opened_at: Option<Instant>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            open: None,
            opened_at: None,
            limit: LIMIT,
        }
    }
}

impl History {
    /// Record one edit, merging it into the open group when it continues it.
    pub fn record(&mut self, edit: Edit, before: Cursor, after: Cursor, now: Instant) {
        let continues = self.open.is_some()
            && self
                .opened_at
                .is_some_and(|at| now.saturating_duration_since(at) <= PAUSE)
            && self
                .open
                .as_ref()
                .is_some_and(|group| mergeable(group, &edit));

        if continues {
            let group = self.open.as_mut().expect("checked above");
            merge(group, edit);
            group.cursor_after = after;
        } else {
            self.close();
            self.open = Some(Group {
                edits: vec![edit],
                cursor_before: before,
                cursor_after: after,
            });
        }
        self.opened_at = Some(now);
        self.undone.clear();
    }

    /// Record several edits that must undo together (a block cut, a replace).
    pub fn record_group(&mut self, edits: Vec<Edit>, before: Cursor, after: Cursor) {
        self.close();
        if !edits.is_empty() {
            self.done.push(Group {
                edits,
                cursor_before: before,
                cursor_after: after,
            });
            self.trim();
        }
        self.opened_at = None;
        self.undone.clear();
    }

    /// Close the open group: cut, paste, save, buffer switch, pointer click.
    pub fn break_group(&mut self) {
        self.close();
        self.opened_at = None;
    }

    /// Undo the newest group; returns the cursor to restore.
    pub fn undo(&mut self, buffer: &mut TextBuffer) -> Option<Cursor> {
        self.close();
        let group = self.done.pop()?;
        for edit in group.edits.iter().rev() {
            edit.revert(buffer);
        }
        let cursor = group.cursor_before;
        self.undone.push(group);
        Some(cursor)
    }

    /// Redo the newest undone group; returns the cursor to restore.
    pub fn redo(&mut self, buffer: &mut TextBuffer) -> Option<Cursor> {
        let group = self.undone.pop()?;
        for edit in &group.edits {
            edit.apply(buffer);
        }
        let cursor = group.cursor_after;
        self.done.push(group);
        self.opened_at = None;
        Some(cursor)
    }

    pub fn can_undo(&self) -> bool {
        self.open.is_some() || !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    fn close(&mut self) {
        if let Some(group) = self.open.take() {
            self.done.push(group);
            self.trim();
        }
    }

    fn trim(&mut self) {
        if self.done.len() > self.limit {
            self.done.remove(0);
        }
    }
}

/// Can `edit` continue the group's last edit without a visible seam?
fn mergeable(group: &Group, edit: &Edit) -> bool {
    let Some(last) = group.edits.last() else {
        return false;
    };
    // A replacement (cut into paste, replace) is always its own group.
    if !last.removed.is_empty() && !last.inserted.is_empty() {
        return false;
    }
    if !edit.removed.is_empty() && !edit.inserted.is_empty() {
        return false;
    }

    if !last.inserted.is_empty() && edit.removed.is_empty() {
        let last_len = last.inserted.chars().count();
        let contiguous = edit.start == last.start + last_len;
        // Undo works word by word: a word starts after whitespace.
        let word_start = last
            .inserted
            .chars()
            .last()
            .is_some_and(char::is_whitespace)
            && edit
                .inserted
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace());
        return contiguous && !word_start;
    }

    if !last.removed.is_empty() && edit.inserted.is_empty() {
        let backspace = edit.start + edit.removed.chars().count() == last.start;
        let forward = edit.start == last.start;
        return backspace || forward;
    }

    false
}

fn merge(group: &mut Group, edit: Edit) {
    let last = group.edits.last_mut().expect("mergeable checks the edit");
    if !last.inserted.is_empty() {
        last.inserted.push_str(&edit.inserted);
    } else if edit.start < last.start {
        // Backspace: the removal grows to the left.
        last.start = edit.start;
        last.removed = format!("{}{}", edit.removed, last.removed);
    } else {
        last.removed.push_str(&edit.removed);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Edit, History};
    use crate::buffer::{Cursor, TextBuffer};

    fn now() -> Instant {
        Instant::now()
    }

    fn text(buffer: &TextBuffer) -> String {
        (0..buffer.line_count())
            .map(|line| buffer.line_text(line))
            .collect::<Vec<_>>()
            .join("\\n")
    }

    #[test]
    fn typing_a_word_is_one_group() {
        let mut buffer = TextBuffer::new();
        let mut history = History::default();
        let start = now();
        for (index, ch) in "hola".chars().enumerate() {
            let before = Cursor::new(0, index);
            buffer.insert_str(index, &ch.to_string());
            history.record(
                Edit::insert(index, ch.to_string()),
                before,
                Cursor::new(0, index + 1),
                start,
            );
        }
        assert_eq!(text(&buffer), "hola");
        let cursor = history.undo(&mut buffer).unwrap();
        assert_eq!(text(&buffer), "");
        assert_eq!((cursor.line(), cursor.col()), (0, 0));
        assert!(!history.can_undo());
    }

    #[test]
    fn a_pause_closes_the_group() {
        let mut buffer = TextBuffer::new();
        let mut history = History::default();
        let start = now();
        buffer.insert_str(0, "a");
        history.record(
            Edit::insert(0, "a".to_string()),
            Cursor::new(0, 0),
            Cursor::new(0, 1),
            start,
        );
        buffer.insert_str(1, "b");
        history.record(
            Edit::insert(1, "b".to_string()),
            Cursor::new(0, 1),
            Cursor::new(0, 2),
            start + Duration::from_millis(500),
        );
        history.undo(&mut buffer);
        assert_eq!(text(&buffer), "a", "only the late edit is undone");
        history.undo(&mut buffer);
        assert_eq!(text(&buffer), "");
    }

    #[test]
    fn a_word_boundary_closes_the_group() {
        let mut buffer = TextBuffer::new();
        let mut history = History::default();
        let start = now();
        for (index, ch) in "ab cd".chars().enumerate() {
            let before = Cursor::new(0, index);
            buffer.insert_str(index, &ch.to_string());
            history.record(
                Edit::insert(index, ch.to_string()),
                before,
                Cursor::new(0, index + 1),
                start,
            );
        }
        history.undo(&mut buffer);
        assert_eq!(text(&buffer), "ab ");
        history.undo(&mut buffer);
        assert_eq!(text(&buffer), "");
    }

    #[test]
    fn backspaces_merge_and_reinsert_in_order() {
        let mut buffer = TextBuffer::from_text("hola");
        let mut history = History::default();
        let start = now();
        for index in (0..4).rev() {
            let before = Cursor::new(0, index + 1);
            buffer.remove(index..index + 1);
            let removed = "hola"[index..index + 1].to_string();
            history.record(
                Edit::remove(index, removed),
                before,
                Cursor::new(0, index),
                start,
            );
        }
        assert_eq!(text(&buffer), "");
        history.undo(&mut buffer);
        assert_eq!(text(&buffer), "hola");
    }

    #[test]
    fn delete_forward_edits_merge_at_the_same_start() {
        let mut buffer = TextBuffer::from_text("abc");
        let mut history = History::default();
        let start = now();
        buffer.remove(0..1);
        history.record(
            Edit::remove(0, "a"),
            Cursor::new(0, 0),
            Cursor::new(0, 0),
            start,
        );
        buffer.remove(0..1);
        history.record(
            Edit::remove(0, "b"),
            Cursor::new(0, 0),
            Cursor::new(0, 0),
            start,
        );
        assert_eq!(text(&buffer), "c");
        history.undo(&mut buffer);
        assert_eq!(text(&buffer), "abc");
    }

    #[test]
    fn redo_replays_the_group() {
        let mut buffer = TextBuffer::new();
        let mut history = History::default();
        let start = now();
        buffer.insert_str(0, "ab");
        history.record(
            Edit::insert(0, "ab"),
            Cursor::new(0, 0),
            Cursor::new(0, 2),
            start,
        );
        history.undo(&mut buffer);
        assert!(history.can_redo());
        let cursor = history.redo(&mut buffer).unwrap();
        assert_eq!(text(&buffer), "ab");
        assert_eq!((cursor.line(), cursor.col()), (0, 2));
        assert!(!history.can_redo());
    }

    #[test]
    fn a_new_edit_drops_the_redo_stack() {
        let mut buffer = TextBuffer::from_text("a");
        let mut history = History::default();
        let start = now();
        buffer.insert_str(1, "b");
        history.record(
            Edit::insert(1, "b"),
            Cursor::new(0, 1),
            Cursor::new(0, 2),
            start,
        );
        history.undo(&mut buffer);
        assert!(history.can_redo());
        buffer.insert_str(0, "z");
        history.record(
            Edit::insert(0, "z"),
            Cursor::new(0, 0),
            Cursor::new(0, 1),
            start,
        );
        assert!(!history.can_redo());
    }

    #[test]
    fn a_recorded_group_undoes_as_one_step() {
        let mut buffer = TextBuffer::from_text("uno\ndos");
        let mut history = History::default();
        buffer.remove(4..7);
        buffer.remove(0..3);
        history.record_group(
            vec![Edit::remove(4, "dos"), Edit::remove(0, "uno")],
            Cursor::new(0, 0),
            Cursor::new(1, 0),
        );
        assert_eq!(text(&buffer), "\\n");
        history.undo(&mut buffer);
        assert_eq!(text(&buffer), "uno\\ndos");
    }

    #[test]
    fn the_stack_is_capped() {
        let mut buffer = TextBuffer::new();
        let mut history = History {
            limit: 2,
            ..History::default()
        };
        for index in 0..5 {
            buffer.insert_str(index, "x");
            history.record(
                Edit::insert(index, "x"),
                Cursor::new(0, index),
                Cursor::new(0, index + 1),
                now(),
            );
            history.break_group();
        }
        assert_eq!(history.done.len(), 2);
    }
}
