//! `platctl` – step 002: commands, subcommands, flags, positional and global arguments.
//!
//! The CLI is declared in `cli.rs`; `main` only parses and dispatches to one function per
//! command. The "platform" is simulated with static data.

mod cli;

use clap::Parser;
use cli::{Cli, Command, DeployArgs, EnvCommand, GlobalArgs};

const SERVICES: [(&str, &str, &str); 3] = [
    ("billing", "1.4.1", "running"),
    ("search", "2.0.0", "running"),
    ("notifications", "0.9.3", "degraded"),
];

const ENVIRONMENTS: [(&str, &str); 3] = [
    ("dev", "Development, deployed on every merge"),
    ("staging", "Pre-production, mirrors prod configuration"),
    ("prod", "Production, changes need approval"),
];

fn main() {
    // `parse()` reads `std::env::args_os()`. On `--help`/`--version` it prints and exits with 0;
    // on a usage error it prints the error to stderr and exits with 2.
    let cli = Cli::parse();

    if cli.global.verbose > 0 {
        // Diagnostics -> stderr (step 001).
        eprintln!("[debug] parsed: {cli:?}");
    }

    // Exhaustive `match` – adding a new subcommand without handling it is a compile error.
    match cli.command {
        Command::Deploy(args) => deploy(&cli.global, &args),
        Command::Status { services } => status(&services),
        Command::Logs {
            service,
            lines,
            follow,
        } => logs(&service, lines, follow),
        Command::Env(EnvCommand::List) => env_list(),
        Command::Env(EnvCommand::Show { name }) => env_show(&name),
    }
}

fn deploy(global: &GlobalArgs, args: &DeployArgs) {
    let tag = args.tag.as_deref().unwrap_or("latest");
    let prefix = if args.dry_run {
        "[dry-run] would deploy"
    } else {
        "deploying"
    };
    println!(
        "{prefix} {}:{tag} to {} ({} replica(s))",
        args.service, global.env, args.replicas
    );
}

fn status(services: &[String]) {
    for (name, version, state) in SERVICES {
        if services.is_empty() || services.iter().any(|s| s == name) {
            println!("{name:<15} {version:<8} {state}");
        }
    }
}

fn logs(service: &str, lines: usize, follow: bool) {
    for i in 1..=lines.min(3) {
        println!("ts={i} service={service} msg=\"request handled\"");
    }
    if follow {
        eprintln!("(--follow is simulated in this example)");
    }
}

fn env_list() {
    for (name, _) in ENVIRONMENTS {
        println!("{name}");
    }
}

fn env_show(name: &str) {
    match ENVIRONMENTS.iter().find(|(env, _)| *env == name) {
        Some((env, description)) => println!("{env}: {description}"),
        None => {
            eprintln!("platctl: unknown environment '{name}'");
            std::process::exit(1);
        }
    }
}
