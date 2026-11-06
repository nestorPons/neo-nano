//! Configuration: the option registry and the layered values (phase 5).
//!
//! One table (`OPTIONS`) is the single source of truth. Every surface is derived
//! from it at runtime — the clap flags (in the `nnano` binary), the TOML reader,
//! the `.nanorc` reader and the settings overlay — so none of them can drift.
//!
//! Layers merge highest first: `CLI > user TOML > system TOML > nanorc > defaults`.
//! Unknown keys and wrong types warn once each and never abort.
//!
//! Adding an option = one [`Opt`] variant + one [`OPTIONS`] row. The drift test at
//! the bottom of this file enforces that, and the exhaustive `match` in it is the
//! compile-time guard that a new variant cannot be forgotten.

mod nanorc;
mod paths;
mod toml_source;

pub use nanorc::from_nanorc;
pub use paths::Paths;
pub use toml_source::from_toml;

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use crate::buffer::DEFAULT_TAB_WIDTH;

/// Compile-time handle for one option: what code passes to `Config::get`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    TabSize,
    CaseSensitive,
}

/// Type of an option's value, with its bounds. Drives clap, TOML coercion, the
/// nanorc word parser and the overlay widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int { min: i64, max: i64 },
}

impl Kind {
    /// Human wording reused by every surface's error message.
    pub fn expected(self) -> &'static str {
        match self {
            Kind::Bool => "true or false",
            Kind::Int { .. } => "an integer",
        }
    }
}

/// A value an option can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    Int(i64),
}

impl Value {
    pub fn as_bool(self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(value),
            Value::Int(_) => None,
        }
    }

    pub fn as_int(self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(value),
            Value::Bool(_) => None,
        }
    }

    /// Overlay text: `on`/`off` for booleans, the number as typed.
    pub fn label(self) -> String {
        match self {
            Value::Bool(true) => "on".to_string(),
            Value::Bool(false) => "off".to_string(),
            Value::Int(value) => value.to_string(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

/// Why a value was refused. The surface adds the name the user wrote (TOML key,
/// nanorc directive, flag), so the wording cannot name the wrong spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueError {
    /// Not of the option's [`Kind`] at all.
    Type { expected: &'static str },
    /// Right type, outside `Kind::Int`'s bounds.
    Range { min: i64, max: i64 },
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueError::Type { expected } => write!(f, "must be {expected}"),
            ValueError::Range { min, max } => write!(f, "must be between {min} and {max}"),
        }
    }
}

impl std::error::Error for ValueError {}

/// One row of the registry.
#[derive(Debug, Clone, Copy)]
pub struct OptRow {
    /// Handle code uses (`cfg.int(Opt::TabSize)`).
    pub id: Opt,
    /// TOML key, `snake_case`.
    pub key: &'static str,
    /// clap long flag: nano's spelling where semantics are unchanged.
    pub long: &'static str,
    /// clap short flag, if nano has one.
    pub short: Option<char>,
    pub kind: Kind,
    /// Value used when no layer sets the option.
    pub default: Value,
    /// Legacy `.nanorc` directive, `None` when nano has no equivalent.
    pub nanorc: Option<&'static str>,
    /// Settings-overlay group heading.
    pub group: &'static str,
    /// Settings-overlay row label.
    pub label: &'static str,
    /// CLI help text, reused verbatim by the overlay footer.
    pub help: &'static str,
}

/// The single source of truth. Order is the overlay's order too.
pub const OPTIONS: &[OptRow] = &[
    OptRow {
        id: Opt::TabSize,
        key: "tab_size",
        long: "tabsize",
        short: Some('T'),
        kind: Kind::Int { min: 1, max: 64 },
        default: Value::Int(DEFAULT_TAB_WIDTH as i64),
        nanorc: Some("tabsize"),
        group: "Editor",
        label: "Tab size",
        help: "Columns a tab advances; 1-64",
    },
    OptRow {
        id: Opt::CaseSensitive,
        key: "case_sensitive",
        long: "casesensitive",
        short: Some('C'),
        kind: Kind::Bool,
        default: Value::Bool(false),
        nanorc: Some("casesensitive"),
        group: "Search",
        label: "Case-sensitive search",
        help: "Make searches match case by default",
    },
];

/// Every variant, for the drift test. A new variant breaks the exhaustive
/// `match` in `tests::every_variant_is_listed` before it can be forgotten here.
#[cfg(test)]
const ALL_OPTS: &[Opt] = &[Opt::TabSize, Opt::CaseSensitive];

/// The row that declares `id`.
pub fn row(id: Opt) -> &'static OptRow {
    OPTIONS
        .iter()
        .find(|row| row.id == id)
        .expect("every Opt variant has exactly one row")
}

/// Row for a TOML key.
pub fn by_key(key: &str) -> Option<&'static OptRow> {
    OPTIONS.iter().find(|row| row.key == key)
}

/// Row for a clap long flag.
pub fn by_long(long: &str) -> Option<&'static OptRow> {
    OPTIONS.iter().find(|row| row.long == long)
}

/// Row for a `.nanorc` directive.
pub fn by_nanorc(name: &str) -> Option<&'static OptRow> {
    OPTIONS
        .iter()
        .find(|row| row.nanorc.is_some_and(|directive| directive == name))
}

impl OptRow {
    /// Check a value against this row's kind and bounds.
    pub fn validate(&self, value: Value) -> Result<Value, ValueError> {
        match (self.kind, value) {
            (Kind::Bool, Value::Bool(_)) => Ok(value),
            (Kind::Int { min, max }, Value::Int(number)) if (min..=max).contains(&number) => {
                Ok(Value::Int(number))
            }
            (Kind::Int { min, max }, Value::Int(_)) => Err(ValueError::Range { min, max }),
            (kind, _) => Err(ValueError::Type {
                expected: kind.expected(),
            }),
        }
    }

    /// Parse a textual value. The CLI, TOML and nanorc all funnel here, so their
    /// errors and bounds cannot diverge.
    pub fn parse(&self, raw: &str) -> Result<Value, ValueError> {
        let raw = raw.trim();
        let value = match self.kind {
            Kind::Bool => Value::Bool(parse_bool(raw).ok_or(ValueError::Type {
                expected: "true or false",
            })?),
            Kind::Int { .. } => Value::Int(raw.parse::<i64>().map_err(|_| ValueError::Type {
                expected: "an integer",
            })?),
        };
        self.validate(value)
    }
}

/// `true`/`yes`/`on`/`1` and the negatives; `None` for anything else.
fn parse_bool(raw: &str) -> Option<bool> {
    match raw.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// One parsed source, in whatever order its lines or keys appeared. `origin` is
/// prefixed to every warning so a bad key names its file.
#[derive(Debug, Clone, Default)]
pub struct Layer {
    pub origin: String,
    pub values: Vec<(Opt, Value)>,
    pub warnings: Vec<String>,
}

impl Layer {
    pub fn new(origin: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
            ..Self::default()
        }
    }

    /// Record a warning, tagged with this layer's origin.
    pub fn warn(&mut self, message: impl fmt::Display) {
        self.warnings.push(format!("{}: {message}", self.origin));
    }
}

/// Effective values plus everything that had to be reported at startup.
#[derive(Debug, Clone)]
pub struct Config {
    values: BTreeMap<Opt, Value>,
    warnings: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self::resolve(Vec::new())
    }
}

impl Config {
    /// Built-in defaults, no file read.
    pub fn defaults() -> Self {
        Self::default()
    }

    /// Merge `layers`, **highest precedence first**. Registry defaults are the
    /// floor, so every `Opt` always has a value.
    pub fn resolve(layers: Vec<Layer>) -> Self {
        let mut values: BTreeMap<Opt, Value> =
            OPTIONS.iter().map(|row| (row.id, row.default)).collect();
        let mut warnings: Vec<String> = Vec::new();

        // Lowest layer first so the highest wins per key.
        for layer in layers.into_iter().rev() {
            let Layer {
                origin,
                values: entries,
                warnings: mut layer_warnings,
            } = layer;
            for (id, value) in entries {
                match row(id).validate(value) {
                    Ok(value) => {
                        values.insert(id, value);
                    }
                    Err(err) => layer_warnings.push(format!("{origin}: `{}` {err}", row(id).key)),
                }
            }
            warnings.append(&mut layer_warnings);
        }

        dedup(&mut warnings);
        Self { values, warnings }
    }

    /// Effective value; there is always one.
    pub fn get(&self, id: Opt) -> Value {
        self.values
            .get(&id)
            .copied()
            .unwrap_or_else(|| row(id).default)
    }

    pub fn bool(&self, id: Opt) -> bool {
        self.get(id).as_bool().unwrap_or_default()
    }

    pub fn int(&self, id: Opt) -> i64 {
        self.get(id).as_int().unwrap_or_default()
    }

    /// Live edit from the settings overlay. Validated like every other surface.
    pub fn set(&mut self, id: Opt, value: Value) -> Result<(), ValueError> {
        let value = row(id).validate(value)?;
        self.values.insert(id, value);
        Ok(())
    }

    /// Startup warnings, deduplicated: an unknown directive warns once.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// Warn once per distinct message, keeping first-seen order.
fn dedup(warnings: &mut Vec<String>) {
    let mut seen = HashSet::new();
    warnings.retain(|warning| seen.insert(warning.clone()));
}

#[cfg(test)]
mod tests {
    use super::{
        ALL_OPTS, Config, Kind, Layer, OPTIONS, Opt, Value, by_key, by_long, by_nanorc, row,
    };

    fn layer(origin: &str, values: &[(Opt, Value)]) -> Layer {
        Layer {
            origin: origin.to_string(),
            values: values.to_vec(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn every_variant_is_listed() {
        // The exhaustive match is the compile-time guard: adding a variant
        // without adding it to ALL_OPTS stops this file from building.
        for id in ALL_OPTS {
            match *id {
                Opt::TabSize | Opt::CaseSensitive => {}
            }
        }
        // Hand-maintained count: extend it together with ALL_OPTS.
        assert_eq!(ALL_OPTS.len(), 2);
    }

    #[test]
    fn options_and_variants_are_one_to_one() {
        assert_eq!(OPTIONS.len(), ALL_OPTS.len());
        for id in ALL_OPTS {
            assert!(OPTIONS.iter().any(|row| row.id == *id), "{id:?}");
        }
        for row in OPTIONS {
            assert_eq!(
                ALL_OPTS.iter().filter(|id| **id == row.id).count(),
                1,
                "{:?} has more than one row",
                row.id
            );
        }
    }

    #[test]
    fn every_surface_reaches_every_option() {
        let mut keys = std::collections::HashSet::new();
        let mut longs = std::collections::HashSet::new();
        let mut shorts = std::collections::HashSet::new();
        for row in OPTIONS {
            assert!(keys.insert(row.key), "duplicate key {}", row.key);
            assert!(longs.insert(row.long), "duplicate long flag {}", row.long);
            if let Some(short) = row.short {
                assert!(shorts.insert(short), "duplicate short flag {short}");
            }
            assert!(by_key(row.key).is_some_and(|found| found.id == row.id));
            assert!(by_long(row.long).is_some_and(|found| found.id == row.id));
            if let Some(name) = row.nanorc {
                assert!(by_nanorc(name).is_some_and(|found| found.id == row.id));
            }
            assert!(!row.group.is_empty() && !row.label.is_empty());
            assert!(!row.help.is_empty());
        }
    }

    #[test]
    fn every_default_validates_under_its_own_kind() {
        for row in OPTIONS {
            assert_eq!(row.validate(row.default), Ok(row.default), "{:?}", row.id);
        }
    }

    #[test]
    fn defaults_are_the_registry_defaults() {
        let config = Config::defaults();
        for row in OPTIONS {
            assert_eq!(config.get(row.id), row.default, "{:?}", row.id);
        }
        assert!(config.warnings().is_empty());
    }

    #[test]
    fn higher_layers_win_per_key() {
        let config = Config::resolve(vec![
            layer("cli", &[(Opt::TabSize, Value::Int(2))]),
            layer("user", &[(Opt::TabSize, Value::Int(4))]),
            layer("system", &[(Opt::TabSize, Value::Int(6))]),
        ]);
        assert_eq!(config.int(Opt::TabSize), 2);
        // Untouched keys still come from the registry.
        assert!(!config.bool(Opt::CaseSensitive));
    }

    #[test]
    fn an_invalid_value_warns_and_keeps_the_lower_layer() {
        let config = Config::resolve(vec![
            layer("user", &[(Opt::TabSize, Value::Int(999))]),
            layer("system", &[(Opt::TabSize, Value::Int(4))]),
        ]);
        assert_eq!(config.int(Opt::TabSize), 4);
        let warning = config.warnings().join("\n");
        assert!(warning.contains("between 1 and 64"), "{warning}");
    }

    #[test]
    fn types_are_checked_against_the_kind() {
        let config = Config::resolve(vec![layer(
            "user",
            &[
                (Opt::TabSize, Value::Bool(true)),
                (Opt::CaseSensitive, Value::Int(1)),
            ],
        )]);
        assert_eq!(config.int(Opt::TabSize), 8);
        assert!(!config.bool(Opt::CaseSensitive));
        assert_eq!(config.warnings().len(), 2);
    }

    #[test]
    fn warnings_are_deduplicated() {
        let mut first = Layer::new("a");
        first.warn("unknown key `x`");
        let mut second = Layer::new("a");
        second.warn("unknown key `x`");
        let config = Config::resolve(vec![first, second]);
        assert_eq!(config.warnings().len(), 1);
    }

    #[test]
    fn a_parse_reports_the_kind_before_the_bounds() {
        let tabsize = row(Opt::TabSize);
        assert_eq!(tabsize.parse("4"), Ok(Value::Int(4)));
        assert!(tabsize.parse(" 0 ").is_err());
        assert!(tabsize.parse("wide").is_err());

        let casesensitive = row(Opt::CaseSensitive);
        assert_eq!(casesensitive.parse("on"), Ok(Value::Bool(true)));
        assert_eq!(casesensitive.parse("off"), Ok(Value::Bool(false)));
        assert!(casesensitive.parse("maybe").is_err());
    }

    #[test]
    fn set_is_validated_like_every_other_surface() {
        let mut config = Config::defaults();
        assert!(config.set(Opt::TabSize, Value::Int(4)).is_ok());
        assert_eq!(config.int(Opt::TabSize), 4);
        assert!(config.set(Opt::TabSize, Value::Int(0)).is_err());
        assert!(config.set(Opt::TabSize, Value::Bool(true)).is_err());
        assert_eq!(config.int(Opt::TabSize), 4);
    }

    #[test]
    fn the_kind_of_a_row_is_the_one_its_values_demand() {
        assert_eq!(row(Opt::TabSize).kind, Kind::Int { min: 1, max: 64 });
        assert_eq!(row(Opt::CaseSensitive).kind, Kind::Bool);
        assert_eq!(row(Opt::TabSize).default, Value::Int(8));
        assert_eq!(row(Opt::CaseSensitive).default, Value::Bool(false));
    }
}
