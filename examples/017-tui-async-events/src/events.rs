//! One event stream for the whole application.
//!
//! Several asynchronous sources – terminal input, a timer, background tasks – are funnelled into
//! a single `mpsc` channel of `AppEvent`s. The main loop then only awaits `rx.recv()`; it never
//! blocks on any particular source, so the UI stays responsive while work happens elsewhere.
//!
//! ```text
//! crossterm EventStream ─┐
//! tick interval ─────────┼──► mpsc::Sender<AppEvent> ──► main loop ──► app.update(event)
//! background tasks ──────┘
//! ```

use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyEvent, MouseEvent};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize,
    /// Periodic timer – animations, polling, "x seconds ago" labels.
    Tick,
    /// A log line from the background log streamer.
    Log(String),
    /// Progress of a deployment task, 0..=100.
    DeployProgress {
        service: String,
        percent: u16,
    },
    DeployFinished {
        service: String,
    },
}

/// Spawns the terminal reader and the ticker. Both stop when `token` is cancelled.
pub fn spawn_sources(tx: mpsc::Sender<AppEvent>, token: CancellationToken, tick: Duration) {
    let input_tx = tx.clone();
    let input_token = token.clone();
    tokio::spawn(async move {
        // `EventStream` – crossterm's async reader (feature `event-stream`); `next()` from
        // `StreamExt` yields `Option<io::Result<Event>>`.
        let mut events = EventStream::new();
        loop {
            let event = tokio::select! {
                () = input_token.cancelled() => break,
                event = events.next() => event,
            };
            let app_event = match event {
                Some(Ok(Event::Key(key))) => AppEvent::Key(key),
                Some(Ok(Event::Mouse(mouse))) => AppEvent::Mouse(mouse),
                Some(Ok(Event::Resize(_, _))) => AppEvent::Resize,
                Some(Ok(_)) => continue, // focus, paste – not used here
                Some(Err(_)) | None => break,
            };
            if input_tx.send(app_event).await.is_err() {
                break; // receiver gone – app is shutting down
            }
        }
    });

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick);
        loop {
            tokio::select! {
                () = token.cancelled() => break,
                _ = interval.tick() => {
                    if tx.send(AppEvent::Tick).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// Background producer: simulated log stream from the platform.
pub fn spawn_log_streamer(tx: mpsc::Sender<AppEvent>, token: CancellationToken) {
    tokio::spawn(async move {
        let messages = [
            "billing   invoice created",
            "search    index updated",
            "gateway   upstream timeout (retrying)",
            "billing   payment captured",
            "reports   export finished",
        ];
        let mut counter = 0usize;
        loop {
            tokio::select! {
                () = token.cancelled() => break,
                () = tokio::time::sleep(Duration::from_millis(700)) => {
                    let line = format!("#{counter:04} {}", messages[counter % messages.len()]);
                    counter += 1;
                    if tx.send(AppEvent::Log(line)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// A "command" started by the update function: simulated deployment reporting progress.
pub fn spawn_deployment(service: String, tx: mpsc::Sender<AppEvent>, token: CancellationToken) {
    tokio::spawn(async move {
        for percent in (0..=100).step_by(5) {
            tokio::select! {
                () = token.cancelled() => return,
                () = tokio::time::sleep(Duration::from_millis(120)) => {}
            }
            let event = AppEvent::DeployProgress {
                service: service.clone(),
                percent,
            };
            if tx.send(event).await.is_err() {
                return;
            }
        }
        let _ = tx.send(AppEvent::DeployFinished { service }).await;
    });
}
