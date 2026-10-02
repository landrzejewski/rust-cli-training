//! `platctl-logq` – step 013: a well-behaved Unix filter.
//!
//! ```text
//! platctl-logq [--level L] [--service S] [--count | --json | --files-with-matches [-0]] [FILE]...
//! ```
//!
//! - reads files or stdin (`-` or no FILE), streams line by line – constant memory,
//! - buffered output, correct behavior in pipes (`| head`, `| grep`, `| jq`, `| xargs -0`),
//! - grep-like exit codes: 0 = something matched, 1 = nothing matched, 2 = error.

mod logfmt;

use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::Parser;

/// Filter platform logs (logfmt) by level and service.
#[derive(Debug, Parser)]
#[command(name = "platctl-logq", version)]
struct Cli {
    /// Only lines with this level (info, warn, error)
    // No short form: `-l` is reserved for `--files-with-matches`, as in grep.
    #[arg(long)]
    level: Option<String>,

    /// Only lines of this service
    #[arg(short, long)]
    service: Option<String>,

    /// Print the number of matching lines instead of the lines
    #[arg(short, long, conflicts_with_all = ["json", "files_with_matches"])]
    count: bool,

    /// Print matching lines as JSON Lines (for jq)
    #[arg(long, conflicts_with = "files_with_matches")]
    json: bool,

    /// Print only names of files with at least one match
    #[arg(short = 'l', long)]
    files_with_matches: bool,

    /// Separate file names with NUL instead of newline (for `xargs -0`)
    #[arg(short = '0', long, requires = "files_with_matches")]
    null: bool,

    /// Log files; `-` or none = standard input
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        // Downstream closed the pipe (`platctl-logq big.log | head -1`): not an error – the
        // consumer simply has enough. Exit quietly with success, like `cat`/`grep` do.
        Err(error) if is_broken_pipe(&error) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("platctl-logq: {error:#}");
            ExitCode::from(2)
        }
    }
}

/// Returns whether anything matched.
fn run(cli: &Cli) -> anyhow::Result<bool> {
    let inputs: Vec<Option<PathBuf>> = if cli.files.is_empty() {
        vec![None]
    } else {
        cli.files
            .iter()
            .map(|p| (p.as_os_str() != "-").then(|| p.clone()))
            .collect()
    };

    if inputs.contains(&None) && io::stdin().is_terminal() {
        // A human ran us without input – explain why we seem to hang (like `cat`, `wc`).
        eprintln!("platctl-logq: reading from standard input (Ctrl+D to end, Ctrl+C to abort)");
    }

    // Rust's `stdout()` is LINE-buffered: one `write` syscall per line – slow for big outputs
    // into pipes/files. `BufWriter` batches writes (8 KiB). Must be flushed at the end; errors
    // during flush (e.g. disk full) must be reported, not ignored.
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut any_match = false;
    let mut total = 0usize;

    for input in &inputs {
        let name = input.as_ref().map_or_else(
            || "(standard input)".to_string(),
            |p| p.display().to_string(),
        );
        // `Box<dyn BufRead>` – one code path for files and stdin.
        let reader: Box<dyn BufRead> = match input {
            None => Box::new(io::stdin().lock()),
            Some(path) => match File::open(path) {
                Ok(file) => Box::new(BufReader::new(file)),
                Err(error) => {
                    // Report and continue with the other files (step 001).
                    eprintln!("platctl-logq: {name}: {error}");
                    continue;
                }
            },
        };

        let matches = process(cli, reader, &mut out)?;
        total += matches;
        if matches > 0 {
            any_match = true;
            if cli.files_with_matches {
                // NUL can't appear in file names – the only fully safe separator.
                let separator = if cli.null { b'\0' } else { b'\n' };
                out.write_all(name.as_bytes())?;
                out.write_all(&[separator])?;
            }
        }
    }

    if cli.count {
        writeln!(out, "{total}")?;
    }
    out.flush()?;
    Ok(any_match)
}

/// Streams one input. Returns the number of matching lines.
fn process(cli: &Cli, mut reader: Box<dyn BufRead>, out: &mut impl Write) -> anyhow::Result<usize> {
    let mut matches = 0;
    // Reusable byte buffer: no allocation per line, and NO failure on invalid UTF-8 –
    // `lines()` would return an error for the whole file on the first bad byte.
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer)? == 0 {
            break; // EOF
        }
        let line = String::from_utf8_lossy(&buffer);
        let line = line.trim_end_matches(['\n', '\r']);
        let fields = logfmt::parse(line);

        let level_ok = cli
            .level
            .as_deref()
            .is_none_or(|level| logfmt::get(&fields, "level") == Some(level));
        let service_ok = cli
            .service
            .as_deref()
            .is_none_or(|service| logfmt::get(&fields, "service") == Some(service));
        if !(level_ok && service_ok) {
            continue;
        }
        matches += 1;

        if cli.count || cli.files_with_matches {
            continue;
        }
        if cli.json {
            let object: serde_json::Map<String, serde_json::Value> = fields
                .iter()
                .map(|(k, v)| ((*k).to_string(), serde_json::Value::from(*v)))
                .collect();
            writeln!(out, "{}", serde_json::Value::Object(object))?;
        } else {
            writeln!(out, "{line}")?;
        }
    }
    Ok(matches)
}

/// Rust ignores SIGPIPE (the default would kill the process silently); writes to a closed pipe
/// return `ErrorKind::BrokenPipe` instead – which must not be printed as an error.
fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
    })
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    // clap's definition checks run only in debug builds and only when parsing – a duplicated
    // short option would panic at runtime. This test turns it into a test failure.
    #[test]
    fn cli_definition_is_valid() {
        super::Cli::command().debug_assert();
    }
}
