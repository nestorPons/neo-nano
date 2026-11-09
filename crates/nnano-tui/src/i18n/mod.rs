//! Interface translations: one table, every user-visible string.
//!
//! Lookup is keyed by the **English source string**, gettext-style: a call site
//! reads `t(lang, "Save in place")` and the table holds one column per language.
//! English is column 0 and the fallback, so a missing row degrades to English
//! instead of failing — and `every_surface_has_every_translation` fails the build
//! before a missing row can ship.
//!
//! Columns are ordered by [`Lang::ALL`] and indexed by [`Lang::index`].
//!
//! Translating a string means editing one row here. Adding a language means one
//! [`Lang`] variant, one code in `nnano-core::i18n::CODES` and one column in
//! every row — the compiler counts the columns, so a forgotten one does not
//! build. Rows whose translation must stay English pass the English text again
//! (key names like `Ctrl+S` are never translated).

mod chrome;
mod keys;

pub use nnano_core::i18n::Lang;

/// One translated string, with one column per language.
pub struct Entry {
    /// The English source string, i.e. the key every call site passes to [`t`].
    /// For the two strings that are the same word in two places with two
    /// meanings, this is prefixed (`settings.search`) and column 0 is the text.
    pub en: &'static str,
    /// One text per language; column 0 is English and doubles as the fallback.
    pub texts: [&'static str; Lang::COUNT],
}

macro_rules! entry {
    ($en:expr => [$($text:expr),* $(,)?]) => {
        Entry { en: $en, texts: [$($text),*] }
    };
}

pub(crate) use entry;

/// Every table. Lookup is linear over ~100 rows, which is far cheaper than any
/// index and keeps adding a string a one-line change.
const TABLES: &[&[Entry]] = &[chrome::CHROME, keys::KEYS];

fn lookup(english: &str) -> Option<&'static Entry> {
    TABLES
        .iter()
        .flat_map(|table| table.iter())
        .find(|entry| entry.en == english)
}

/// `english` in `lang`. An unknown key returns `english` unchanged: a new string
/// shows in English instead of breaking the interface.
pub fn t(lang: Lang, english: &'static str) -> &'static str {
    lookup(english).map_or(english, |entry| entry.texts[lang.index()])
}

/// [`t`] with `{}` placeholders filled in order. A translation with a missing
/// placeholder is a table bug, caught by `placeholders_match_the_english`.
pub fn tf(lang: Lang, english: &'static str, args: &[&str]) -> String {
    let mut text = t(lang, english).to_string();
    for arg in args {
        match text.find("{}") {
            Some(at) => text.replace_range(at..at + 2, arg),
            None => break,
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{Lang, TABLES, lookup, t, tf};

    /// Every string a surface can print, collected from the places that own
    /// them. A new binding, command or option that forgets a row fails here.
    fn required_strings() -> Vec<&'static str> {
        use crate::term::input::{GROUPS, KEYMAP};
        use crate::ui::overlays::help::notes;
        use crate::ui::overlays::palette::COMMANDS;
        use nnano_core::config::OPTIONS;

        let mut strings: Vec<&'static str> = Vec::new();
        strings.extend(KEYMAP.iter().map(|binding| binding.label));
        strings.extend(COMMANDS.iter().map(|command| command.label));
        strings.extend(GROUPS.iter().map(|group| group.title()));
        for row in OPTIONS {
            strings.push(row.group);
            strings.push(row.label);
            strings.push(row.help);
        }
        for (title, rows) in notes() {
            strings.push(title);
            strings.extend(rows.iter().map(|(_, label)| *label));
        }
        strings
    }

    #[test]
    fn every_surface_has_every_translation() {
        for english in required_strings() {
            let entry =
                lookup(english).unwrap_or_else(|| panic!("`{english}` has no translation row"));
            for lang in Lang::all() {
                assert!(
                    !entry.texts[lang.index()].is_empty(),
                    "`{english}` has an empty {} column",
                    lang.code()
                );
            }
        }
    }

    #[test]
    fn every_row_is_unique_and_starts_in_english() {
        let mut seen = std::collections::HashSet::new();
        for entry in TABLES.iter().flat_map(|table| table.iter()) {
            assert!(seen.insert(entry.en), "`{}` is declared twice", entry.en);
            assert!(!entry.texts[0].is_empty(), "`{}` has no English", entry.en);
        }
    }

    #[test]
    fn placeholders_match_the_english() {
        for entry in TABLES.iter().flat_map(|table| table.iter()) {
            let expected = entry.texts[0].matches("{}").count();
            for lang in Lang::all() {
                let text = entry.texts[lang.index()];
                assert_eq!(
                    text.matches("{}").count(),
                    expected,
                    "`{}` has the wrong number of `{{}}` in {}",
                    entry.en,
                    lang.code()
                );
            }
        }
    }

    #[test]
    fn translations_are_not_english_copies() {
        // Spot checks that a column was actually translated. Every language must
        // differ on the strings a user meets first.
        for lang in Lang::all().filter(|lang| *lang != Lang::En) {
            for english in ["Settings", "Help", "Save in place", "Quit"] {
                assert_ne!(
                    t(lang, english),
                    english,
                    "{} still says `{english}`",
                    lang.code()
                );
            }
        }
    }

    #[test]
    fn text_is_the_english_column_and_unknown_keys_pass_through() {
        assert_eq!(t(Lang::En, "Save in place"), "Save in place");
        assert_eq!(t(Lang::Es, "Save in place"), "Guardar en el mismo archivo");
        assert_eq!(
            t(Lang::Es, "not a declared string"),
            "not a declared string"
        );
    }

    #[test]
    fn placeholders_are_filled_in_order() {
        assert_eq!(
            tf(Lang::En, "saved {}", &["/tmp/x"]),
            "saved /tmp/x".to_string()
        );
        assert_eq!(
            tf(Lang::En, "not found: {}", &["x"]),
            "not found: x".to_string()
        );
        // A chip with no placeholder is harmless.
        assert_eq!(tf(Lang::En, "Help", &["x"]), "Help".to_string());
    }
}
