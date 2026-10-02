//! `platctl-deploy` – step 003: error handling and exit codes.
//!
//! Layers of error handling:
//! - modules return typed errors (`thiserror`): `ManifestError`, `PlatformError`,
//! - the application glue uses `anyhow::Result` and adds *context* ("while doing X"),
//! - `main` reports the error chain on stderr and maps it to a meaningful exit code.

mod exit_codes;
mod manifest;
mod platform;

use std::{path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;

use crate::{manifest::ManifestError, platform::PlatformError};

/// Deploy a service described by a manifest file.
#[derive(Debug, Parser)]
// `about` comes from the doc comment above (a bare `about` would use the Cargo.toml description).
#[command(name = "platctl-deploy", version)]
struct Cli {
    /// Manifest file (key = value lines)
    manifest: PathBuf,

    /// Target environment
    #[arg(short, long, default_value = "dev")]
    env: String,

    /// Print the full error chain with debug details
    #[arg(short, long)]
    verbose: bool,

    /// Pretend the platform API timed out (to demonstrate a transient error)
    #[arg(long, hide = true)]
    simulate_timeout: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error, cli.verbose);
            ExitCode::from(exit_codes::for_error(&error))
        }
    }
}

/// The application logic. `anyhow::Result` accepts any error type via `?` and lets every step
/// add context – the user learns not only WHAT failed but WHILE DOING WHAT.
fn run(cli: &Cli) -> anyhow::Result<()> {
    let manifest = manifest::load(&cli.manifest)
        // `with_context` builds the message lazily (only on error).
        .with_context(|| format!("invalid deployment manifest {}", cli.manifest.display()))?;

    let deployment = platform::deploy(&manifest, &cli.env, cli.simulate_timeout)
        .with_context(|| format!("failed to deploy {} to {}", manifest.service, cli.env))?;

    // Result on stdout – a script can capture it: `ID=$(platctl-deploy m.manifest)`.
    println!("{deployment}");
    Ok(())
}

/// Human-oriented error output on stderr:
///
/// ```text
/// error: failed to deploy payments to dev
///   caused by: service `payments` is not registered on the platform
/// ```
///
/// - one line per cause (`anyhow::Error::chain` walks `source()`),
/// - lowercase, no trailing period – the format used by cargo, rustc, git,
/// - `--verbose`: full `Debug` output (includes a backtrace when `RUST_BACKTRACE=1`).
fn report(error: &anyhow::Error, verbose: bool) {
    eprintln!("error: {error}");
    for cause in error.chain().skip(1) {
        eprintln!("  caused by: {cause}");
    }
    if let Some(hint) = hint(error) {
        eprintln!("  hint: {hint}");
    }
    if verbose {
        eprintln!("\ndebug: {error:?}");
    }
}

/// Actionable advice for errors the user can fix. `downcast_ref` looks for a concrete error
/// type anywhere in the chain – typed errors stay inspectable through `anyhow`.
fn hint(error: &anyhow::Error) -> Option<&'static str> {
    for cause in error.chain() {
        if let Some(PlatformError::PermissionDenied(_)) = cause.downcast_ref() {
            return Some("request an approval with `platctl approvals request`");
        }
        if let Some(PlatformError::Timeout(_)) = cause.downcast_ref() {
            return Some("the platform may be overloaded; retry in a minute");
        }
        if let Some(ManifestError::Read { .. }) = cause.downcast_ref() {
            return Some("check the path; manifests live in ./manifests");
        }
    }
    None
}
