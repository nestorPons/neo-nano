//! The user/system TOML layer.
//!
//! Parsed into a `toml::Table` and validated against the registry at runtime: no
//! `serde` derive, so a new option is a registry row and nothing else. Unknown
//! keys and wrong types warn once each; neither aborts.

use super::{Layer, OptRow, Value, by_key};

/// Parse `text` as the TOML layer named `origin`. Never fails: a broken file
/// becomes one warning and contributes no values.
pub fn from_toml(origin: &str, text: &str) -> Layer {
    let mut layer = Layer::new(origin);
    let table: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(err) => {
            layer.warn(format!("invalid TOML: {}", one_line(&err.to_string())));
            return layer;
        }
    };

    for (key, entry) in &table {
        let Some(row) = by_key(key) else {
            layer.warn(format!("unknown key `{key}`"));
            continue;
        };
        let Some(value) = from_toml_value(row, entry) else {
            layer.warn(format!("`{key}` must be {}", row.kind.expected()));
            continue;
        };
        match row.validate(value) {
            Ok(value) => layer.values.push((row.id, value)),
            Err(err) => layer.warn(format!("`{key}` {err}")),
        }
    }

    layer
}

/// The three shapes the registry can hold: booleans, integers and strings (an
/// `Enum`, whose value the row must canonicalise). Arrays, tables, floats and
/// datetimes are refused with the row's expected type.
fn from_toml_value(row: &OptRow, value: &toml::Value) -> Option<Value> {
    match value {
        toml::Value::Boolean(value) => Some(Value::Bool(*value)),
        toml::Value::Integer(value) => Some(Value::Int(*value)),
        // `language = "es"`: only an `Enum` row takes a string, and the row owns
        // the allowed set, so the value is parsed by the same code the CLI and
        // `.nanorc` use. A string for a bool or an int stays a type error.
        toml::Value::String(value) if row.kind.values().is_some() => row.parse(value).ok(),
        _ => None,
    }
}

/// TOML errors span several lines with a caret; one warning line is enough.
fn one_line(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::from_toml;
    use crate::config::{Opt, Value};

    #[test]
    fn reads_the_known_keys() {
        let layer = from_toml("test.toml", "tab_size = 4\ncase_sensitive = true\n");
        assert!(layer.warnings.is_empty(), "{:?}", layer.warnings);
        let mut values = layer.values;
        values.sort_by_key(|(id, _)| format!("{id:?}"));
        assert_eq!(
            values,
            vec![
                (Opt::CaseSensitive, Value::Bool(true)),
                (Opt::TabSize, Value::Int(4)),
            ]
        );
    }

    #[test]
    fn unknown_keys_warn_and_do_not_abort_the_file() {
        let layer = from_toml("test.toml", "nope = 1\ntab_size = 2\n");
        assert_eq!(layer.values, vec![(Opt::TabSize, Value::Int(2))]);
        assert_eq!(layer.warnings.len(), 1);
        assert!(layer.warnings[0].contains("unknown key `nope`"));
        assert!(layer.warnings[0].starts_with("test.toml: "));
    }

    #[test]
    fn a_wrong_type_warns_for_that_key_only() {
        let layer = from_toml("test.toml", "tab_size = \"four\"\ncase_sensitive = true\n");
        assert_eq!(layer.values, vec![(Opt::CaseSensitive, Value::Bool(true))]);
        assert_eq!(layer.warnings.len(), 1);
        assert!(layer.warnings[0].contains("`tab_size` must be"));
    }

    #[test]
    fn out_of_range_is_refused_with_the_bounds() {
        let layer = from_toml("test.toml", "tab_size = 0\n");
        assert!(layer.values.is_empty());
        assert!(layer.warnings[0].contains("between 1 and 64"));
    }

    #[test]
    fn a_broken_file_is_one_warning() {
        let layer = from_toml("test.toml", "this is not toml");
        assert!(layer.values.is_empty());
        assert_eq!(layer.warnings.len(), 1);
        assert!(layer.warnings[0].contains("invalid TOML"));
        assert!(!layer.warnings[0].contains('\n'));
    }

    #[test]
    fn tables_and_arrays_are_refused_not_ignored_silently() {
        let layer = from_toml("test.toml", "[editor]\ntab_size = 4\n");
        assert!(layer.values.is_empty());
        assert!(layer.warnings[0].contains("unknown key `editor`"));
    }

    #[test]
    fn a_string_sets_an_enum_and_is_canonicalised() {
        let layer = from_toml("test.toml", "language = \"es_ES\"\n");
        assert!(layer.warnings.is_empty(), "{:?}", layer.warnings);
        assert_eq!(layer.values, vec![(Opt::Language, Value::Text("es"))]);
    }

    #[test]
    fn an_unknown_enum_value_warns_for_that_key_only() {
        let layer = from_toml("test.toml", "language = \"xx\"\ntab_size = 2\n");
        assert_eq!(layer.values, vec![(Opt::TabSize, Value::Int(2))]);
        assert_eq!(layer.warnings.len(), 1);
        assert!(
            layer.warnings[0].contains("`language` must be"),
            "{layer:?}"
        );
    }

    #[test]
    fn a_string_is_still_a_type_error_for_a_bool() {
        let layer = from_toml("test.toml", "mouse = \"yes\"\n");
        assert!(layer.values.is_empty());
        assert!(layer.warnings[0].contains("must be true or false"));
    }
}
