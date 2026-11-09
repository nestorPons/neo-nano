//! The man page and the shell completions are written by hand, because the
//! registry generates the CLI but there is no completion generator in the
//! dependency set (and adding one would be weight for a six-option command).
//!
//! That means four files can drift from one table. These tests are the drift
//! guard: every registry flag, every language code and the two universal flags
//! must be in each file, and the man page header must carry the crate version.

use nnano_core::config::{Kind, OPTIONS};
use nnano_core::i18n::CODES;

const MAN: &str = include_str!("../../../man/nnano.1");
const BASH: &str = include_str!("../../../completions/nnano.bash");
const ZSH: &str = include_str!("../../../completions/_nnano");
const FISH: &str = include_str!("../../../completions/nnano.fish");

#[test]
fn the_man_page_names_every_flag() {
    for row in OPTIONS {
        // roff writes a hyphen as `\-`, so a flag is `\-\-name` in the source.
        assert!(
            MAN.contains(&format!("\\-\\-{}", row.long)),
            "man/nnano.1 does not document --{}",
            row.long
        );
        if matches!(row.kind, Kind::Bool) {
            assert!(
                MAN.contains(&format!("\\-\\-no\\-{}", row.long)),
                "man/nnano.1 does not document --no-{}",
                row.long
            );
        }
    }
    for flag in ["\\-h", "\\-\\-help", "\\-V", "\\-\\-version"] {
        assert!(MAN.contains(flag), "man/nnano.1 is missing {flag}");
    }
}

#[test]
fn the_completions_name_every_flag() {
    for row in OPTIONS {
        assert!(
            BASH.contains(&format!("--{}", row.long)),
            "completions/nnano.bash does not offer --{}",
            row.long
        );
        assert!(
            ZSH.contains(&format!("--{}", row.long)),
            "completions/_nnano does not offer --{}",
            row.long
        );
        assert!(
            FISH.contains(&format!("-l {}", row.long)),
            "completions/nnano.fish does not offer --{}",
            row.long
        );
        if matches!(row.kind, Kind::Bool) {
            for (file, name) in [
                (BASH, "completions/nnano.bash"),
                (ZSH, "completions/_nnano"),
                (FISH, "completions/nnano.fish"),
            ] {
                assert!(
                    file.contains(&format!("no-{}", row.long)),
                    "{name} does not offer --no-{}",
                    row.long
                );
            }
        }
    }
    for (file, name) in [
        (BASH, "completions/nnano.bash"),
        (ZSH, "completions/_nnano"),
        (FISH, "completions/nnano.fish"),
    ] {
        assert!(
            file.contains("--help") || file.contains("-l help"),
            "{name} does not offer --help"
        );
    }
}

#[test]
fn the_completions_offer_every_language() {
    for code in CODES {
        for (file, name) in [
            (BASH, "completions/nnano.bash"),
            (ZSH, "completions/_nnano"),
            (FISH, "completions/nnano.fish"),
        ] {
            assert!(file.contains(code), "{name} does not offer `{code}`");
        }
        assert!(MAN.contains(code), "man/nnano.1 does not list `{code}`");
    }
}

#[test]
fn the_man_page_header_matches_the_crate_version() {
    let expected = format!("\"nnano {}\"", env!("CARGO_PKG_VERSION"));
    assert!(
        MAN.contains(&expected),
        "man/nnano.1 header must say {expected}"
    );
}
