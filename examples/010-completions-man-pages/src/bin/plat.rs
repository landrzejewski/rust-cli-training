//! The `plat` tool.

use std::io;

use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;
use ex010_completions_man_pages::{Cli, Command};

fn main() {
    // Dynamic completion hook – MUST run first. When the shell calls `plat` with
    // `COMPLETE=<shell>` in the environment, this prints completions (or the registration
    // script) and exits; otherwise it returns immediately and the program runs normally.
    //
    //   source <(COMPLETE=bash plat)        # bash
    //   source <(COMPLETE=zsh plat)         # zsh
    //   COMPLETE=fish plat | source         # fish
    CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();
    match cli.command {
        // Static completion: a script generated once (e.g. by a package manager at install
        // time). Completes subcommands, flags and ValueEnum values, but not dynamic values.
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "plat", &mut io::stdout());
        }
        Command::Deploy {
            service,
            env,
            manifest,
        } => {
            let manifest = manifest
                .map(|p| format!(" using {}", p.display()))
                .unwrap_or_default();
            println!("deploying {service} to {env:?}{manifest}");
        }
        Command::Logs { service, follow } => {
            println!(
                "logs of {service}{}",
                if follow { " (following)" } else { "" }
            );
        }
    }
}
