//! nano operation layer (phases 2 and 7).
//!
//! Planned: cut line / paste line (`^K`/`^U`), justify (`^J`), replace (`^\`,
//! `M-R`), goto line (`^G`), plus the `Action` enum that keyboard, mouse and the
//! command palette all produce. `Action` lives here so no surface can drift.
