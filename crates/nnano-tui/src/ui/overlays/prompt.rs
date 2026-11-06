//! Single-line prompts: the find pattern and the palette filter.
//!
//! The cursor is a char offset, like the editor's, so CJK input behaves the same
//! in a prompt as in the buffer. Copy/paste across prompts is not a thing yet: the
//! register lands with `^K`/`^U` in phase 7.

use nnano_core::editor::Action;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Paragraph;

use crate::ui::compose_line;

/// Text plus a char-offset cursor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prompt {
    text: String,
    cursor: usize,
}

impl Prompt {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start with `text` and the cursor at the end, as nano reopens the last
    /// search pattern.
    pub fn with_text(text: &str) -> Self {
        Self {
            text: text.to_string(),
            cursor: text.chars().count(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Chars before the cursor; a renderer places the terminal cursor after them.
    pub fn before(&self) -> &str {
        &self.text[..self.byte_offset()]
    }

    pub fn after(&self) -> &str {
        &self.text[self.byte_offset()..]
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn byte_offset(&self) -> usize {
        self.text
            .char_indices()
            .nth(self.cursor)
            .map_or(self.text.len(), |(offset, _)| offset)
    }

    fn insert(&mut self, ch: char) {
        let at = self.byte_offset();
        self.text.insert(at, ch);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = self.byte_offset();
        let start = self
            .text
            .char_indices()
            .nth(self.cursor - 1)
            .map_or(0, |(offset, _)| offset);
        self.text.replace_range(start..end, "");
        self.cursor -= 1;
    }

    fn delete(&mut self) {
        if self.cursor >= self.text.chars().count() {
            return;
        }
        let start = self.byte_offset();
        let end = self
            .text
            .char_indices()
            .nth(self.cursor + 1)
            .map_or(self.text.len(), |(offset, _)| offset);
        self.text.replace_range(start..end, "");
    }

    /// Text-editing keys shared by every prompt. Returns `false` for anything
    /// else (accept, cancel, toggles), which the overlay handles itself.
    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Insert(ch) => self.insert(ch),
            Action::Backspace => self.backspace(),
            Action::Delete => self.delete(),
            Action::MoveLeft => self.cursor = self.cursor.saturating_sub(1),
            Action::MoveRight => {
                self.cursor = (self.cursor + 1).min(self.text.chars().count());
            }
            Action::MoveLineStart => self.cursor = 0,
            Action::MoveLineEnd => self.cursor = self.text.chars().count(),
            _ => return false,
        }
        true
    }
}

/// Width of a prompt string in cells; prompts are one line, so tabs do not occur
/// but wide glyphs and combining marks do.
pub fn prompt_width(text: &str) -> usize {
    nnano_core::buffer::line_visual_column(text.chars(), 1)
}

/// One reversed row: `label` + text on the left, flags on the right, cursor in
/// the text. Find and both replace prompts share it.
pub fn draw_prompt_row(frame: &mut Frame, area: Rect, label: &str, prompt: &Prompt, flags: &str) {
    let style = Style::default().add_modifier(Modifier::REVERSED);
    let left = format!("{label}{}", prompt.text());
    let line = compose_line(&left, flags, area.width);
    frame.render_widget(Paragraph::new(line).style(style), area);

    let offset = prompt_width(label) + prompt_width(prompt.before());
    let limit = usize::from(area.width).saturating_sub(1);
    let x = area.x + u16::try_from(offset.min(limit)).unwrap_or(0);
    frame.set_cursor_position((x, area.y));
}

#[cfg(test)]
mod tests {
    use super::{Prompt, prompt_width};
    use nnano_core::editor::Action;

    #[test]
    fn typing_appends_and_moves_the_cursor() {
        let mut prompt = Prompt::new();
        for ch in "abc".chars() {
            assert!(prompt.apply(Action::Insert(ch)));
        }
        assert_eq!(prompt.text(), "abc");
        assert_eq!(prompt.before(), "abc");
        assert_eq!(prompt.after(), "");
    }

    #[test]
    fn the_cursor_can_sit_inside_and_edit_there() {
        let mut prompt = Prompt::with_text("ac");
        prompt.apply(Action::MoveLeft);
        prompt.apply(Action::Insert('b'));
        assert_eq!(prompt.text(), "abc");
        assert_eq!(prompt.before(), "ab");
        prompt.apply(Action::Delete);
        assert_eq!(prompt.text(), "ab");
    }

    #[test]
    fn backspace_at_the_start_does_nothing() {
        let mut prompt = Prompt::with_text("x");
        prompt.apply(Action::MoveLineStart);
        prompt.apply(Action::Backspace);
        assert_eq!(prompt.text(), "x");
    }

    #[test]
    fn delete_at_the_end_does_nothing() {
        let mut prompt = Prompt::with_text("x");
        prompt.apply(Action::Delete);
        assert_eq!(prompt.text(), "x");
    }

    #[test]
    fn home_and_end_move_without_editing() {
        let mut prompt = Prompt::with_text("hola");
        prompt.apply(Action::MoveLineStart);
        prompt.apply(Action::MoveLeft);
        assert_eq!(prompt.before(), "");
        prompt.apply(Action::MoveLineEnd);
        prompt.apply(Action::MoveRight);
        assert_eq!(prompt.before(), "hola");
    }

    #[test]
    fn non_editing_actions_are_left_to_the_overlay() {
        let mut prompt = Prompt::new();
        for action in [Action::Newline, Action::Cancel, Action::ToggleCase] {
            assert!(!prompt.apply(action.clone()), "{action:?}");
        }
    }

    #[test]
    fn cjk_edits_on_char_boundaries() {
        let mut prompt = Prompt::with_text("漢字");
        prompt.apply(Action::MoveLeft);
        prompt.apply(Action::Insert('ñ'));
        assert_eq!(prompt.text(), "漢ñ字");
        assert_eq!(prompt.text().chars().count(), 3);
    }

    #[test]
    fn width_is_measured_in_cells() {
        assert_eq!(prompt_width("ab"), 2);
        assert_eq!(prompt_width("漢"), 2);
        assert_eq!(prompt_width("e\u{301}"), 1);
    }
}
