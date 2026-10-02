//! `platctl-logs` – prints the last lines of platform log files, optionally for one service.
//!
//! Step 001 – command-line conventions without any library:
//! - arguments parsed by hand (`args.rs`) following POSIX/GNU rules,
//! - `--help` / `--version` on stdout, exit code 0,
//! - data on stdout, diagnostics on stderr,
//! - exit codes: 0 success, 1 runtime failure, 2 usage error.

mod args;

use std::{
    collections::VecDeque,
    fs::File,
    io::{self, BufRead, BufReader, Write},
    process::ExitCode,
};

use args::{Command, Input, Mode, Options};

/// Program name used in messages. Conventionally the name the binary was invoked with.
const NAME: &str = "platctl-logs";

// Help follows the docopt/GNU layout: usage line(s), description, options with short and long
// form, defaults in the description. Users scan it – keep it aligned and short.
const HELP: &str = "\
Print the last lines of platform log files.

Usage: platctl-logs [OPTIONS] [FILE]...

With no FILE, or when FILE is -, read standard input.

Options:
  -n, --lines <N>        Number of lines to print [default: 10]
  -s, --service <NAME>   Only lines containing service=<NAME>
  -q, --quiet            Never print headers with file names
  -v, --verbose          More diagnostics on stderr (repeatable: -vv)
  -h, --help             Print help
  -V, --version          Print version
";

// `main` returning `ExitCode` – the exit status is part of a CLI's contract with scripts
// (`if platctl-logs ...; then`, `set -e`, CI steps).
fn main() -> ExitCode {
    // GNU tools honour POSIXLY_CORRECT: stop option parsing at the first positional argument.
    let mode = if std::env::var_os("POSIXLY_CORRECT").is_some() {
        Mode::Posix
    } else {
        Mode::Gnu
    };

    // `skip(1)` – the first element is the program path.
    let command = match args::parse(std::env::args_os().skip(1), mode) {
        Ok(command) => command,
        Err(error) => {
            // Usage error: short message + hint on STDERR, exit code 2 (POSIX utilities and
            // shells use 2 for "incorrect usage"). Don't dump the whole help – it hides the error.
            eprintln!("{NAME}: {error}");
            eprintln!("Try '{NAME} --help' for more information.");
            return ExitCode::from(2);
        }
    };

    match command {
        // Requested help/version is normal output: STDOUT, exit code 0
        // (so `platctl-logs --help | less` works).
        Command::Help => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Command::Version => {
            // `env!` reads a variable at COMPILE time; Cargo sets CARGO_PKG_VERSION from Cargo.toml.
            println!("{NAME} {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Run(options) => run(options),
    }
}

fn run(mut options: Options) -> ExitCode {
    if options.inputs.is_empty() {
        options.inputs.push(Input::Stdin);
    }
    let show_headers = options.inputs.len() > 1 && !options.quiet;

    // Lock stdout once – avoids re-locking per line and keeps output from different files intact.
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut failed = false;

    for (index, input) in options.inputs.iter().enumerate() {
        let name = match input {
            Input::Stdin => "standard input".to_string(),
            Input::File(path) => path.display().to_string(),
        };
        if options.verbose > 0 {
            // Diagnostics go to STDERR: they never pollute data piped to another program.
            eprintln!("{NAME}: reading {name}");
        }

        let lines = match input {
            Input::Stdin => tail(io::stdin().lock(), &options),
            Input::File(path) => File::open(path).and_then(|f| tail(BufReader::new(f), &options)),
        };

        match lines {
            Ok(lines) => {
                if show_headers {
                    // Same format as `tail`: blank line between files, header with the name.
                    let separator = if index > 0 { "\n" } else { "" };
                    let _ = writeln!(out, "{separator}==> {name} <==");
                }
                for line in lines {
                    // A failed write (e.g. closed pipe: `platctl-logs big.log | head -1`) ends the
                    // program quietly – details in step 013.
                    if writeln!(out, "{line}").is_err() {
                        return ExitCode::SUCCESS;
                    }
                }
            }
            Err(error) => {
                // Like `cat`/`tail`: report, continue with the remaining files, fail at the end.
                eprintln!("{NAME}: {name}: {}", describe(&error));
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::FAILURE // 1 – runtime error
    } else {
        ExitCode::SUCCESS
    }
}

/// `io::Error`'s `Display` ends with " (os error 2)" – noise for users. Standard tools print
/// only the OS message: `platctl-logs: nope.log: No such file or directory`.
fn describe(error: &io::Error) -> String {
    let text = error.to_string();
    match text.rfind(" (os error ") {
        Some(position) => text[..position].to_string(),
        None => text,
    }
}

/// Last `options.lines` lines matching the service filter. Streams the input and keeps only N
/// lines in memory – works for files of any size and for endless pipes.
fn tail(reader: impl BufRead, options: &Options) -> io::Result<VecDeque<String>> {
    let needle = options.service.as_ref().map(|s| format!("service={s}"));
    let mut last = VecDeque::with_capacity(options.lines);
    for line in reader.lines() {
        let line = line?;
        if needle.as_ref().is_some_and(|n| !line.contains(n.as_str())) {
            continue;
        }
        if options.lines == 0 {
            continue;
        }
        if last.len() == options.lines {
            last.pop_front();
        }
        last.push_back(line);
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
ts=1 service=billing msg=start
ts=2 service=search msg=start
ts=3 service=billing msg=ready
ts=4 service=billing msg=request
";

    #[test]
    fn keeps_last_lines_of_selected_service() {
        let options = Options {
            lines: 2,
            service: Some("billing".into()),
            ..Options::default()
        };

        let lines = tail(LOG.as_bytes(), &options).unwrap();

        assert_eq!(
            lines,
            [
                "ts=3 service=billing msg=ready",
                "ts=4 service=billing msg=request"
            ]
        );
    }
}
