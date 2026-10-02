//! Builder API: the CLI is a value (`clap::Command`) constructed at runtime.
//!
//! - pro: structure can depend on runtime data – subcommands from config files, plugins, aliases
//! - pro: no proc-macro – slightly faster compile, everything explicit
//! - con: values are looked up by string id (`get_one::<u32>("replicas")`) – a typo is a runtime
//!   panic (caught by `debug_assert` in tests); more verbose
//!
//! Both APIs produce the same `clap::Command` underneath; derive types can even be mixed with
//! builder code (`Cli::command()` returns a `Command` you can extend).

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::Action;

/// `aliases` – user-defined shortcuts known only at runtime, e.g. read from
/// `~/.config/platctl/config.toml` (like `git` aliases). Impossible with derive alone.
pub fn command(aliases: &[(&str, &str)]) -> Command {
    let mut command = Command::new("platctl")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Operate services on the internal platform.")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("env")
                .short('e')
                .long("env")
                // Derive uppercases value names automatically (`<ENV>`); the builder doesn't.
                .value_name("ENV")
                .help("Target environment")
                .global(true)
                .default_value("dev"),
        )
        .subcommand(
            Command::new("scale")
                .about("Change the number of replicas")
                .arg(
                    Arg::new("service")
                        .value_name("SERVICE")
                        .required(true)
                        .help("Service name"),
                )
                .arg(
                    Arg::new("replicas")
                        .short('r')
                        .long("replicas")
                        .value_name("REPLICAS")
                        .required(true)
                        .help("Desired number of replicas")
                        // `value_parser!` – parse/validate into a type (same as a typed field).
                        .value_parser(value_parser!(u32)),
                ),
        )
        .subcommand(
            Command::new("restart")
                .about("Restart all replicas of a service")
                .arg(
                    Arg::new("service")
                        .value_name("SERVICE")
                        .required(true)
                        .help("Service name"),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .help("Restart immediately, without draining connections")
                        .action(ArgAction::SetTrue),
                ),
        );

    // Runtime-generated subcommands. Names/help built from `String`s (not `&'static str`) need
    // clap's `string` feature (enabled in the workspace `Cargo.toml`).
    for (name, expands_to) in aliases {
        command = command
            .subcommand(Command::new(name.to_string()).about(format!("Alias for `{expands_to}`")));
    }
    command
}

/// Reads values by id. `get_one::<T>` must use the same type as the `value_parser`.
pub fn action(matches: &ArgMatches, aliases: &[(&str, &str)]) -> Action {
    let env = matches
        .get_one::<String>("env")
        .cloned()
        .unwrap_or_default();
    match matches.subcommand() {
        Some(("scale", sub)) => Action::Scale {
            service: sub
                .get_one::<String>("service")
                .cloned()
                .unwrap_or_default(),
            replicas: sub.get_one::<u32>("replicas").copied().unwrap_or_default(),
            env,
        },
        Some(("restart", sub)) => Action::Restart {
            service: sub
                .get_one::<String>("service")
                .cloned()
                .unwrap_or_default(),
            force: sub.get_flag("force"),
            env,
        },
        Some((name, _)) => {
            let expands_to = aliases
                .iter()
                .find(|(alias, _)| *alias == name)
                .map(|(_, target)| target.to_string())
                .unwrap_or_default();
            Action::Alias {
                name: name.to_string(),
                expands_to,
            }
        }
        // `subcommand_required(true)` makes this unreachable for parsed input.
        None => unreachable!("subcommand is required"),
    }
}
