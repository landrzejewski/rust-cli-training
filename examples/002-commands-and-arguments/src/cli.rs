//! Command-line interface of `platctl`, declared with clap's derive API.
//!
//! Structs and enums describe the CLI; `#[derive(Parser)]` generates the parser, `--help`,
//! `--version`, error messages with suggestions – everything implemented by hand in step 001.
//!
//! ```text
//! platctl [GLOBAL OPTIONS] <COMMAND>
//!   deploy <SERVICE> [--tag <TAG>] [--replicas <N>] [--dry-run]
//!   status [SERVICE]...                     (alias: st)
//!   logs <SERVICE> [-n <N>] [-f]
//!   env list | env show <NAME>              (nested subcommands)
//! ```

use clap::{Args, Parser, Subcommand};

/// Command-line tool of the internal developer platform.
///
/// Deploys services, shows their status and logs, and manages environments.
// Doc comments (`///`) become help texts: the first paragraph is the short `about` shown by `-h`,
// the whole comment is the long description shown by `--help`. Plain `//` comments are not used.
#[derive(Debug, Parser)]
// `name` – program name in help/version (default: the package name).
// `version` – adds `-V/--version` using CARGO_PKG_VERSION.
// `about` – taken from the doc comment above.
// `propagate_version` – subcommands print the same version.
#[command(name = "platctl", version, about, propagate_version = true)]
pub struct Cli {
    /// Global options are defined once and accepted before AND after the subcommand.
    // `flatten` inlines the fields of another struct – keeps groups of options reusable.
    #[command(flatten)]
    pub global: GlobalArgs,

    // A required subcommand. Without one, clap prints an error with the list of commands.
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Args)]
pub struct GlobalArgs {
    /// More output (repeatable: -v, -vv, -vvv)
    // `ArgAction::Count` – counts occurrences into a number.
    // `global = true` – may appear anywhere: `platctl -v status` and `platctl status -v`.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Target environment
    // `default_value` is shown in help as `[default: dev]`.
    #[arg(short, long, global = true, default_value = "dev", value_name = "NAME")]
    pub env: String,
}

/// Subcommands as an enum – every variant is a command, its fields are the command's arguments.
/// `match` on it in `main` is checked for exhaustiveness by the compiler.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Deploy a service to the target environment
    Deploy(DeployArgs),

    /// Show the status of services
    // `visible_alias` – `platctl st` works too and the alias appears in help.
    #[command(visible_alias = "st")]
    Status {
        /// Services to show; all when omitted
        // `Vec<T>` positional = zero or more values (`[SERVICE]...`).
        services: Vec<String>,
    },

    /// Print recent log lines of a service
    Logs {
        /// Service name
        // A plain `String` positional is required (`<SERVICE>`).
        service: String,

        /// Number of lines
        #[arg(short = 'n', long, default_value_t = 20)]
        lines: usize,

        /// Keep printing new lines
        // `bool` field = flag (`--follow`, `-f`), no value.
        #[arg(short, long)]
        follow: bool,
    },

    /// Manage environments
    // Nested subcommands: `platctl env list`, `platctl env show prod`.
    #[command(subcommand)]
    Env(EnvCommand),
}

/// Arguments of `deploy` in a separate struct – handy when a command has many options or the
/// arguments are passed around (`fn deploy(args: &DeployArgs)`).
#[derive(Debug, Args)]
pub struct DeployArgs {
    /// Service to deploy
    pub service: String,

    /// Image tag to deploy [default: latest]
    // `Option<T>` = optional option without a default.
    // Named `--tag`, not `--version`: `propagate_version` already adds `--version` to every
    // subcommand – the `cli_definition_is_valid` test catches such clashes.
    #[arg(short, long, value_name = "TAG")]
    pub tag: Option<String>,

    /// Number of instances
    #[arg(short, long, default_value_t = 1)]
    pub replicas: u32,

    /// Show what would happen without changing anything
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum EnvCommand {
    /// List environments
    List,
    /// Show details of an environment
    Show {
        /// Environment name
        name: String,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    // Validates the whole CLI definition (conflicting names, wrong defaults...) at test time
    // instead of at runtime in front of a user.
    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    // `try_parse_from` parses an explicit argument list (first item = program name)
    // and returns `Err` instead of exiting – ideal for tests.
    #[test]
    fn global_options_work_before_and_after_the_subcommand() {
        let before = Cli::try_parse_from(["platctl", "-vv", "--env", "prod", "status"]).unwrap();
        let after = Cli::try_parse_from(["platctl", "status", "-vv", "-e", "prod"]).unwrap();

        for cli in [before, after] {
            assert_eq!(cli.global.verbose, 2);
            assert_eq!(cli.global.env, "prod");
        }
    }

    #[test]
    fn deploy_arguments_and_defaults() {
        let cli = Cli::try_parse_from(["platctl", "deploy", "billing", "--tag", "1.4.2"]).unwrap();

        let Command::Deploy(args) = cli.command else {
            panic!("expected deploy");
        };
        assert_eq!(args.service, "billing");
        assert_eq!(args.tag.as_deref(), Some("1.4.2"));
        assert_eq!(args.replicas, 1);
        assert!(!args.dry_run);
    }

    #[test]
    fn alias_and_variadic_positionals() {
        let cli = Cli::try_parse_from(["platctl", "st", "billing", "search"]).unwrap();

        assert!(matches!(cli.command, Command::Status { services } if services.len() == 2));
    }

    #[test]
    fn missing_required_positional_is_an_error() {
        let error = Cli::try_parse_from(["platctl", "deploy"]).unwrap_err();

        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
        // clap's usage errors exit with code 2 – the same convention as in step 001.
        assert_eq!(error.exit_code(), 2);
    }
}
