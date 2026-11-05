//! `nnano` binary: args -> config -> terminal init -> `App::run`.
//!
//! The clap command is built with the *builder* API on purpose: every flag will
//! come from `nnano_core::config`'s option registry, so no `#[derive(Parser)]`
//! struct may be added here.

use std::process::ExitCode;

use clap::{Arg, Command, value_parser};

fn main() -> ExitCode {
    let matches = command().get_matches();
    let file = matches.get_one::<String>("file").cloned();

    match nnano_tui::run(file) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("nnano: {err}");
            ExitCode::FAILURE
        }
    }
}

/// CLI surface. Options are absent until the option registry exists; only the
/// invocation shape is fixed here (`nnano [FILE]`, `-` for stdin).
fn command() -> Command {
    Command::new("nnano")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A modern, nano-compatible terminal text editor")
        .arg(
            Arg::new("file")
                .value_name("FILE")
                .help("File to open; \"-\" reads standard input")
                .value_parser(value_parser!(String)),
        )
}

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn accepts_a_file_argument() {
        let matches = command()
            .try_get_matches_from(["nnano", "notes.txt"])
            .unwrap();
        assert_eq!(
            matches.get_one::<String>("file").map(String::as_str),
            Some("notes.txt")
        );
    }

    #[test]
    fn accepts_dash_as_stdin() {
        let matches = command().try_get_matches_from(["nnano", "-"]).unwrap();
        assert_eq!(
            matches.get_one::<String>("file").map(String::as_str),
            Some("-")
        );
    }

    #[test]
    fn runs_without_arguments() {
        assert!(command().try_get_matches_from(["nnano"]).is_ok());
    }
}
