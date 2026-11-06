//! The find prompt, nano's model.
//!
//! Not incremental: the search runs when the prompt is accepted. The flags are
//! modal, as in nano — `M-C` case, `M-R` regexp, `M-B` backwards are prompt keys,
//! while outside a prompt `M-R` will open Replace (phase 7).
//!
//! The prompt takes over the status row instead of opening a box, so the text
//! area never shrinks and the cursor stays where nano puts it.

use nnano_core::config::Config;
use nnano_core::config::Opt;
use nnano_core::search::Direction;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Paragraph;

use super::prompt::{Prompt, prompt_width};
use crate::ui::compose_line;

/// Session state of the last search: nano reopens the prompt with the previous
/// pattern and keeps the toggles.
#[derive(Debug, Clone, Default)]
pub struct FindSettings {
    pub query: String,
    pub case_sensitive: bool,
    pub regexp: bool,
    pub backwards: bool,
}

impl FindSettings {
    /// The `case_sensitive` option supplies the default; the prompt can still
    /// flip it for this session without writing config.
    pub fn from_config(config: &Config) -> Self {
        Self {
            case_sensitive: config.bool(Opt::CaseSensitive),
            ..Self::default()
        }
    }

    pub fn direction(&self) -> Direction {
        if self.backwards {
            Direction::Backward
        } else {
            Direction::Forward
        }
    }

    /// Reopen with the previous pattern, cursor at the end.
    pub fn open(&self) -> Prompt {
        Prompt::with_text(&self.query)
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
        if self.backwards {
            flags.push("[Backwards]");
        }
        flags.join(" ")
    }
}

/// Draw the prompt on the status row and put the terminal cursor in it.
pub fn draw_prompt(frame: &mut Frame, area: Rect, settings: &FindSettings, prompt: &Prompt) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default().add_modifier(Modifier::REVERSED);
    let left = format!(" Find: {}", prompt.text());
    let line = compose_line(&left, &settings.flags(), area.width);
    frame.render_widget(Paragraph::new(line).style(style), area);

    let offset = prompt_width(" Find: ") + prompt_width(prompt.before());
    let limit = usize::from(area.width).saturating_sub(1);
    let x = area.x + u16::try_from(offset.min(limit)).unwrap_or(0);
    frame.set_cursor_position((x, area.y));
}
