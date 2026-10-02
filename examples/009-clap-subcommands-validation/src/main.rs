//! `platctl-release` – step 009: subcommands as enums, `ValueEnum`, ranges, groups, conflicts,
//! custom value parsers. All checks happen BEFORE any command code runs.

mod cli;
mod parsers;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Create(args) => {
            // Cross-field rule not expressible declaratively; `exit()` prints it like a clap
            // error and exits with 2.
            if let Err(error) = args.validate() {
                error.exit();
            }
            let source = match (&args.version, &args.commit) {
                (Some(version), _) => format!("version {version}"),
                (None, Some(commit)) => format!("commit {commit}"),
                // The ArgGroup guarantees one of them.
                (None, None) => unreachable!("validated by clap"),
            };
            let mode = if args.dry_run { "[dry-run] " } else { "" };
            println!(
                "{mode}release {} ({source}) to {} in {} via {:?}{}, timeout {:?}",
                args.service,
                args.target.env,
                args.target.regions.join(","),
                args.strategy,
                args.canary_percent
                    .map(|p| format!(" {p}%"))
                    .unwrap_or_default(),
                args.timeout,
            );
            for (key, value) in &args.labels {
                println!("  label {key}={value}");
            }
        }
        Command::Rollback {
            service,
            to,
            target,
        } => println!("rollback {service} in {} to {to}", target.env),
        Command::List { since } => match since {
            Some(since) => println!("releases from the last {since:?}"),
            None => println!("all releases"),
        },
    }
}
