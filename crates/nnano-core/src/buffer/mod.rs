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
pub mod encoding;
mod grapheme;
mod history;
mod rope;
mod selection;

pub use cursor::Cursor;
pub use encoding::{DecodeError, EncodeError, Encoding, Loaded};
pub use grapheme::{
    DEFAULT_TAB_WIDTH, Width, char_offset_for_column, line_visual_column, next_tab_stop,
};
pub use history::{Edit, History};
pub use rope::TextBuffer;
pub use selection::{Mode as SelectionMode, Selection};
