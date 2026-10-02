//! `platctl-release` CLI: validation is declared, not hand-written.
//!
//! ```text
//! platctl-release create <SERVICE> (--version <VER> | --commit <SHA>) [--strategy rolling|blue-green|canary]
//!                        [--canary-percent 1..=100] [--timeout 30s] [--label k=v]... [--region r,...]
//!                        [--dry-run | --yes]
//! platctl-release rollback <SERVICE> --to <VER>
//! platctl-release list [--since 7d...]
//! ```

use std::time::Duration;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

use crate::parsers;

/// Create, roll back and list releases.
#[derive(Debug, Parser)]
#[command(name = "platctl-release", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start a new release
    Create(CreateArgs),
    /// Roll back to a previous version
    Rollback {
        /// Service name
        service: String,
        /// Version to roll back to
        #[arg(long, value_parser = parsers::version)]
        to: semver::Version,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// List recent releases
    List {
        /// Only releases newer than this (e.g. 30m, 12h)
        #[arg(long, value_parser = parsers::duration, value_name = "DURATION")]
        since: Option<Duration>,
    },
}

/// Options shared by several subcommands – defined once, `flatten`ed where needed.
#[derive(Debug, Args)]
pub struct TargetArgs {
    /// Target environment
    #[arg(short, long, default_value = "dev")]
    pub env: String,

    /// Regions (comma-separated or repeated)
    // `value_delimiter` – `--region a,b` == `--region a --region b`.
    #[arg(long = "region", value_delimiter = ',', default_value = "eu-west-1")]
    pub regions: Vec<String>,
}

/// Rollout strategy. `ValueEnum` derives the accepted values from the variants
/// (`rolling`, `blue-green`, `canary`), shows doc comments in `--help`, and suggests
/// the closest value on typos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Strategy {
    /// Replace instances in batches
    Rolling,
    /// Start a full new set, switch traffic at once
    // Extra accepted spelling.
    #[value(alias = "bg")]
    BlueGreen,
    /// Send a percentage of traffic to the new version first
    Canary,
}

#[derive(Debug, Args)]
// ArgGroup: exactly one of `--version` / `--commit` must be given.
// `required = true` – at least one; `multiple = false` (default) – at most one.
#[command(group(ArgGroup::new("source").required(true).args(["version", "commit"])))]
pub struct CreateArgs {
    /// Service name
    pub service: String,

    /// Version to release
    #[arg(long, value_parser = parsers::version)]
    pub version: Option<semver::Version>,

    /// Git commit to build and release (7–40 hex chars)
    #[arg(long, value_parser = commit_sha)]
    pub commit: Option<String>,

    /// Rollout strategy
    #[arg(long, value_enum, default_value_t = Strategy::Rolling)]
    pub strategy: Strategy,

    /// Traffic percentage for the canary
    // Built-in range validation; `required_if_eq` – mandatory when `--strategy canary`.
    // `requires`-style check in the other direction is done in `validate` below.
    #[arg(
        long,
        value_parser = clap::value_parser!(u8).range(1..=100),
        required_if_eq("strategy", "canary")
    )]
    pub canary_percent: Option<u8>,

    /// Maximum rollout time (e.g. 90, 30s, 5m, 1h30m)
    #[arg(long, value_parser = parsers::duration, default_value = "10m", value_name = "DURATION")]
    pub timeout: Duration,

    /// Labels attached to the release (repeatable)
    #[arg(short, long = "label", value_parser = parsers::key_value, value_name = "KEY=VALUE")]
    pub labels: Vec<(String, String)>,

    /// Only show the plan
    // `conflicts_with` – the two flags make no sense together.
    #[arg(long, conflicts_with = "yes")]
    pub dry_run: bool,

    /// Skip the confirmation prompt
    #[arg(short, long)]
    pub yes: bool,

    #[command(flatten)]
    pub target: TargetArgs,
}

/// Inline closure-free parser: a plain `fn` is enough.
fn commit_sha(input: &str) -> Result<String, String> {
    let valid = (7..=40).contains(&input.len()) && input.chars().all(|c| c.is_ascii_hexdigit());
    if valid {
        Ok(input.to_lowercase())
    } else {
        Err(format!("`{input}` is not a git commit SHA"))
    }
}

impl CreateArgs {
    /// Rules clap can't express declaratively are checked after parsing and reported in the
    /// SAME format as clap's own errors (`Cli::command().error(...)`) – consistent UX, exit 2.
    pub fn validate(&self) -> Result<(), clap::Error> {
        use clap::CommandFactory;
        if self.canary_percent.is_some() && self.strategy != Strategy::Canary {
            // Build the command tree (computes full bin names) and take the `create`
            // subcommand, so the error shows `Usage: platctl-release create ...`.
            let mut command = Cli::command();
            command.build();
            let create = command
                .find_subcommand_mut("create")
                .expect("create subcommand exists");
            return Err(create.error(
                clap::error::ErrorKind::ArgumentConflict,
                "--canary-percent can only be used with --strategy canary",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, error::ErrorKind};

    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("platctl-release").chain(args.iter().copied()))
    }

    fn create(args: &[&str]) -> CreateArgs {
        match parse(args).unwrap().command {
            Command::Create(args) => args,
            other => panic!("expected create, got {other:?}"),
        }
    }

    fn error_kind(args: &[&str]) -> ErrorKind {
        parse(args).unwrap_err().kind()
    }

    #[test]
    fn definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn typed_values_and_defaults() {
        let args = create(&[
            "create",
            "billing",
            "--version",
            "v1.4.2",
            "--strategy",
            "bg",
            "-l",
            "team=payments",
            "--region",
            "eu-west-1,us-east-1",
            "--timeout",
            "1h30m",
        ]);

        assert_eq!(args.version.unwrap().to_string(), "1.4.2");
        assert_eq!(args.strategy, Strategy::BlueGreen);
        assert_eq!(args.labels, [("team".to_string(), "payments".to_string())]);
        assert_eq!(args.target.regions, ["eu-west-1", "us-east-1"]);
        assert_eq!(args.timeout, Duration::from_secs(5400));
    }

    #[test]
    fn exactly_one_source_is_required() {
        assert_eq!(
            error_kind(&["create", "billing"]),
            ErrorKind::MissingRequiredArgument
        );
        assert_eq!(
            error_kind(&[
                "create",
                "billing",
                "--version",
                "1.0.0",
                "--commit",
                "abc1234"
            ]),
            ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn canary_percent_rules() {
        let canary = [
            "create",
            "billing",
            "--version",
            "1.0.0",
            "--strategy",
            "canary",
        ];
        assert_eq!(error_kind(&canary), ErrorKind::MissingRequiredArgument);

        let mut out_of_range = canary.to_vec();
        out_of_range.extend(["--canary-percent", "150"]);
        assert_eq!(error_kind(&out_of_range), ErrorKind::ValueValidation);

        let wrong_strategy = create(&[
            "create",
            "billing",
            "--version",
            "1.0.0",
            "--canary-percent",
            "10",
        ]);
        assert!(wrong_strategy.validate().is_err());
    }

    #[test]
    fn conflicts_and_invalid_values() {
        let base = ["create", "billing", "--version", "1.0.0"];
        let with = |extra: &[&'static str]| [&base[..], extra].concat();

        assert_eq!(
            error_kind(&with(&["--dry-run", "--yes"])),
            ErrorKind::ArgumentConflict
        );
        assert_eq!(
            error_kind(&with(&["--strategy", "big-bang"])),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            error_kind(&with(&["--timeout", "5x"])),
            ErrorKind::ValueValidation
        );
        assert_eq!(
            error_kind(&["create", "billing", "--commit", "xyz"]),
            ErrorKind::ValueValidation
        );
    }
}
