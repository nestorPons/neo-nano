//! Terminal layer for `nnano`.
//!
//! Owns everything that assumes a terminal: raw mode and its restore path, the
//! panic hook, capability detection, input decoding, drawing and overlays. All
//! editor semantics stay in `nnano-core`.
//!
//! Event loop (phase 3): `poll(16ms)` -> decode one key into an `Action` -> apply
//! -> redraw only when dirty. No fixed tick, no async runtime, no worker thread.

pub mod term;
pub mod ui;

use std::io;
use std::io::IsTerminal;
use std::time::Duration;

use crossterm::event::{self, Event};
use nnano_core::buffer::DEFAULT_TAB_WIDTH;
use nnano_core::editor::{Action, Editor};

use crate::term::Caps;
use crate::ui::viewport::Viewport;

/// Event-poll granularity. It is not a redraw rate: a frame is drawn only when
/// the app is dirty.
const POLL_INTERVAL: Duration = Duration::from_millis(16);

/// Application state: one buffer, one cursor, one viewport (no split view).
#[derive(Debug)]
pub struct App {
    /// Buffer name as shown in the status bar.
    pub(crate) file: String,
    pub(crate) caps: Caps,
    editor: Editor,
    viewport: Viewport,
    body_height: u16,
    redraw: bool,
    quit: bool,
}

impl App {
    /// Empty buffer named `[No Name]`.
    pub fn new() -> Self {
        Self::from_text("", "")
    }

    /// A buffer with known text, used by tests and by [`App::open`].
    pub fn from_text(file: &str, text: &str) -> Self {
        Self {
            file: if file.is_empty() {
                "[No Name]".to_string()
            } else {
                file.to_string()
            },
            caps: Caps::detect(),
            editor: Editor::from_text(text),
            viewport: Viewport::default(),
            body_height: 0,
            redraw: true,
            quit: false,
        }
    }

    /// Read `path` into a buffer. A missing file opens an empty buffer with that
    /// name, as nano does; anything else is an error.
    pub fn open(path: Option<String>) -> io::Result<Self> {
        let Some(path) = path else {
            return Ok(Self::new());
        };
        match std::fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => Ok(Self::from_text(&path, &text)),
                Err(_) => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{path}: not valid UTF-8 (encoding detection arrives in phase 4)"),
                )),
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::from_text(&path, "")),
            Err(err) => Err(err),
        }
    }

    /// Run until the user quits. The `Session` guard restores the terminal on
    /// every exit path: normal quit, `?`, or unwind.
    pub fn run(mut self) -> io::Result<()> {
        // The editor owns the whole screen, so there is nothing to render into a
        // pipe. Refuse early with a readable message instead of failing inside
        // raw-mode setup with `ENXIO`.
        if !io::stdout().is_terminal() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "standard output is not a terminal",
            ));
        }
        let mut session = term::Session::start()?;
        while !self.quit {
            if self.redraw {
                let size = session.terminal().size()?;
                let body_height = size.height.saturating_sub(1);
                self.body_height = body_height;
                ui::ensure_cursor_visible(&mut self, size.width, body_height);
                session.terminal().draw(|frame| ui::draw(frame, &self))?;
                self.redraw = false;
            }
            if !event::poll(POLL_INTERVAL)? {
                continue;
            }
            match event::read()? {
                Event::Key(key) => {
                    if let Some(action) = term::input::decode(&key) {
                        self.apply(action);
                    }
                }
                Event::Resize(_, _) => self.redraw = true,
                _ => {}
            }
        }
        Ok(())
    }

    /// Single mutation path: keyboard (and later mouse and palette) all land here.
    fn apply(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit = true,
            // Pages are the only actions that need viewport geometry.
            Action::PageUp => self.editor.move_vertical(-page_step(self.body_height)),
            Action::PageDown => self.editor.move_vertical(page_step(self.body_height)),
            other => self.editor.apply(other),
        }
        self.redraw = true;
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.editor
    }

    pub(crate) fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub(crate) fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    pub(crate) fn tab_width(&self) -> usize {
        // Becomes the `tab_size` registry option; 8 until the registry exists.
        DEFAULT_TAB_WIDTH
    }

    /// Test seam: move the cursor and mark the frame dirty.
    #[cfg(test)]
    pub(crate) fn set_cursor(&mut self, line: usize, col: usize) {
        self.editor.set_cursor(line, col);
        self.redraw = true;
    }

    /// Left of the status bar: identity, dirty marker and the multiplexer notice.
    /// `^B`/`^A` are documented, never rebound: the multiplexer keeps its prefix.
    fn status_left(&self) -> String {
        let mut text = format!(" nnano {}", env!("CARGO_PKG_VERSION"));
        if self.editor.is_dirty() {
            text.push_str("  [Modified]");
        }
        if self.caps.tmux {
            text.push_str("   ^B: tmux prefix");
        } else if self.caps.screen {
            text.push_str("   ^A: screen prefix");
        }
        text
    }

    /// Right of the status bar: `line:column  file`, one-based and visual, as
    /// nano prints it.
    fn status_right(&self) -> String {
        let (line, col) = self
            .editor
            .cursor()
            .display_position(self.editor.buffer(), self.tab_width());
        format!("{line}:{col}  {} ", self.file)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// One page is the visible area minus the row the cursor already sits on.
fn page_step(body_height: u16) -> isize {
    isize::try_from(body_height.saturating_sub(1).max(1)).unwrap_or(isize::MAX)
}

/// Entry point used by the `nnano` binary.
pub fn run(file: Option<String>) -> io::Result<()> {
    App::open(file)?.run()
}

#[cfg(test)]
mod tests {
    use nnano_core::editor::Action;

    use super::{App, page_step};

    #[test]
    fn page_step_leaves_the_cursor_row_visible() {
        assert_eq!(page_step(24), 23);
        assert_eq!(page_step(1), 1);
        assert_eq!(page_step(0), 1);
    }

    #[test]
    fn typing_through_actions_marks_the_buffer_dirty() {
        let mut app = App::from_text("x.txt", "");
        app.apply(Action::Insert('a'));
        app.apply(Action::Insert('b'));
        assert!(app.editor().is_dirty());
        assert_eq!(app.editor().buffer().line_text(0), "ab");
    }

    #[test]
    fn quit_action_ends_the_loop() {
        let mut app = App::from_text("x.txt", "");
        app.apply(Action::Quit);
        assert!(app.quit);
    }

    #[test]
    fn pages_move_by_the_body_height() {
        let text = (0..100).map(|n| format!("line {n}\n")).collect::<String>();
        let mut app = App::from_text("long.txt", &text);
        app.body_height = 10;
        app.apply(Action::PageDown);
        assert_eq!(app.editor().cursor().line(), 9);
        app.apply(Action::PageUp);
        assert_eq!(app.editor().cursor().line(), 0);
    }

    #[test]
    fn missing_file_opens_an_empty_buffer_with_that_name() {
        let path = std::env::temp_dir().join("nnano-does-not-exist.txt");
        let _ = std::fs::remove_file(&path);
        let app = App::open(Some(path.display().to_string())).unwrap();
        assert!(app.editor().buffer().is_empty());
        assert!(app.file.ends_with("nnano-does-not-exist.txt"));
    }

    #[test]
    fn a_file_is_read_in_one_pass() {
        let path = std::env::temp_dir().join("nnano-phase2-open.txt");
        std::fs::write(&path, "uno\ndos\n").unwrap();
        let app = App::open(Some(path.display().to_string())).unwrap();
        assert_eq!(app.editor().buffer().line_count(), 3);
        assert_eq!(app.editor().buffer().line_text(1), "dos");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_utf8_is_refused_for_now() {
        let path = std::env::temp_dir().join("nnano-phase2-latin1.txt");
        std::fs::write(&path, b"caf\xe9\n").unwrap();
        let err = App::open(Some(path.display().to_string())).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn no_argument_opens_an_unnamed_buffer() {
        let app = App::open(None).unwrap();
        assert_eq!(app.file, "[No Name]");
    }
}
