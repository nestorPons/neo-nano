//! Interface languages: the codes the `language` option accepts and the
//! system-locale detection behind `auto`.
//!
//! The translations themselves live in `nnano-tui::i18n`: this crate only owns
//! the *identity* of a language, because the option registry (which is here) has
//! to validate and print the codes. That keeps the dependency direction intact:
//! `nnano-core` never renders a translated string.

use std::fmt;

/// One interface language. The order of the variants is the order of the
/// translation columns in `nnano-tui::i18n`, so it is load-bearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Lang {
    #[default]
    En,
    ZhHans,
    Hi,
    Es,
    Ar,
    Fr,
    Bn,
    Pt,
    Id,
    Ja,
}

impl Lang {
    /// Every language, in column order. Index `n` of this slice is index `n` of
    /// every translation column.
    pub const ALL: [Lang; Self::COUNT] = [
        Lang::En,
        Lang::ZhHans,
        Lang::Hi,
        Lang::Es,
        Lang::Ar,
        Lang::Fr,
        Lang::Bn,
        Lang::Pt,
        Lang::Id,
        Lang::Ja,
    ];

    /// Number of translated columns; also the length of a translation array.
    pub const COUNT: usize = 10;

    /// Column index of this language.
    pub fn index(self) -> usize {
        match self {
            Lang::En => 0,
            Lang::ZhHans => 1,
            Lang::Hi => 2,
            Lang::Es => 3,
            Lang::Ar => 4,
            Lang::Fr => 5,
            Lang::Bn => 6,
            Lang::Pt => 7,
            Lang::Id => 8,
            Lang::Ja => 9,
        }
    }

    /// The code written in `config.toml`, `--language` and `.nanorc`-free
    /// settings. ASCII on purpose: it has to survive a terminal without the
    /// language's script.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::ZhHans => "zh-Hans",
            Lang::Hi => "hi",
            Lang::Es => "es",
            Lang::Ar => "ar",
            Lang::Fr => "fr",
            Lang::Bn => "bn",
            Lang::Pt => "pt",
            Lang::Id => "id",
            Lang::Ja => "ja",
        }
    }

    /// Name of the language **in that language**, as a language picker should
    /// show it. Never translated: a user looking for their language recognises
    /// it written in it.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::ZhHans => "简体中文",
            Lang::Hi => "हिन्दी",
            Lang::Es => "Español",
            Lang::Ar => "العربية",
            Lang::Fr => "Français",
            Lang::Bn => "বাংলা",
            Lang::Pt => "Português",
            Lang::Id => "Bahasa Indonesia",
            Lang::Ja => "日本語",
        }
    }

    /// Resolve a code or a locale string: `es`, `es_ES`, `es-ES.UTF-8`,
    /// `zh-Hans`, `zh_CN`, `in` (legacy Indonesian) all work.
    pub fn from_code(raw: &str) -> Option<Self> {
        let lower = raw.trim().to_ascii_lowercase();
        let base = lower.split(['.', '@']).next().unwrap_or_default();
        let head = base.split(['_', '-']).next().unwrap_or_default();
        Some(match head {
            // The POSIX locale is ASCII English; `LANG=C` must land here too.
            "c" | "posix" => Lang::En,
            "en" => Lang::En,
            "zh" => Lang::ZhHans,
            "hi" => Lang::Hi,
            "es" => Lang::Es,
            "ar" => Lang::Ar,
            "fr" => Lang::Fr,
            "bn" => Lang::Bn,
            "pt" => Lang::Pt,
            "id" | "in" => Lang::Id,
            "ja" => Lang::Ja,
            _ => return None,
        })
    }

    /// `auto` resolves through the environment; anything unknown falls back to
    /// English rather than failing.
    pub fn resolve(code: &str) -> Self {
        if code.eq_ignore_ascii_case("auto") {
            Self::detect()
        } else {
            Self::from_code(code).unwrap_or_default()
        }
    }

    /// First of `LC_ALL`, `LC_MESSAGES`, `LANG` that names a known language.
    pub fn detect() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find_map(|value| Self::from_code(&value))
            .unwrap_or_default()
    }

    /// The languages, in picker order.
    pub fn all() -> impl Iterator<Item = Lang> {
        Self::ALL.into_iter()
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.native_name())
    }
}

/// Every value the `language` option accepts, in picker order. `auto` is not a
/// language: it means "follow the system locale".
pub const CODES: &[&str] = &[
    "auto", "en", "zh-Hans", "hi", "es", "ar", "fr", "bn", "pt", "id", "ja",
];

#[cfg(test)]
mod tests {
    use super::{CODES, Lang};

    #[test]
    fn every_language_has_a_column_and_a_code_in_the_registry() {
        assert_eq!(Lang::ALL.len(), Lang::COUNT);
        for (index, lang) in Lang::all().enumerate() {
            assert_eq!(lang.index(), index, "{lang:?}");
            assert!(CODES.contains(&lang.code()), "{lang:?}");
        }
        // `auto` plus one code per language, no duplicates.
        assert_eq!(CODES.len(), Lang::COUNT + 1);
        let mut codes: Vec<&str> = CODES.to_vec();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), CODES.len());
    }

    #[test]
    fn codes_round_trip_through_from_code() {
        for lang in Lang::all() {
            assert_eq!(Lang::from_code(lang.code()), Some(lang), "{lang:?}");
        }
    }

    #[test]
    fn locale_strings_resolve_to_their_language() {
        for (raw, expected) in [
            ("es_ES.UTF-8", Lang::Es),
            ("es-ES", Lang::Es),
            ("zh_CN.GB18030", Lang::ZhHans),
            ("pt_BR", Lang::Pt),
            ("bn_BD", Lang::Bn),
            ("id_ID.UTF-8", Lang::Id),
            ("in_ID", Lang::Id),
            ("ja_JP", Lang::Ja),
            ("hi_IN", Lang::Hi),
            ("ar_SA", Lang::Ar),
            ("fr_FR", Lang::Fr),
            ("C", Lang::En),
            ("en_US.UTF-8", Lang::En),
        ] {
            assert_eq!(Lang::from_code(raw), Some(expected), "{raw}");
        }
        assert_eq!(Lang::from_code("kl_GL"), None);
        assert_eq!(Lang::from_code(""), None);
    }

    #[test]
    fn auto_is_resolved_and_unknown_codes_fall_back_to_english() {
        // `auto` reads the environment; whatever it finds must be a real
        // language, and an unknown code must never panic or invent one.
        assert!(Lang::ALL.contains(&Lang::resolve("auto")));
        assert_eq!(Lang::resolve("xx"), Lang::En);
        assert_eq!(Lang::resolve("es"), Lang::Es);
    }

    #[test]
    fn native_names_are_not_english_placeholders() {
        for lang in Lang::all() {
            assert!(!lang.native_name().is_empty(), "{lang:?}");
        }
        assert_eq!(Lang::ZhHans.native_name(), "简体中文");
    }
}
