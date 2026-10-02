//! Step 010 – shell completions and documentation generated from the clap definition.
//!
//! The CLI lives in the library so that two binaries share it:
//! - `plat` – the tool itself (`plat completions zsh`, dynamic completion via `COMPLETE=zsh plat`),
//! - `gen-docs` – writes man pages, completion scripts and Markdown at release time.
//!
//! (The binary is called `plat` because `platctl` is already built by example 002 and
//! binary names must be unique in a workspace.)

use std::{ffi::OsStr, path::PathBuf};

use clap::{Parser, Subcommand, ValueEnum, ValueHint};
use clap_complete::engine::{ArgValueCompleter, CompletionCandidate};

/// Internal developer platform CLI.
///
/// Deploys services and shows their logs. Shell completion and man pages are generated from
/// this definition – help text, completions and documentation never drift apart.
#[derive(Debug, Parser)]
#[command(name = "plat", version, author = "Platform Team")]
pub struct Cli {
    /// More output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Deploy a service
    Deploy {
        /// Service to deploy
        // Dynamic completion: candidates computed at completion time (here a static list;
        // a real tool would ask the platform API or read a local cache).
        #[arg(add = ArgValueCompleter::new(complete_service))]
        service: String,

        /// Target environment
        // `ValueEnum` values are completed automatically – no extra code.
        #[arg(short, long, value_enum, default_value_t = Env::Dev)]
        env: Env,

        /// Deployment manifest
        // `ValueHint` tells shells what kind of value is expected (file path -> file completion).
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        manifest: Option<PathBuf>,
    },
    /// Show service logs
    Logs {
        /// Service whose logs to show
        #[arg(add = ArgValueCompleter::new(complete_service))]
        service: String,

        /// Keep streaming new lines
        #[arg(short, long)]
        follow: bool,
    },
    /// Print a shell completion script
    ///
    /// Static script, e.g.: plat completions zsh > ~/.zfunc/_plat
    Completions {
        /// Target shell
        // `clap_complete::Shell` implements `ValueEnum`: bash, elvish, fish, powershell, zsh.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Env {
    Dev,
    Staging,
    Prod,
}

const SERVICES: [(&str, &str); 4] = [
    ("billing", "Invoices and payments"),
    ("search", "Full-text search"),
    ("notifications", "E-mail and push"),
    ("reports", "Scheduled reports"),
];

/// Completer: receives the word typed so far, returns candidates (with optional help shown by
/// zsh/fish next to the value).
pub fn complete_service(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(prefix) = current.to_str() else {
        return Vec::new();
    };
    SERVICES
        .iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, help)| CompletionCandidate::new(*name).help(Some((*help).into())))
        .collect()
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn service_completer_filters_by_prefix() {
        let names: Vec<_> = complete_service(OsStr::new("se"))
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, ["search"]);
    }

    #[test]
    fn static_scripts_are_generated_for_every_shell() {
        for shell in [
            clap_complete::Shell::Bash,
            clap_complete::Shell::Zsh,
            clap_complete::Shell::Fish,
            clap_complete::Shell::PowerShell,
        ] {
            let mut buffer = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "plat", &mut buffer);
            let script = String::from_utf8(buffer).unwrap();
            assert!(script.contains("deploy"), "{shell:?}");
        }
    }
}
