//! `platctl-services` – step 006: predictable output formats.
//!
//! ```text
//! platctl-services list [--env ENV] [-o auto|human|table|json|jsonl|yaml|tsv] [--no-headers]
//! platctl-services get <NAME> [-o ...]
//! ```

mod model;
mod output;

use std::{
    io::{self, IsTerminal, Write},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use output::Format;

#[derive(Debug, Parser)]
#[command(name = "platctl-services", version, about = "List platform services")]
struct Cli {
    /// Output format
    // `value_enum` – accepted values come from `Format` (and appear in `--help`).
    #[arg(short, long, global = true, value_enum, default_value_t = Format::Auto)]
    output: Format,

    /// Omit the header row (table, tsv)
    #[arg(long, global = true)]
    no_headers: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List services
    List {
        /// Only services in this environment
        #[arg(long)]
        env: Option<String>,
    },
    /// Show one service
    Get { name: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // `IsTerminal` (std since 1.70) – is stdout a terminal or a pipe/file?
    let format = cli.output.resolve(io::stdout().is_terminal());
    let services = model::sample();

    let rendered = match &cli.command {
        Command::List { env } => {
            let selected: Vec<_> = services
                .into_iter()
                .filter(|s| env.as_ref().is_none_or(|e| &s.env == e))
                .collect();
            output::render_list(&selected, format, !cli.no_headers)
        }
        Command::Get { name } => match services.iter().find(|s| &s.name == name) {
            Some(service) => output::render_one(service, format),
            None => {
                // Errors stay human-readable on stderr regardless of --output; the exit code
                // tells scripts that there is no document on stdout.
                eprintln!("error: service '{name}' not found");
                return ExitCode::FAILURE;
            }
        },
    };

    // Ignore write errors from closed pipes (`| head -1`) – details in step 013.
    let _ = io::stdout().write_all(rendered.as_bytes());
    ExitCode::SUCCESS
}
