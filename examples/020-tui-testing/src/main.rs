//! `platctl-incidents` binary: TUI by default, plus non-interactive subcommands for scripts.
//!
//! ```text
//! platctl-incidents                  interactive TUI
//! platctl-incidents list [-o json]   print incidents
//! platctl-incidents ack <ID>         acknowledge (simulated)
//! ```

use std::{io, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use crossterm::event::{self, Event, KeyEventKind};
use ex020_tui_testing::{App, sample_incidents};

/// Browse and acknowledge platform incidents.
#[derive(Debug, Parser)]
#[command(name = "platctl-incidents", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print incidents
    List {
        #[arg(short, long, value_enum, default_value_t = Output::Human)]
        output: Output,
    },
    /// Acknowledge an incident
    Ack {
        /// Incident id
        id: u32,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Output {
    Human,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let incidents = sample_incidents();
    match cli.command {
        None => match ratatui::run(|terminal| -> io::Result<()> {
            let mut app = App::new(incidents);
            while !app.should_quit {
                terminal.draw(|frame| app.render(frame))?;
                if let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press
                {
                    app.handle_key(key);
                }
            }
            Ok(())
        }) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        },
        Some(Command::List { output }) => {
            match output {
                Output::Human => {
                    for i in &incidents {
                        let status = if i.acknowledged {
                            "acknowledged"
                        } else {
                            "open"
                        };
                        println!(
                            "#{} {:<14} {:<9} {status:<13} {}",
                            i.id,
                            i.service,
                            format!("{:?}", i.severity).to_lowercase(),
                            i.title
                        );
                    }
                }
                Output::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&incidents).expect("serializable")
                ),
            }
            ExitCode::SUCCESS
        }
        Some(Command::Ack { id }) => match incidents.iter().find(|i| i.id == id) {
            Some(incident) if incident.acknowledged => {
                eprintln!("incident #{id} is already acknowledged");
                ExitCode::SUCCESS
            }
            Some(_) => {
                println!("acknowledged #{id}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("error: incident #{id} not found");
                ExitCode::from(1)
            }
        },
    }
}
