//! The legacy `.nanorc` layer: read-only, never migrated, never rewritten.
//!
//! `nnano` and `nano` must stay safe to use side by side, so this reader only
//! *reads* the directives that map 1:1 to a registry row. `syntax`, `color` and
//! `icolor` belong to the grammar table (phase 7) and are accepted silently;
//! `include` is never followed; anything else warns once and is skipped.

use super::{Layer, by_nanorc};

/// Directives handled outside the option registry.
const GRAMMAR: &[&str] = &["syntax", "color", "icolor", "header"];

/// Parse `text` as a `.nanorc` layer named `origin`.
pub fn from_nanorc(origin: &str, text: &str) -> Layer {
    let mut layer = Layer::new(origin);

    for raw_line in text.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let mut words = line.split_whitespace();
        let directive = words.next().unwrap_or_default();

        match directive {
            "set" => set(
                &mut layer,
                words.next(),
                words.collect::<Vec<_>>().join(" "),
            ),
            "unset" => unset(&mut layer, words.next()),
            // Grammar blocks are phase 7; accepting them keeps a shared
            // ~/.nanorc quiet.
            directive if GRAMMAR.contains(&directive) => {}
            "include" => layer.warn("`include` is never followed"),
            other => layer.warn(format!("unknown directive `{other}`")),
        }
    }

    layer
}

/// `set <name> [value]`. A bare boolean name means "on", as in nano.
fn set(layer: &mut Layer, name: Option<&str>, value: String) {
    let Some(name) = name else {
        layer.warn("`set` without a name");
        return;
    };
    let Some(row) = by_nanorc(name) else {
        layer.warn(format!("unknown directive `{name}`"));
        return;
    };
    let raw = if value.is_empty() {
        "on".to_string()
    } else {
        unquote(&value)
    };
    match row.parse(&raw) {
        Ok(value) => layer.values.push((row.id, value)),
        Err(err) => layer.warn(format!("`{name}` {err}")),
    }
}

/// `unset <name>`: only booleans can be turned off; an integer has no "off".
fn unset(layer: &mut Layer, name: Option<&str>) {
    let Some(name) = name else {
        layer.warn("`unset` without a name");
        return;
    };
    let Some(row) = by_nanorc(name) else {
        layer.warn(format!("unknown directive `{name}`"));
        return;
    };
    match row.kind {
        super::Kind::Bool => layer.values.push((row.id, super::Value::Bool(false))),
        super::Kind::Int { .. } => layer.warn(format!("`{name}` takes a value; use `set`")),
    }
}

/// `#` starts a comment when it begins a word, as in nano.
fn strip_comment(line: &str) -> &str {
    let mut previous_is_space = true;
    for (index, ch) in line.char_indices() {
        if ch == '#' && previous_is_space {
            return &line[..index];
        }
        previous_is_space = ch.is_whitespace();
    }
    line
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::from_nanorc;
    use crate::config::{Opt, Value};

    #[test]
    fn maps_nano_directives_to_registry_rows() {
        let layer = from_nanorc("~/.nanorc", "set tabsize 4\nset casesensitive\n");
        assert!(layer.warnings.is_empty(), "{:?}", layer.warnings);
        assert_eq!(
            layer.values,
            vec![
                (Opt::TabSize, Value::Int(4)),
                (Opt::CaseSensitive, Value::Bool(true)),
            ]
        );
    }

    #[test]
    fn unset_turns_a_boolean_off() {
        let layer = from_nanorc("~/.nanorc", "set casesensitive\nunset casesensitive\n");
        assert_eq!(
            layer.values.last(),
            Some(&(Opt::CaseSensitive, Value::Bool(false)))
        );
    }

    #[test]
    fn unset_on_an_integer_warns() {
        let layer = from_nanorc("~/.nanorc", "unset tabsize\n");
        assert!(layer.values.is_empty());
        assert!(layer.warnings[0].contains("takes a value"));
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let layer = from_nanorc(
            "~/.nanorc",
            "# a comment\n\n  set tabsize 2   # trailing\n\t\n",
        );
        assert_eq!(layer.values, vec![(Opt::TabSize, Value::Int(2))]);
        assert!(layer.warnings.is_empty(), "{:?}", layer.warnings);
    }

    #[test]
    fn a_hash_inside_a_value_is_not_a_comment() {
        let layer = from_nanorc("~/.nanorc", "set casesensitive\n");
        assert_eq!(layer.values, vec![(Opt::CaseSensitive, Value::Bool(true))]);
        // `#` only starts a comment at a word boundary.
        assert_eq!(super::strip_comment("set tabsize 4#x"), "set tabsize 4#x");
    }

    #[test]
    fn grammar_blocks_are_accepted_without_a_warning() {
        let layer = from_nanorc(
            "~/.nanorc",
            "syntax \"rust\" \"\\.rs$\"\ncolor red \"\\bif\\b\"\nicolor green \"x\"\nheader \"^#!\"\n",
        );
        assert!(layer.values.is_empty());
        assert!(layer.warnings.is_empty(), "{:?}", layer.warnings);
    }

    #[test]
    fn include_is_never_followed() {
        let layer = from_nanorc("~/.nanorc", "include /etc/nano/extra.nanorc\n");
        assert_eq!(layer.warnings.len(), 1);
        assert!(layer.warnings[0].contains("`include` is never followed"));
    }

    #[test]
    fn unknown_directives_and_bad_values_warn_once() {
        let layer = from_nanorc("~/.nanorc", "set mouse\nset tabsize huge\nset tabsize 0\n");
        let warnings = layer.warnings.join("\n");
        assert!(warnings.contains("unknown directive `mouse`"), "{warnings}");
        assert!(
            warnings.contains("`tabsize` must be an integer"),
            "{warnings}"
        );
        assert!(warnings.contains("between 1 and 64"), "{warnings}");
        assert!(layer.values.is_empty());
    }

    #[test]
    fn quoted_values_are_unquoted() {
        let layer = from_nanorc("~/.nanorc", "set tabsize \"3\"\n");
        assert_eq!(layer.values, vec![(Opt::TabSize, Value::Int(3))]);
    }
}
