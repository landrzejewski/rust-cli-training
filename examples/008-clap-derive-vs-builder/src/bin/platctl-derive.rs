//! Entry point using the derive front-end. Binaries stay thin: parse -> execute -> report.

use std::process::ExitCode;

use clap::Parser;
use ex008_clap_derive_vs_builder::{cli::derive::Cli, commands};

fn main() -> ExitCode {
    let action = Cli::parse().into_action();
    match commands::execute(action) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
