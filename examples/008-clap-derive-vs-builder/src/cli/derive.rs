//! Derive API: the CLI is described by Rust types; clap generates the parser at compile time.
//!
//! - pro: concise, type-safe – parsed values land in typed fields, no string lookups
//! - pro: help texts from doc comments, refactoring is checked by the compiler
//! - con: structure fixed at compile time

use clap::{Parser, Subcommand};

use crate::Action;

/// Operate services on the internal platform.
#[derive(Debug, Parser)]
// No bare `about` here: `about` without a value means "use the package description from
// Cargo.toml" and would override the doc comment above.
#[command(name = "platctl", version)]
pub struct Cli {
    /// Target environment
    #[arg(short, long, global = true, default_value = "dev")]
    pub env: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Change the number of replicas
    Scale {
        /// Service name
        service: String,
        /// Desired number of replicas
        #[arg(short, long)]
        replicas: u32,
    },
    /// Restart all replicas of a service
    Restart {
        /// Service name
        service: String,
        /// Restart immediately, without draining connections
        #[arg(long)]
        force: bool,
    },
}

impl Cli {
    /// Converts clap's types into the parser-independent `Action`.
    pub fn into_action(self) -> Action {
        match self.command {
            Command::Scale { service, replicas } => Action::Scale {
                service,
                replicas,
                env: self.env,
            },
            Command::Restart { service, force } => Action::Restart {
                service,
                force,
                env: self.env,
            },
        }
    }
}
