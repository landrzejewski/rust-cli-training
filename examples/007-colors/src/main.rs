//! `platctl-status` – step 007: colored output done right.
//!
//! - `color.rs` decides whether colors are allowed (flag, NO_COLOR, CLICOLOR_FORCE, TERM, TTY),
//! - output is painted with `owo-colors` (stdout table) and `nu-ansi-term` (stderr messages) –
//!   two popular crates shown side by side,
//! - colors only REINFORCE information: status words and symbols are always printed.

mod color;

use std::io::{self, IsTerminal};

use clap::Parser;
use color::{ColorChoice, Environment};
use nu_ansi_term::{Color as AnsiColor, Style as AnsiStyle};
use owo_colors::{OwoColorize, Style};

#[derive(Debug, Parser)]
#[command(
    name = "platctl-status",
    version,
    about = "Show service health with colors"
)]
struct Cli {
    /// When to use colors
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, value_name = "WHEN")]
    color: ColorChoice,
}

const SERVICES: [(&str, &str, &str); 4] = [
    ("billing", "running", "all 3 replicas healthy"),
    ("search", "running", "p95 latency 120 ms"),
    (
        "notifications",
        "degraded",
        "1 of 2 replicas failing health checks",
    ),
    ("reports", "stopped", "scaled to zero"),
];

fn main() {
    let cli = Cli::parse();

    // Separate decisions for stdout and stderr.
    let stdout_color =
        color::use_color(cli.color, &Environment::detect(io::stdout().is_terminal()));
    let stderr_color =
        color::use_color(cli.color, &Environment::detect(io::stderr().is_terminal()));

    // Windows 10+ consoles understand ANSI escape codes only after enabling "virtual terminal
    // processing"; nu-ansi-term provides the call. (Modern Windows Terminal has it on.)
    #[cfg(windows)]
    let _ = nu_ansi_term::enable_ansi_support();

    print_table(stdout_color);
    print_warnings(stderr_color);
}

/// owo-colors: extension methods on any `Display` value (`"text".green()`), zero allocations.
/// A `Style` value lets us pick "colored" or "plain" once instead of branching on every call.
fn print_table(enabled: bool) {
    let plain = Style::new();
    let pick = |style: Style| if enabled { style } else { plain };
    let header = pick(Style::new().bold().underline());

    println!(
        "{}  {}  {}",
        format!("{:<15}", "SERVICE").style(header),
        format!("{:<10}", "STATUS").style(header),
        "DETAILS".style(header)
    );
    for (name, status, details) in SERVICES {
        // Semantic colors + a symbol + the word: readable for color-blind users, monochrome
        // terminals and screen readers. Never encode meaning in color alone.
        let (symbol, style) = match status {
            "running" => ("✔", pick(Style::new().green())),
            "degraded" => ("⚠", pick(Style::new().yellow().bold())),
            _ => ("✖", pick(Style::new().red())),
        };
        // Pad BEFORE styling: escape codes have no width but count as characters in `{:<10}`.
        println!(
            "{:<15}  {}  {}",
            name,
            format!("{symbol} {status:<8}").style(style),
            details.style(pick(Style::new().dimmed()))
        );
    }
}

/// nu-ansi-term: `Color`/`Style` values that `paint` strings. `Style::default()` paints nothing,
/// so "disabled" is just another style.
fn print_warnings(enabled: bool) {
    let label = if enabled {
        AnsiColor::Yellow.bold()
    } else {
        AnsiStyle::default()
    };
    for (name, status, _) in SERVICES {
        if status != "running" {
            eprintln!("{} {name} is {status}", label.paint("warning:"));
        }
    }
}
