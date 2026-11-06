//! Overlays: the find prompt, the settings mirror, the command palette and help.
//!
//! Only one overlay is open at a time, and keys are routed through the same
//! [`Action`](nnano_core::editor::Action) enum as the editor: a prompt consumes
//! the text-editing actions and lets the app loop handle the rest, so there is no
//! second key path to keep in sync.
//!
//! The find prompt takes the status row (nano's bottom line); the other three are
//! floating panels over the text area.

pub mod find;
pub mod help;
pub mod palette;
pub mod prompt;
pub mod replace;
pub mod settings;

pub use find::FindSettings;
pub use palette::Palette;
pub use prompt::Prompt;
pub use replace::Replace;
pub use settings::Settings;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Clear};

use crate::App;

/// The overlay that currently owns the keys.
#[derive(Debug)]
pub enum Overlay {
    /// nano's prompt row; the pattern lives in the prompt, the flags in
    /// [`FindSettings`].
    Find(Prompt),
    Settings(Settings),
    Help,
    Palette(Palette),
    /// Replace prompts and the per-match question; drawn on the status row.
    Replace(Replace),
}

impl Overlay {
    /// True for overlays that take over the status row instead of floating.
    pub fn is_prompt(&self) -> bool {
        matches!(self, Overlay::Find(_) | Overlay::Replace(_))
    }

    /// Draw the floating panels. The find prompt is drawn by the status row and
    /// is a no-op here.
    pub fn draw(&self, frame: &mut Frame, area: Rect, app: &App) {
        match self {
            Overlay::Find(_) | Overlay::Replace(_) => {}
            Overlay::Settings(settings) => settings::draw(frame, area, app, settings),
            Overlay::Help => help::draw(frame, area),
            Overlay::Palette(palette) => {
                palette::draw(frame, area, palette, &app.buffer_names());
            }
        }
    }
}

/// Centered rectangle, clamped to the available area.
pub(crate) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// Inner area of a panel without drawing anything: the mouse hit test needs the
/// same geometry the renderer used.
pub(crate) fn panel_inner(area: Rect, width: u16, height: u16) -> Rect {
    Block::bordered().inner(centered(area, width, height))
}

/// Clear a centered rectangle, draw a bordered title around it, and return the
/// inner area a panel can render into.
pub(crate) fn popup_frame(
    frame: &mut Frame,
    area: Rect,
    width: u16,
    height: u16,
    title: &str,
) -> Rect {
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::bordered().title(format!(" {title} "));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    inner
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::centered;

    #[test]
    fn centers_and_clamps() {
        let area = Rect::new(0, 0, 40, 10);
        assert_eq!(centered(area, 20, 4), Rect::new(10, 3, 20, 4));
        assert_eq!(centered(area, 80, 40), area, "never larger than the area");
        assert_eq!(centered(area, 0, 0), Rect::new(20, 5, 0, 0));
    }
}
