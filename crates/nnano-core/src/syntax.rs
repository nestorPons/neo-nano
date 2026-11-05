//! nano-style syntax highlighting (phase 7).
//!
//! Two parts: a parser for `.nanorc` `syntax` / `color` / `icolor` blocks and a
//! per-line, stateless span matcher. Only viewport lines are matched, at redraw
//! time; there is no multi-line state, no cache and no mirror of the buffer.
//! Regexes are POSIX ERE on `regex-lite`; `[[:alpha:]]` and friends must parse.
