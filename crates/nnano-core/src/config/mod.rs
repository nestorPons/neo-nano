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
pub use paths::{Paths, detect_state_dir, state_dir};
pub use toml_source::from_toml;

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use crate::buffer::DEFAULT_TAB_WIDTH;
use crate::i18n::{CODES as LANGUAGE_CODES, Lang};

/// Compile-time handle for one option: what code passes to `Config::get`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    TabSize,
    CaseSensitive,
    Mouse,
    Syntax,
    Osc52,
    Language,
}

/// Type of an option's value, with its bounds. Drives clap, TOML coercion, the
/// nanorc word parser and the overlay widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int {
        min: i64,
        max: i64,
    },
    /// A closed set of strings, e.g. the language codes. The list is the single
    /// source of truth for the CLI's values, TOML validation and the overlay's
    /// cycling picker.
    Enum(&'static [&'static str]),
}

impl Kind {
    /// Human wording reused by every surface's error message.
    pub fn expected(self) -> &'static str {
        match self {
            Kind::Bool => "true or false",
            Kind::Int { .. } => "an integer",
            Kind::Enum(_) => "one of the allowed values",
        }
    }

    /// The allowed values of an [`Kind::Enum`], for the surfaces that list them.
    pub fn values(self) -> Option<&'static [&'static str]> {
        match self {
            Kind::Enum(values) => Some(values),
            Kind::Bool | Kind::Int { .. } => None,
        }
    }
}

/// A value an option can hold. `Text` only ever holds a `&'static str` taken
/// from its row's [`Kind::Enum`] list, so it stays `Copy` and cannot point into
/// a file that has already been dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Text(&'static str),
}

impl Value {
    pub fn as_bool(self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_int(self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_text(self) -> Option<&'static str> {
        match self {
            Value::Text(value) => Some(value),
            _ => None,
        }
    }

    /// Overlay text: `on`/`off` for booleans, the number as typed, the code for
    /// an enum. Neutral on purpose; the TUI translates `on`/`off` itself.
    pub fn label(self) -> String {
        match self {
            Value::Bool(true) => "on".to_string(),
            Value::Bool(false) => "off".to_string(),
            Value::Int(value) => value.to_string(),
            Value::Text(value) => value.to_string(),
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
    OptRow {
        id: Opt::Syntax,
        key: "syntax",
        long: "syntax",
        short: None,
        kind: Kind::Bool,
        default: Value::Bool(true),
        nanorc: Some("syntax"),
        group: "Editor",
        label: "Syntax highlighting",
        help: "Colour lines with the grammar of the file's extension",
    },
    OptRow {
        id: Opt::Mouse,
        key: "mouse",
        long: "mouse",
        short: Some('m'),
        kind: Kind::Bool,
        default: Value::Bool(true),
        nanorc: Some("mouse"),
        group: "Interface",
        label: "Mouse support",
        help: "Click to place the cursor, drag to select, wheel to scroll",
    },
    OptRow {
        id: Opt::Osc52,
        key: "osc52",
        long: "osc52",
        short: None,
        kind: Kind::Bool,
        default: Value::Bool(false),
        // nano has no equivalent: this is a copy *out*, opt-in because some
        // terminals ignore it and a slow tty can stall on a big sequence.
        nanorc: None,
        group: "Interface",
        label: "Copy out with OSC 52",
        help: "Also send a copy to the system clipboard, capped at 64 KiB",
    },
    OptRow {
        id: Opt::Language,
        key: "language",
        long: "language",
        short: None,
        kind: Kind::Enum(LANGUAGE_CODES),
        default: Value::Text("en"),
        // nano has no interface language, and `.nanorc` has no directive to map.
        nanorc: None,
        group: "Interface",
        label: "Language",
        help: "Interface language; auto follows the system locale",
    },
];

/// Every variant, for the drift test. A new variant breaks the exhaustive
/// `match` in `tests::every_variant_is_listed` before it can be forgotten here.
#[cfg(test)]
const ALL_OPTS: &[Opt] = &[
    Opt::TabSize,
    Opt::CaseSensitive,
    Opt::Mouse,
    Opt::Syntax,
    Opt::Osc52,
    Opt::Language,
];

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
    /// Check a value against this row's kind and bounds. An enum also
    /// canonicalises: `ZH-hans` and `zh` both land on the code the registry
    /// declares, so the value stored is always one of `Kind::values`.
    pub fn validate(&self, value: Value) -> Result<Value, ValueError> {
        match (self.kind, value) {
            (Kind::Bool, Value::Bool(_)) => Ok(value),
            (Kind::Int { min, max }, Value::Int(number)) if (min..=max).contains(&number) => {
                Ok(Value::Int(number))
            }
            (Kind::Int { min, max }, Value::Int(_)) => Err(ValueError::Range { min, max }),
            (Kind::Enum(values), Value::Text(text)) => self
                .canonical(values, text)
                .map(Value::Text)
                .ok_or(ValueError::Type {
                    expected: self.kind.expected(),
                }),
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
            Kind::Enum(values) => {
                Value::Text(self.canonical(values, raw).ok_or(ValueError::Type {
                    expected: self.kind.expected(),
                })?)
            }
        };
        self.validate(value)
    }

    /// The declared spelling of `raw`, or `None` when the set does not allow it.
    /// Language codes additionally accept the usual locale aliases (`zh_CN`,
    /// `pt-BR`, `in`), because those are what a user copies from `$LANG`.
    fn canonical(&self, values: &'static [&'static str], raw: &str) -> Option<&'static str> {
        if let Some(found) = values
            .iter()
            .find(|value| value.eq_ignore_ascii_case(raw.trim()))
        {
            return Some(*found);
        }
        if self.id == Opt::Language {
            let code = Lang::from_code(raw)?.code();
            return values.iter().copied().find(|value| *value == code);
        }
        None
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

    /// Interface language of the session. `auto` (the only non-literal value)
    /// resolves through `$LC_ALL`/`$LC_MESSAGES`/`$LANG`; anything else is an
    /// exact code from `Kind::Enum`. English when the row is somehow not text.
    pub fn language(&self) -> Lang {
        match self.get(Opt::Language) {
            Value::Text(code) => Lang::resolve(code),
            Value::Bool(_) | Value::Int(_) => Lang::default(),
        }
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
                Opt::TabSize
                | Opt::CaseSensitive
                | Opt::Mouse
                | Opt::Syntax
                | Opt::Osc52
                | Opt::Language => {}
            }
        }
        // Hand-maintained count: extend it together with ALL_OPTS.
        assert_eq!(ALL_OPTS.len(), 6);
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

    #[test]
    fn copying_out_is_opt_in_and_has_no_nanorc_directive() {
        // nano has no equivalent, so the nanorc reader must not claim one, and the
        // default is off: a copy never stalls a slow tty unless asked to.
        assert_eq!(row(Opt::Osc52).default, Value::Bool(false));
        assert_eq!(row(Opt::Osc52).nanorc, None);
        assert_eq!(row(Opt::Osc52).kind, Kind::Bool);
    }

    #[test]
    fn an_enum_accepts_only_its_own_values_and_canonicalises_them() {
        use crate::i18n::Lang;

        let language = row(Opt::Language);
        assert_eq!(language.parse("es"), Ok(Value::Text("es")));
        // Declared spelling wins, so one `Value` per language.
        assert_eq!(language.parse("ZH-HANS"), Ok(Value::Text("zh-Hans")));
        assert_eq!(language.parse("zh_CN.UTF-8"), Ok(Value::Text("zh-Hans")));
        assert_eq!(language.parse(" auto "), Ok(Value::Text("auto")));
        assert!(language.parse("klingon").is_err());
        assert!(language.validate(Value::Int(1)).is_err());
        assert!(language.validate(Value::Bool(true)).is_err());
        assert_eq!(language.default, Value::Text("en"));
        // Every code the language table declares is accepted.
        for code in crate::i18n::CODES {
            assert!(language.parse(code).is_ok(), "{code}");
        }
        assert_eq!(
            language.kind.values().map(|values| values.len()),
            Some(Lang::COUNT + 1)
        );
    }

    #[test]
    fn the_language_option_resolves_to_a_language() {
        use crate::i18n::Lang;

        let defaults = Config::defaults();
        assert_eq!(defaults.language(), Lang::En, "the default is English");

        let mut config = Config::defaults();
        assert_eq!(
            config.set(Opt::Language, Value::Text("ja")),
            Ok(()),
            "a live change from the settings overlay"
        );
        assert_eq!(config.language(), Lang::Ja);

        config.set(Opt::Language, Value::Text("auto")).unwrap();
        assert!(Lang::ALL.contains(&config.language()));
        assert!(config.set(Opt::Language, Value::Text("xx")).is_err());
        assert_eq!(config.language(), Lang::detect());
    }

    #[test]
    fn the_language_row_has_no_nanorc_directive() {
        assert_eq!(row(Opt::Language).nanorc, None);
        assert_eq!(row(Opt::Language).group, "Interface");
        assert_eq!(row(Opt::Language).long, "language");
    }
}
