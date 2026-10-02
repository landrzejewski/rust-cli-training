//! `platctl-live` – step 017: asynchronous event loop with tokio.
//!
//! - keyboard and mouse from crossterm's async `EventStream`,
//! - a 100 ms tick for animations,
//! - background tasks (log stream, deployments) sending messages through a channel,
//! - one `select`-free main loop: `draw` -> `recv().await` -> `update` -> run command.

mod app;
mod events;

use std::{io, time::Duration};

use app::{App, Command};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use events::AppEvent;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    // Mouse reporting is off by default (it disables the terminal's own text selection).
    execute!(io::stdout(), EnableMouseCapture)?;

    let result = run(&mut terminal).await;

    // Restore in reverse order – also when `run` failed.
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}

async fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    // Bounded channel: if the UI can't keep up, producers wait instead of eating memory.
    let (tx, mut rx) = mpsc::channel::<AppEvent>(256);
    let token = CancellationToken::new();

    events::spawn_sources(tx.clone(), token.clone(), Duration::from_millis(100));
    events::spawn_log_streamer(tx.clone(), token.clone());

    let mut app = App::new();
    loop {
        terminal.draw(|frame| app.render(frame))?;

        // Wait for the next event from ANY source. The terminal is redrawn after every event
        // (ticks included) – cheap thanks to ratatui's diffing. Drawing only when something
        // changed is an easy optimization for heavier UIs.
        let Some(event) = rx.recv().await else {
            break;
        };
        match app.update(event) {
            Some(Command::Quit) => break,
            Some(Command::StartDeployment(service)) => {
                events::spawn_deployment(service, tx.clone(), token.child_token());
            }
            None => {}
        }
    }

    // Stop every background task; they observe the token in their `select!`.
    token.cancel();
    Ok(())
}
