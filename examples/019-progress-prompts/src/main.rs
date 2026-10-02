//! `platctl-setup` – step 019: interactive prompts and progress reporting.
//!
//! ```text
//! platctl-setup init [--name N] [--template T] [--feature F]... [--yes] [--prompts dialoguer|inquire]
//! platctl-setup rollout [--service S]... [--fail S] [--quiet]
//! ```

mod dialoguer_prompts;
mod inquire_prompts;
mod rollout;
mod setup;

use std::{
    io::{self, IsTerminal},
    process::ExitCode,
};

use clap::{Parser, Subcommand, ValueEnum};
use indicatif::ProgressDrawTarget;
use setup::{Given, Outcome, Prompter};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PromptLib {
    Dialoguer,
    Inquire,
}

/// Bootstrap services and roll them out.
#[derive(Debug, Parser)]
#[command(name = "platctl-setup", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new service (asks for missing values when run in a terminal)
    Init {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        template: Option<String>,
        /// Feature to enable (repeatable)
        #[arg(long = "feature")]
        features: Option<Vec<String>>,
        /// Answer "yes" to the confirmation (required without a terminal)
        #[arg(short, long)]
        yes: bool,
        /// Prompt library to use
        #[arg(long, value_enum, default_value_t = PromptLib::Dialoguer)]
        prompts: PromptLib,
    },
    /// Roll out services with progress bars
    Rollout {
        #[arg(long = "service", default_values = ["billing", "search", "notifications", "reports"])]
        services: Vec<String>,
        /// Simulate a failing health check for this service
        #[arg(long)]
        fail: Option<String>,
        /// No progress output
        #[arg(short, long)]
        quiet: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Init {
            name,
            template,
            features,
            yes,
            prompts,
        } => {
            // Prompts read stdin and draw on stderr – both must be terminals.
            let interactive = io::stdin().is_terminal() && io::stderr().is_terminal();
            let prompter: Box<dyn Prompter> = match prompts {
                PromptLib::Dialoguer => Box::new(dialoguer_prompts::DialoguerPrompter::new()),
                PromptLib::Inquire => Box::new(inquire_prompts::InquirePrompter),
            };
            let given = Given {
                name,
                template,
                features,
                yes,
            };
            match setup::resolve(given, interactive, prompter.as_ref())? {
                Outcome::Create(plan) => {
                    if interactive {
                        let token = prompter.api_token()?;
                        if !token.is_empty() {
                            eprintln!(
                                "token received ({} chars, not stored in this demo)",
                                token.len()
                            );
                        }
                    }
                    // Result on stdout.
                    println!(
                        "created {} (template {}, features [{}])",
                        plan.name,
                        plan.template,
                        plan.features.join(", ")
                    );
                    Ok(ExitCode::SUCCESS)
                }
                Outcome::Cancelled => {
                    eprintln!("cancelled");
                    Ok(ExitCode::from(1))
                }
            }
        }
        Command::Rollout {
            services,
            fail,
            quiet,
        } => {
            // indicatif already hides bars when stderr is not a terminal (pipes, CI logs);
            // `--quiet` hides them explicitly.
            let target = if quiet {
                ProgressDrawTarget::hidden()
            } else {
                ProgressDrawTarget::stderr()
            };
            let report = rollout::rollout(&services, fail.as_deref(), target);
            println!(
                "succeeded: {}; failed: {}",
                report.succeeded.join(","),
                if report.failed.is_empty() {
                    "-".into()
                } else {
                    report.failed.join(",")
                }
            );
            Ok(if report.failed.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        super::Cli::command().debug_assert();
    }
}
