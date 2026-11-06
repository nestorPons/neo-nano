//! Terminal lifecycle (phase 1) and input (phase 3).
//!
//! Everything that assumes a terminal lives under `term/`: raw mode and its
//! restore path, the panic hook, capability detection and, later, event decoding
//! into `nnano_core::editor::Action`. `^H` and `^?` will both decode to
//! `Action::Backspace`.

mod caps;
pub mod input;

pub use caps::Caps;

use std::io;

use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::execute;
use ratatui::DefaultTerminal;

/// Owns the terminal for the whole run.
///
/// `start` enters raw mode and the alternate screen; `Drop` restores both, so an
/// early `?`, a normal quit and an unwind all leave a usable shell. `try_init`
/// also installs a panic hook that restores the terminal before printing the
/// panic, which covers a panic before the guard exists.
pub struct Session {
    terminal: DefaultTerminal,
    /// Whether mouse capture is currently on, so the app loop can set the option
    /// every iteration without emitting an escape sequence each time.
    mouse: bool,
}

impl Session {
    /// Raw mode + alternate screen.
    ///
    /// `ratatui::try_init` delegates to crossterm's `enable_raw_mode`, which is
    /// libc `cfmakeraw` + `tcsetattr`: it clears `IXON` and `ISIG`, so `^S`,
    /// `^Q`, `^C` and `^Z` arrive as keys instead of stopping the process.
    pub fn start() -> io::Result<Self> {
        let terminal = ratatui::try_init()?;
        // Bracketed paste is always on: it is what keeps a multi-line paste from
        // being replayed as keystrokes. Mouse capture follows the `mouse` option.
        execute!(io::stdout(), EnableBracketedPaste)?;
        Ok(Self {
            terminal,
            mouse: false,
        })
    }

    pub fn terminal(&mut self) -> &mut DefaultTerminal {
        &mut self.terminal
    }

    /// Turn mouse capture on or off. Idempotent: the sequence is only emitted when
    /// the state actually changes, because the app loop calls this every tick.
    pub fn set_mouse(&mut self, enabled: bool) -> io::Result<()> {
        if self.mouse == enabled {
            return Ok(());
        }
        let mut out = io::stdout();
        if enabled {
            execute!(out, EnableMouseCapture)?;
        } else {
            execute!(out, DisableMouseCapture)?;
        }
        self.mouse = enabled;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Both are idempotent; the terminal must never be left capturing mouse
        // events or swallowing a paste after a panic or a normal quit.
        let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
        // Idempotent: crossterm restores only while raw mode is still on.
        let _ = ratatui::try_restore();
    }
}
