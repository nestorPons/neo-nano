//! `nnano` binary: args -> config -> terminal init -> `App::run`.
//!
//! The command is built with the *builder* API and every flag comes from
//! `nnano_core::config::OPTIONS`: adding an option is one registry row, never a
//! `#[derive(Parser)]` struct that could drift from the file readers or the
//! settings overlay.

use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use nnano_core::config::{self, Layer, OPTIONS, OptRow, Value};

fn main() -> ExitCode {
    let matches = command().get_matches();
    let file = matches.get_one::<String>("file").cloned();

    let config = config::Paths::detect().load(cli_layer(&matches));
    for warning in config.warnings() {
        eprintln!("nnano: {warning}");
    }

    // Distro grammars are inherited from nano (/usr/share/nano), so a system with
    // nano installed highlights without any nnano setup.
    let (syntax, syntax_warnings) =
        nnano_core::syntax::SyntaxSet::load(&nnano_core::syntax::detect_dirs());
    for warning in syntax_warnings {
        eprintln!("nnano: {warning}");
    }

    match nnano_tui::run(file, config, syntax) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("nnano: {err}");
            ExitCode::FAILURE
        }
    }
}

/// CLI surface: the invocation shape plus one flag (or flag pair) per registry
/// row, generated at runtime.
fn command() -> Command {
    let mut command = Command::new("nnano")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A modern, nano-compatible terminal text editor")
        .arg(
            Arg::new("file")
                .value_name("FILE")
                .help("File to open; \"-\" reads standard input")
                .value_parser(value_parser!(String)),
        );

    for row in OPTIONS {
        command = command.args(row_args(row));
    }
    command
}

/// One row becomes one flag. A boolean becomes a pair (`--x` / `--no-x`) so the
/// CLI can express "off" and still beat a lower layer.
fn row_args(row: &OptRow) -> Vec<Arg> {
    match row.kind {
        config::Kind::Bool => {
            let negative = format!("no_{}", row.key);
            let positive = Arg::new(row.key)
                .long(row.long)
                .action(ArgAction::SetTrue)
                .help(row.help)
                .overrides_with(negative.clone());
            let positive = match row.short {
                Some(short) => positive.short(short),
                None => positive,
            };
            vec![
                positive,
                Arg::new(negative)
                    .long(format!("no-{}", row.long))
                    .action(ArgAction::SetTrue)
                    .help(format!("Turn `{}` off, overriding a lower layer", row.key))
                    .overrides_with(row.key),
            ]
        }
        config::Kind::Int { .. } => {
            let row = *row;
            let arg = Arg::new(row.key)
                .long(row.long)
                .value_name("N")
                .help(row.help)
                .value_parser(move |raw: &str| {
                    row.parse(raw)
                        .map(|value| value.as_int().unwrap_or_default())
                        .map_err(|err| format!("`--{}` {err}", row.long))
                });
            vec![match row.short {
                Some(short) => arg.short(short),
                None => arg,
            }]
        }
    }
}

/// The layer the command line produced. Only options actually typed appear, so
/// a lower layer still wins for the rest.
fn cli_layer(matches: &ArgMatches) -> Layer {
    let mut layer = Layer::new("command line");
    for row in OPTIONS {
        match row.kind {
            config::Kind::Bool => {
                let negative = format!("no_{}", row.key);
                if matches.get_flag(row.key) {
                    layer.values.push((row.id, Value::Bool(true)));
                } else if matches.get_flag(&negative) {
                    layer.values.push((row.id, Value::Bool(false)));
                }
            }
            config::Kind::Int { .. } => {
                if matches.value_source(row.key) == Some(clap::parser::ValueSource::CommandLine)
                    && let Some(value) = matches.get_one::<i64>(row.key)
                {
                    layer.values.push((row.id, Value::Int(*value)));
                }
            }
        }
    }
    layer
}

#[cfg(test)]
mod tests {
    use nnano_core::config::{Kind, OPTIONS, Opt};

    use super::{cli_layer, command};

    fn layer(args: &[&str]) -> super::Layer {
        let mut argv = vec!["nnano"];
        argv.extend_from_slice(args);
        let matches = command().try_get_matches_from(argv).unwrap();
        cli_layer(&matches)
    }

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
    fn runs_without_arguments_and_yields_no_cli_values() {
        let matches = command().try_get_matches_from(["nnano"]).unwrap();
        assert!(cli_layer(&matches).values.is_empty());
    }

    #[test]
    fn integers_accept_both_spellings_and_are_range_checked() {
        let long = layer(&["--tabsize", "4"]);
        assert_eq!(long.values, vec![(Opt::TabSize, super::Value::Int(4))]);
        let short = layer(&["-T", "4"]);
        assert_eq!(short.values, long.values);
        assert!(long.warnings.is_empty(), "{:?}", long.warnings);

        let error = command()
            .try_get_matches_from(["nnano", "--tabsize", "0"])
            .unwrap_err();
        assert!(error.to_string().contains("between 1 and 64"), "{error}");
    }

    #[test]
    fn booleans_have_a_positive_and_a_negative_flag() {
        assert_eq!(
            layer(&["--casesensitive"]).values,
            vec![(Opt::CaseSensitive, super::Value::Bool(true))]
        );
        assert_eq!(
            layer(&["-C"]).values,
            vec![(Opt::CaseSensitive, super::Value::Bool(true))]
        );
        assert_eq!(
            layer(&["--no-casesensitive"]).values,
            vec![(Opt::CaseSensitive, super::Value::Bool(false))]
        );
        assert_eq!(
            layer(&["--casesensitive", "--no-casesensitive"]).values,
            vec![(Opt::CaseSensitive, super::Value::Bool(false))]
        );
    }

    #[test]
    fn help_lists_every_registry_flag() {
        for row in OPTIONS {
            let help = command().render_long_help().to_string();
            assert!(help.contains(&format!("--{}", row.long)), "{help}");
            if let Some(short) = row.short {
                assert!(help.contains(&format!("-{short}")), "{help}");
            }
        }
    }

    /// The CLI half of the drift test: every row is reachable and parseable from
    /// the command line, with a value its own kind allows.
    #[test]
    fn every_registry_row_is_reachable_from_the_cli() {
        for row in OPTIONS {
            let args: Vec<String> = match row.kind {
                Kind::Bool => vec![format!("--{}", row.long)],
                Kind::Int { .. } => vec![format!("--{}", row.long), row.default.to_string()],
            };
            let mut argv = vec!["nnano".to_string()];
            argv.extend(args);
            let matches = command().try_get_matches_from(argv).unwrap();
            let layer = cli_layer(&matches);
            assert_eq!(layer.values.len(), 1, "{:?}", row.id);
            assert_eq!(layer.values[0].0, row.id);
        }
    }
}
