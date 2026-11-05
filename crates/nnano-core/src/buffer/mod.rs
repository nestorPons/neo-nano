//! Rope-backed text buffer (phase 2).
//!
//! Planned submodules, in this order: `rope` (the `ropey` wrapper that is the
//! only place the crate is named), `cursor`/`selection` as `(line, byte offset)`
//! ranges, `undo` (byte-range operation log, grouped at ~400 ms) and `grapheme`
//! (width/segmentation map used only at the render and save edges).
//!
//! Invariants from `AGENTS.md`: load is one O(n) pass, line/column lookups go
//! through the cached newline index, and `^K`/`^U` are range operations.
