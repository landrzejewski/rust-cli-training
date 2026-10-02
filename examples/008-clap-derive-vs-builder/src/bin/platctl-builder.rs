//! Entry point using the builder front-end, with runtime-defined aliases.

use std::process::ExitCode;

use ex008_clap_derive_vs_builder::{cli::builder, commands};

/// In a real tool these would come from the user's config file.
const ALIASES: [(&str, &str); 2] = [("bounce", "restart --force"), ("up", "scale --replicas 1")];

fn main() -> ExitCode {
    // `get_matches` = `parse` of the builder world: exits on --help/--version/usage errors.
    let matches = builder::command(&ALIASES).get_matches();
    let action = builder::action(&matches, &ALIASES);
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
