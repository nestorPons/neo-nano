//! Configuration (phase 1/5): option registry, defaults, TOML table reader,
//! nanorc reader.
//!
//! Planned shape, per `AGENTS.md` *Option registry*:
//! - one `const OPTIONS: &[Opt]` table and one `Opt` variant per option;
//! - `kind` (`Bool`, `Int { min, max }`, `Text`, `Enum(&[&str])`) drives both the
//!   clap `Arg` and the TOML coercion, so CLI and file errors cannot diverge;
//! - TOML is parsed into `toml::Table` and validated against the table at
//!   runtime (no `serde` derive); unknown key or wrong type warns once, never
//!   aborts; `include` is never followed;
//! - layers merge as `Vec<(Layer, Map<Opt, Value>)>`, higher wins per key, and
//!   registry defaults are applied last;
//! - a drift test iterates `OPTIONS` and checks every surface: CLI, TOML,
//!   overlay and nanorc.
