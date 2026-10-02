//! `platctl-up` – step 026: a CLI that updates itself.
//!
//! ```text
//! platctl-up status                       normal work; may print "new version available" afterwards
//! platctl-up update [--check] [--yes]     install the newest release of the selected channel
//! platctl-up channel [stable|beta]        show / persist the update channel
//! platctl-up releases                     list newer releases in the channel
//! ```
//!
//! Releases come from a manifest feed (`--feed URL`, any static file server) or from GitHub
//! Releases (`--repo owner/name`). Both are `self_update` backends.

mod channel;
mod notice;
mod source;

use std::{
    io::{self, IsTerminal},
    process::ExitCode,
};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand};

use crate::{
    channel::{Channel, State},
    source::Source,
};

/// Version compiled into the binary. The demo builds "old" and "new" releases from the same
/// sources by overriding it: `PLATCTL_UP_BUILD_VERSION=0.2.0 cargo build ...`.
/// `option_env!` is evaluated at compile time; Cargo rebuilds when the variable changes.
pub const VERSION: &str = match option_env!("PLATCTL_UP_BUILD_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
pub const BIN: &str = "platctl-up";

/// Platform CLI that keeps itself up to date.
#[derive(Debug, Parser)]
#[command(name = BIN, version = VERSION)]
struct Cli {
    #[command(flatten)]
    source: SourceArgs,
    /// Never check for new versions in the background (also: PLATCTL_NO_UPDATE_CHECK=1)
    #[arg(long, global = true, env = "PLATCTL_NO_UPDATE_CHECK")]
    no_update_check: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct SourceArgs {
    /// Base URL of a manifest feed (`<URL>/manifest.json`); takes precedence over --repo
    #[arg(long, global = true, env = "PLATCTL_UP_FEED")]
    feed: Option<String>,
    /// GitHub repository publishing the releases
    #[arg(
        long,
        global = true,
        env = "PLATCTL_UP_REPO",
        default_value = "example-org/platctl"
    )]
    repo: String,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Do some "real" work – update notices are printed after ordinary commands
    Status,
    /// Install the newest release of the channel
    Update {
        /// Only report whether an update is available (exit code 10 if it is)
        #[arg(long)]
        check: bool,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
        /// Override the saved channel for this run
        #[arg(long)]
        channel: Option<Channel>,
    },
    /// Show or set the update channel
    Channel { channel: Option<Channel> },
    /// List releases newer than the installed one in the channel
    Releases {
        #[arg(long)]
        channel: Option<Channel>,
    },
}

/// Exit code for `update --check` when a newer version exists – scripts can branch on it.
const EXIT_UPDATE_AVAILABLE: u8 = 10;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let state = State::load();
    let source = match Source::from_args(cli.source.feed.as_deref(), &cli.source.repo) {
        Ok(source) => source,
        Err(error) => return fail(&error),
    };

    // Start the throttled background check BEFORE doing the work, collect it AFTER: the network
    // round trip overlaps with the command and never delays it noticeably.
    let pending_notice = matches!(cli.command, Command::Status)
        .then(|| notice::start(&source, &state, cli.no_update_check))
        .flatten();

    let result = run(cli.command, &source, &state);

    if let Some(pending) = pending_notice {
        pending.print_if_ready();
    }
    match result {
        Ok(code) => code,
        Err(error) => fail(&error),
    }
}

fn fail(error: &anyhow::Error) -> ExitCode {
    eprintln!("error: {error:#}");
    ExitCode::FAILURE
}

fn run(command: Command, source: &Source, state: &State) -> anyhow::Result<ExitCode> {
    match command {
        Command::Status => {
            println!("platctl {VERSION}: all services healthy");
        }
        Command::Channel { channel: None } => println!("{}", state.channel()),
        Command::Channel {
            channel: Some(channel),
        } => {
            state.set_channel(channel)?;
            println!("update channel set to {channel}");
        }
        Command::Releases { channel } => {
            let channel = channel.unwrap_or(state.channel());
            let releases = source.releases(channel)?;
            if releases.is_empty() {
                println!("no release newer than {VERSION} in the {channel} channel");
            }
            for release in releases {
                println!("{release}");
            }
        }
        Command::Update {
            check,
            yes,
            channel,
        } => {
            let channel = channel.unwrap_or(state.channel());
            if let Some(manager) = source::package_manager() {
                bail!(
                    "{BIN} was installed by {manager} – update it with {manager} instead \
                     (a self-update would fight with the package manager's files)"
                );
            }
            let Some(newest) = source.newest(channel)? else {
                println!("{BIN} {VERSION} is up to date ({channel} channel)");
                if channel == Channel::Stable && channel::is_prerelease(VERSION) {
                    println!("note: running a beta – it stays until a newer stable release exists");
                }
                return Ok(ExitCode::SUCCESS);
            };
            if check {
                println!("update available: {VERSION} -> {newest} ({channel} channel)");
                return Ok(ExitCode::from(EXIT_UPDATE_AVAILABLE));
            }
            if !yes && !io::stdin().is_terminal() {
                bail!("cannot ask for confirmation without a terminal – pass --yes");
            }
            let status = source
                .installer(&newest, yes)?
                .update()
                .context("update failed – the installed binary was not changed")?;
            println!("updated {BIN}: {VERSION} -> {}", status.version());
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
