//! Editor semantics for `nnano`.
//!
//! This crate must not know that a terminal exists: everything here is testable
//! without a TTY. The binary and the TUI build on top of it; the reverse edge
//! never exists.

pub mod buffer;
pub mod config;
pub mod editor;
pub mod search;
pub mod syntax;
