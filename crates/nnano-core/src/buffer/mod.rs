//! Rope buffer (phase 2).
//!
//! [`TextBuffer`] is the only place in the workspace that names `ropey`. The
//! crate itself never leaks past `nnano-core::buffer`.
//!
//! Invariants from `AGENTS.md`:
//! - load is one O(n) pass (`from_text`), never line by line;
//! - line/column lookups go through the rope's cached newline index, so they are
//!   O(log n) and never a scan;
//! - bytes are stored **as loaded**: `\r\n` is not normalised here, and grapheme
//!   and width mapping stay at the edges (`grapheme`, `render`, `save`).

mod cursor;
mod grapheme;
mod rope;

pub use cursor::Cursor;
pub use grapheme::{DEFAULT_TAB_WIDTH, Width, line_visual_column, next_tab_stop};
pub use rope::TextBuffer;
