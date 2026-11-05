//! Terminal lifecycle (phase 1) and input (phase 3).
//!
//! Everything that assumes a terminal lives under `term/`: raw mode and its
//! restore path, the panic hook, capability detection and, later, event decoding
//! into `nnano_core::editor::Action`. `^H` and `^?` will both decode to
//! `Action::Backspace`.

mod caps;

pub use caps::Caps;

use std::io;

use ratatui::DefaultTerminal;

/// Owns the terminal for the whole run.
///
/// `start` enters raw mode and the alternate screen; `Drop` restores both, so an
/// early `?`, a normal quit and an unwind all leave a usable shell. `try_init`
/// also installs a panic hook that restores the terminal before printing the
/// panic, which covers a panic before the guard exists.
pub struct Session {
    terminal: DefaultTerminal,
}

impl Session {
    /// Raw mode + alternate screen.
    ///
    /// `ratatui::try_init` delegates to crossterm's `enable_raw_mode`, which is
    /// libc `cfmakeraw` + `tcsetattr`: it clears `IXON` and `ISIG`, so `^S`,
    /// `^Q`, `^C` and `^Z` arrive as keys instead of stopping the process.
    pub fn start() -> io::Result<Self> {
        Ok(Self {
            terminal: ratatui::try_init()?,
        })
    }

    pub fn terminal(&mut self) -> &mut DefaultTerminal {
        &mut self.terminal
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Idempotent: crossterm restores only while raw mode is still on.
        let _ = ratatui::try_restore();
    }
}
