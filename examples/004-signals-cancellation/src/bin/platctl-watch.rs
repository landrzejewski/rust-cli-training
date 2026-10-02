//! `platctl-watch` – async tool: several concurrent watchers, stopped by SIGINT/SIGTERM.
//!
//! - `tokio::signal` turns signals into futures that can be awaited / `select!`ed,
//! - `CancellationToken` (tokio-util) broadcasts "stop" to every task,
//! - tasks finish their current step and return; `main` waits with a deadline,
//! - a second signal (or the deadline) ends the process immediately.

use std::{process::ExitCode, time::Duration};

use tokio::{task::JoinSet, time};
use tokio_util::sync::CancellationToken;

const SERVICES: [&str; 3] = ["billing", "search", "notifications"];
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(3);
const EXIT_INTERRUPTED: u8 = 130;

#[tokio::main]
async fn main() -> ExitCode {
    let token = CancellationToken::new();

    // `JoinSet` – a group of spawned tasks we can await together.
    let mut watchers = JoinSet::new();
    for service in SERVICES {
        // Each task gets its own clone; cancelling any clone cancels all of them.
        watchers.spawn(watch(service, token.clone()));
    }

    // Wait for the first signal.
    let signal = shutdown_signal().await;
    eprintln!("\n{signal} received – stopping watchers (send it again to force)");
    token.cancel();

    // Graceful phase: wait until all watchers return, but not forever – and not if the user
    // insists with a second signal.
    let graceful = async {
        while let Some(result) = watchers.join_next().await {
            if let Err(error) = result {
                eprintln!("watcher failed: {error}");
            }
        }
    };
    tokio::select! {
        () = graceful => eprintln!("all watchers stopped"),
        _ = shutdown_signal() => eprintln!("forced shutdown"),
        () = time::sleep(SHUTDOWN_DEADLINE) => eprintln!("shutdown deadline exceeded"),
    }
    // Returning from `main` drops the runtime; tasks still running are cancelled.
    ExitCode::from(EXIT_INTERRUPTED)
}

/// Periodically "checks" a service until cancelled.
async fn watch(service: &'static str, token: CancellationToken) {
    let mut interval = time::interval(Duration::from_millis(700));
    let mut checks = 0u32;
    loop {
        tokio::select! {
            // `biased` – check cancellation first when both are ready.
            biased;
            () = token.cancelled() => break,
            _ = interval.tick() => {
                checks += 1;
                println!("{service}: healthy (check #{checks})");
            }
        }
    }
    // Per-task cleanup after cancellation (flush, close connections...).
    time::sleep(Duration::from_millis(200)).await;
    eprintln!("{service}: watcher stopped after {checks} checks");
}

/// Completes on Ctrl+C (SIGINT) or – on Unix – SIGTERM (`kill`, `docker stop`, systemd).
async fn shutdown_signal() -> &'static str {
    let ctrl_c = async {
        // Errors only if the handler can't be installed – then wait forever instead of failing.
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => "SIGINT",
        () = terminate => "SIGTERM",
    }
}
