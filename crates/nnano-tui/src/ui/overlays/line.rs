//! One-line prompts that are not a search: save as, open, goto and execute.
//!
//! They take over the status row exactly like the find prompt, so the text area
//! never reflows and the cursor stays where it was. `Tab` completes a path for the
//! two prompts that name a file; the filesystem call belongs to the app loop, so
//! this module stays pure and testable.

use nnano_core::editor::Action;
use ratatui::Frame;
use ratatui::layout::Rect;

use super::prompt::{Prompt, draw_prompt_row};
use crate::i18n::{Lang, t};

/// What the typed line means when it is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `M-S`, or `^S` on a buffer with no name.
    SaveAs,
    /// `^O`: open a path in a new buffer.
    Open,
    /// `M-I`: insert a file's text at the cursor.
    InsertFile,
    /// `^G` / `M-G`: jump to a line (and optional column) in this buffer.
    Goto,
    /// `M-X`: run a shell command and insert its output.
    Execute,
}

impl Kind {
    /// Left label of the prompt row.
    pub fn label(self, lang: Lang) -> &'static str {
        match self {
            Kind::SaveAs => t(lang, " Save as: "),
            Kind::Open => t(lang, " Open: "),
            Kind::InsertFile => t(lang, " Insert file: "),
            Kind::Goto => t(lang, " Go to line: "),
            Kind::Execute => t(lang, " Execute: "),
        }
    }

    /// Right side of the prompt row: what the accepted line should look like.
    pub fn hint(self, lang: Lang) -> &'static str {
        match self {
            Kind::SaveAs => t(lang, "[Enter saves]"),
            Kind::Open => t(lang, "[Tab completes]"),
            Kind::InsertFile => t(lang, "[Tab completes]"),
            Kind::Goto => t(lang, "[line[,column] or +N/-N]"),
            Kind::Execute => t(lang, "[sh -c; output is inserted]"),
        }
    }

    /// Prompts that name a path and therefore accept Tab completion.
    pub fn completes(self) -> bool {
        matches!(self, Kind::SaveAs | Kind::Open | Kind::InsertFile)
    }
}

/// A prompt plus the meaning of its text.
#[derive(Debug)]
pub struct Line {
    pub kind: Kind,
    pub prompt: Prompt,
}

impl Line {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            prompt: Prompt::new(),
        }
    }

    pub fn text(&self) -> &str {
        self.prompt.text()
    }

    /// Text keys edit the line; accept, cancel and Tab belong to the app loop.
    pub fn edit(&mut self, action: Action) -> bool {
        self.prompt.apply(action)
    }

    /// Replace the text and put the cursor at its end, as a completion does.
    pub fn set_text(&mut self, text: &str) {
        self.prompt = Prompt::with_text(text);
    }
}

/// Draw the prompt on the status row and put the terminal cursor in it.
pub fn draw_status(frame: &mut Frame, area: Rect, line: &Line, lang: Lang) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    draw_prompt_row(
        frame,
        area,
        line.kind.label(lang),
        &line.prompt,
        line.kind.hint(lang),
    );
}

#[cfg(test)]
mod tests {
    use nnano_core::editor::Action;

    use super::{Kind, Line};

    #[test]
    fn every_kind_has_a_label_and_a_hint() {
        for kind in [
            Kind::SaveAs,
            Kind::Open,
            Kind::InsertFile,
            Kind::Goto,
            Kind::Execute,
        ] {
            for lang in crate::i18n::Lang::all() {
                assert!(!kind.label(lang).is_empty(), "{kind:?}");
                assert!(
                    kind.label(lang).ends_with(' '),
                    "the {lang:?} label ends with a space: {:?}",
                    kind.label(lang)
                );
                assert!(kind.hint(lang).starts_with('['), "{kind:?}/{lang:?}");
            }
        }
    }

    #[test]
    fn only_the_path_prompts_complete() {
        assert!(Kind::SaveAs.completes());
        assert!(Kind::Open.completes());
        assert!(Kind::InsertFile.completes());
        assert!(!Kind::Goto.completes());
        assert!(!Kind::Execute.completes());
    }

    #[test]
    fn text_keys_edit_the_line_and_others_are_left_alone() {
        let mut line = Line::new(Kind::SaveAs);
        for ch in "nota.txt".chars() {
            assert!(line.edit(Action::Insert(ch)));
        }
        assert_eq!(line.text(), "nota.txt");
        assert!(!line.edit(Action::Newline));
        assert!(!line.edit(Action::Cancel));
        assert!(!line.edit(Action::InsertTab), "Tab is the completion key");
        line.set_text("/tmp/completado");
        assert_eq!(line.text(), "/tmp/completado");
    }
}
