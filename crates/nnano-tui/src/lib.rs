//! Terminal layer for `nnano`.
//!
//! Owns everything that assumes a terminal: raw mode and its restore path, the
//! panic hook, capability detection, input decoding, drawing and overlays. All
//! editor semantics stay in `nnano-core`.
//!
//! Event loop (phase 2): `poll(16ms)` -> drain events -> redraw only when dirty.
//! No fixed tick, no async runtime, no worker thread yet.

pub mod term;
pub mod ui;

use std::io;
use std::io::IsTerminal;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nnano_core::buffer::{Cursor, DEFAULT_TAB_WIDTH, TextBuffer};

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
    buffer: TextBuffer,
    cursor: Cursor,
    viewport: Viewport,
    dirty: bool,
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
            buffer: TextBuffer::from_text(text),
            cursor: Cursor::new(0, 0),
            viewport: Viewport::default(),
            dirty: true,
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
        loop {
            if self.dirty {
                self.cursor.clamp(&self.buffer);
                let size = session.terminal().size()?;
                ui::ensure_cursor_visible(&mut self, size.width, size.height.saturating_sub(1));
                session.terminal().draw(|frame| ui::draw(frame, &self))?;
                self.dirty = false;
            }
            if !event::poll(POLL_INTERVAL)? {
                continue;
            }
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press && is_quit(&key) => break,
                Event::Resize(_, _) => self.dirty = true,
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub(crate) fn cursor(&self) -> &Cursor {
        &self.cursor
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

    /// Test seam for scroll/cursor rendering: move the cursor and mark the frame
    /// dirty. Phase 3 replaces it with `Action`-driven movement.
    #[cfg(test)]
    pub(crate) fn set_cursor(&mut self, line: usize, col: usize) {
        self.cursor = Cursor::new(line, col);
        self.dirty = true;
    }

    /// Left of the status bar: identity, plus the multiplexer notice. `^B`/`^A`
    /// are documented, never rebound: the multiplexer keeps its prefix.
    fn status_left(&self) -> String {
        let mut text = format!(" nnano {}", env!("CARGO_PKG_VERSION"));
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
        let (line, col) = self.cursor.display_position(&self.buffer, self.tab_width());
        format!("{line}:{col}  {} ", self.file)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// `^Q` quits and nothing else does: `^C`/`^Z` are actions, `Esc` belongs to
/// overlays and prompts.
fn is_quit(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Entry point used by the `nnano` binary.
pub fn run(file: Option<String>) -> io::Result<()> {
    App::open(file)?.run()
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{App, is_quit};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn control_q_quits() {
        assert!(is_quit(&key(KeyCode::Char('q'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn plain_q_and_other_keys_do_not_quit() {
        assert!(!is_quit(&key(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(!is_quit(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(!is_quit(&key(KeyCode::Esc, KeyModifiers::NONE)));
    }

    #[test]
    fn missing_file_opens_an_empty_buffer_with_that_name() {
        let path = std::env::temp_dir().join("nnano-does-not-exist.txt");
        let _ = std::fs::remove_file(&path);
        let app = App::open(Some(path.display().to_string())).unwrap();
        assert!(app.buffer().is_empty());
        assert!(app.file.ends_with("nnano-does-not-exist.txt"));
    }

    #[test]
    fn a_file_is_read_in_one_pass() {
        let path = std::env::temp_dir().join("nnano-phase2-open.txt");
        std::fs::write(&path, "uno\ndos\n").unwrap();
        let app = App::open(Some(path.display().to_string())).unwrap();
        assert_eq!(app.buffer().line_count(), 3);
        assert_eq!(app.buffer().line_text(1), "dos");
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
