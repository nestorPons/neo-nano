//! Search with nano's semantics (phase 7).
//!
//! Runs once on an accepted prompt (never incremental), literal by default and
//! POSIX ERE through `regex-lite` while the *regexp* toggle is on, compiled once
//! per accepted prompt. Starts at the cursor, wraps around, and reports
//! *Search wrapped*.
