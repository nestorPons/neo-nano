//! Terminal layer for `nnano`.
//!
//! Owns everything that assumes a terminal: raw mode and its restore path, the
//! panic hook, capability detection, input decoding, drawing and overlays. All
//! editor semantics stay in `nnano-core`.
//!
//! Event loop (phase 1): `poll(16ms)` -> drain events -> redraw only when dirty.
//! No fixed tick, no async runtime, no worker thread yet.

pub mod term;
pub mod ui;

use std::io;
use std::io::IsTerminal;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::term::Caps;

/// Event-poll granularity. It is not a redraw rate: a frame is drawn only when
/// the app is dirty.
const POLL_INTERVAL: Duration = Duration::from_millis(16);

/// Application state. Phase 1 holds only what the blank screen shows; the rope
/// buffers replace `file` in phase 2.
pub struct App {
    /// Buffer name as shown in the status bar.
    pub(crate) file: String,
    pub(crate) caps: Caps,
    dirty: bool,
}

impl App {
    pub fn new(file: Option<String>) -> Self {
        Self {
            file: file.unwrap_or_else(|| "[No Name]".to_string()),
            caps: Caps::detect(),
            dirty: true,
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

    fn status_right(&self) -> String {
        format!("{} ", self.file)
    }
}

/// `^Q` quits and nothing else does: `^C`/`^Z` are actions, `Esc` belongs to
/// overlays and prompts.
fn is_quit(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Entry point used by the `nnano` binary.
pub fn run(file: Option<String>) -> io::Result<()> {
    App::new(file).run()
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::is_quit;

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
}
