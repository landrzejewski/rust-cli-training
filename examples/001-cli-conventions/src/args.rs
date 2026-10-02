//! A hand-written command-line parser following POSIX and GNU conventions.
//!
//! Real tools use a library (clap – from step 002). Writing it once by hand shows what such a
//! library does for us and which conventions users expect:
//!
//! | Syntax            | Meaning                                                    |
//! |-------------------|------------------------------------------------------------|
//! | `-v`              | short option (one letter)                                  |
//! | `-vq`             | bundled short flags = `-v -q`                              |
//! | `-n 5`, `-n5`     | short option with a value (separate or attached)           |
//! | `--lines 5`       | long option (GNU) with a value                             |
//! | `--lines=5`       | long option with an attached value                         |
//! | `--`              | end of options: everything after it is positional          |
//! | `-`               | positional meaning "standard input"                        |
//! | `file -n 5`       | GNU: options may follow positionals (argument permutation) |
//!
//! The parser is a pure function (arguments in, `Command` or error out) – easy to unit-test,
//! no printing, no `process::exit` inside.

use std::{ffi::OsString, path::PathBuf};

/// What the user asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Run(Options),
}

/// Options of `platctl-logs` with their defaults.
#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    /// `-n`, `--lines`: how many trailing lines to print.
    pub lines: usize,
    /// `-s`, `--service`: keep only lines containing `service=<name>`.
    pub service: Option<String>,
    /// `-v`, `--verbose`: repeatable (`-vv`) – verbosity level.
    pub verbose: u8,
    /// `-q`, `--quiet`: no headers between files.
    pub quiet: bool,
    /// Positional arguments: files, `-` = stdin. Empty = stdin.
    pub inputs: Vec<Input>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            lines: 10,
            service: None,
            verbose: 0,
            quiet: false,
            inputs: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Input {
    Stdin,
    // `PathBuf`, not `String`: file names on Unix are arbitrary bytes and need not be UTF-8.
    File(PathBuf),
}

/// Usage errors – the user typed something we don't understand. Reported with exit code 2.
#[derive(Debug, PartialEq, Eq)]
pub enum ArgError {
    UnknownOption(String),
    MissingValue(String),
    InvalidValue { option: String, value: String },
    NotUnicode(String),
}

impl std::fmt::Display for ArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownOption(option) => write!(f, "unknown option '{option}'"),
            Self::MissingValue(option) => write!(f, "option '{option}' requires a value"),
            Self::InvalidValue { option, value } => {
                write!(f, "invalid value '{value}' for '{option}'")
            }
            Self::NotUnicode(option) => write!(f, "value of '{option}' is not valid UTF-8"),
        }
    }
}

impl std::error::Error for ArgError {}

/// Parser mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// GNU: options and positionals can be mixed (`file.log -n 5`).
    Gnu,
    /// POSIX: the first positional ends option parsing (`file.log -n 5` -> `-n` is a file name).
    /// GNU tools switch to this mode when `POSIXLY_CORRECT` is set.
    Posix,
}

/// Parses arguments WITHOUT the program name (`std::env::args_os().skip(1)`).
///
/// `OsString` instead of `String`: `std::env::args()` panics on non-UTF-8 arguments;
/// `args_os()` never does. Option names are ASCII, so they can be compared safely;
/// values are converted only where text is required.
pub fn parse<I>(args: I, mode: Mode) -> Result<Command, ArgError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut options = Options::default();
    // A peekable iterator lets an option "steal" the next argument as its value (`-n 5`).
    let mut args = args.into_iter().peekable();
    let mut only_positionals = false;

    while let Some(arg) = args.next() {
        if only_positionals {
            options.inputs.push(positional(arg));
            continue;
        }

        // Non-UTF-8 arguments can only be positionals (file names).
        let Some(text) = arg.to_str() else {
            options.inputs.push(positional(arg));
            if mode == Mode::Posix {
                only_positionals = true;
            }
            continue;
        };

        if text == "--" {
            // End of options. `platctl-logs -- -weird-name.log` reads a file named `-weird-name.log`.
            only_positionals = true;
        } else if let Some(long) = text.strip_prefix("--") {
            // `--name` or `--name=value`. `split_once` separates the attached value.
            let (name, attached) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (long, None),
            };
            match name {
                "help" => return Ok(Command::Help),
                "version" => return Ok(Command::Version),
                "verbose" => options.verbose = options.verbose.saturating_add(1),
                "quiet" => options.quiet = true,
                "lines" => {
                    let value = value(&format!("--{name}"), attached, &mut args)?;
                    options.lines = parse_number(&format!("--{name}"), &value)?;
                }
                "service" => {
                    options.service = Some(value(&format!("--{name}"), attached, &mut args)?);
                }
                _ => return Err(ArgError::UnknownOption(format!("--{name}"))),
            }
        } else if text.len() > 1 && text.starts_with('-') {
            // One or more bundled short options: `-v`, `-vq`, `-vvn5`.
            for (index, letter) in text[1..].char_indices() {
                match letter {
                    'h' => return Ok(Command::Help),
                    'V' => return Ok(Command::Version),
                    'v' => options.verbose = options.verbose.saturating_add(1),
                    'q' => options.quiet = true,
                    'n' | 's' => {
                        // The rest of the bundle is the value (`-n5`), otherwise the next arg.
                        let rest = &text[1 + index + letter.len_utf8()..];
                        let attached = (!rest.is_empty()).then(|| rest.to_string());
                        let option = format!("-{letter}");
                        let value = value(&option, attached, &mut args)?;
                        if letter == 'n' {
                            options.lines = parse_number(&option, &value)?;
                        } else {
                            options.service = Some(value);
                        }
                        break; // the value consumed the rest of this argument
                    }
                    other => return Err(ArgError::UnknownOption(format!("-{other}"))),
                }
            }
        } else {
            // A positional (also the single `-`).
            options.inputs.push(positional(arg));
            if mode == Mode::Posix {
                only_positionals = true;
            }
        }
    }

    Ok(Command::Run(options))
}

fn positional(arg: OsString) -> Input {
    if arg == "-" {
        Input::Stdin
    } else {
        Input::File(PathBuf::from(arg))
    }
}

/// Value of an option: attached (`--lines=5`, `-n5`) or the next argument (`--lines 5`).
fn value<I>(
    option: &str,
    attached: Option<String>,
    args: &mut std::iter::Peekable<I>,
) -> Result<String, ArgError>
where
    I: Iterator<Item = OsString>,
{
    if let Some(value) = attached {
        return Ok(value);
    }
    let next = args
        .next()
        .ok_or_else(|| ArgError::MissingValue(option.to_string()))?;
    next.into_string()
        .map_err(|_| ArgError::NotUnicode(option.to_string()))
}

fn parse_number(option: &str, value: &str) -> Result<usize, ArgError> {
    value.parse().map_err(|_| ArgError::InvalidValue {
        option: option.to_string(),
        value: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str], mode: Mode) -> Result<Command, ArgError> {
        parse(args.iter().map(OsString::from), mode)
    }

    fn options(args: &[&str]) -> Options {
        match run(args, Mode::Gnu) {
            Ok(Command::Run(options)) => options,
            other => panic!("expected options, got {other:?}"),
        }
    }

    #[test]
    fn defaults_read_stdin() {
        assert_eq!(options(&[]), Options::default());
    }

    #[test]
    fn short_long_attached_and_separate_values_are_equivalent() {
        for args in [
            &["-n", "3"][..],
            &["-n3"],
            &["--lines", "3"],
            &["--lines=3"],
        ] {
            assert_eq!(options(args).lines, 3, "{args:?}");
        }
    }

    #[test]
    fn bundled_short_flags() {
        let parsed = options(&["-vvq", "-s", "billing"]);

        assert_eq!(parsed.verbose, 2);
        assert!(parsed.quiet);
        assert_eq!(parsed.service.as_deref(), Some("billing"));
    }

    #[test]
    fn value_can_end_a_bundle() {
        let parsed = options(&["-vn5"]);

        assert_eq!((parsed.verbose, parsed.lines), (1, 5));
    }

    #[test]
    fn dash_is_stdin_and_double_dash_ends_options() {
        let parsed = options(&["a.log", "-", "--", "-n", "--help"]);

        assert_eq!(
            parsed.inputs,
            vec![
                Input::File("a.log".into()),
                Input::Stdin,
                Input::File("-n".into()),
                Input::File("--help".into()),
            ]
        );
        assert_eq!(parsed.lines, 10, "-n after -- is a file name");
    }

    #[test]
    fn gnu_permutes_but_posix_stops_at_first_positional() {
        let gnu = options(&["a.log", "-n", "2"]);
        let posix = run(&["a.log", "-n", "2"], Mode::Posix);

        assert_eq!(gnu.lines, 2);
        assert!(matches!(posix, Ok(Command::Run(o)) if o.inputs.len() == 3 && o.lines == 10));
    }

    #[test]
    fn help_and_version_win() {
        assert_eq!(run(&["-n", "2", "--help"], Mode::Gnu), Ok(Command::Help));
        assert_eq!(run(&["-V"], Mode::Gnu), Ok(Command::Version));
    }

    #[test]
    fn usage_errors() {
        assert_eq!(
            run(&["--follow"], Mode::Gnu),
            Err(ArgError::UnknownOption("--follow".into()))
        );
        assert_eq!(
            run(&["-n"], Mode::Gnu),
            Err(ArgError::MissingValue("-n".into()))
        );
        assert!(matches!(
            run(&["--lines=many"], Mode::Gnu),
            Err(ArgError::InvalidValue { .. })
        ));
    }
}
