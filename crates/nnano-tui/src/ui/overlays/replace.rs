//! Replace (`^\` or `M-R` outside a prompt), nano's prompt flow.
//!
//! Three stages: the pattern, the replacement, then one confirmation per match
//! (`y` replace, `n` skip, `a` all remaining, `q` stop). The pattern flags are the
//! same modal ones as Find: `M-C` case, `M-R` regexp.
//!
//! The replacement is literal text: backreferences are not part of nano's model
//! and are not invented here.

use nnano_core::editor::Action;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Paragraph;

use super::find::FindSettings;
use super::prompt::{Prompt, draw_prompt_row};

/// Which prompt (or question) currently owns the keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Search,
    Replacement,
    Confirm,
}

/// Session state of one replace run.
#[derive(Debug)]
pub struct Replace {
    stage: Stage,
    search: Prompt,
    replacement: Prompt,
    pub case_sensitive: bool,
    pub regexp: bool,
    /// Char range of the match being confirmed.
    active: Option<(usize, usize)>,
    replaced: usize,
}

impl Replace {
    /// Prefill both prompts from the last search/replace, as nano does.
    pub fn new(find: &FindSettings) -> Self {
        Self {
            stage: Stage::Search,
            search: find.open(),
            replacement: Prompt::with_text(&find.replace_with),
            case_sensitive: find.case_sensitive,
            regexp: find.regexp,
            active: None,
            replaced: 0,
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn set_stage(&mut self, stage: Stage) {
        self.stage = stage;
    }

    pub fn search_text(&self) -> &str {
        self.search.text()
    }

    pub fn replacement_text(&self) -> &str {
        self.replacement.text()
    }

    pub fn active(&self) -> Option<(usize, usize)> {
        self.active
    }

    pub fn set_active(&mut self, range: Option<(usize, usize)>) {
        self.active = range;
    }

    pub fn replaced(&self) -> usize {
        self.replaced
    }

    pub fn note_replaced(&mut self) {
        self.replaced += 1;
    }

    /// Text keys edit the prompt of the current stage; the confirm stage has none.
    pub fn edits(&mut self, action: &Action) -> bool {
        match self.stage {
            Stage::Search => self.search.apply(action.clone()),
            Stage::Replacement => self.replacement.apply(action.clone()),
            Stage::Confirm => false,
        }
    }

    /// Active toggles, shown right-aligned on the prompt row.
    pub fn flags(&self) -> String {
        let mut flags = Vec::new();
        if self.case_sensitive {
            flags.push("[Case]");
        }
        if self.regexp {
            flags.push("[Regexp]");
        }
        flags.join(" ")
    }
}

/// Draw the stage on the status row: a prompt, or the per-match question.
pub fn draw_status(frame: &mut Frame, area: Rect, replace: &Replace) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    match replace.stage {
        Stage::Search => {
            draw_prompt_row(frame, area, " Search: ", &replace.search, &replace.flags())
        }
        Stage::Replacement => draw_prompt_row(
            frame,
            area,
            " Replace with: ",
            &replace.replacement,
            &replace.flags(),
        ),
        Stage::Confirm => {
            let text = format!(
                " Replace this?  y yes  n no  a all  q quit   ({} done)",
                replace.replaced()
            );
            frame.render_widget(
                Paragraph::new(text).style(Style::default().add_modifier(Modifier::REVERSED)),
                area,
            );
        }
    }
}
