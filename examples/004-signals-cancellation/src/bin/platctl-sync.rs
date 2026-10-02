//! `platctl-sync` – synchronous work loop interrupted with Ctrl+C (crate `ctrlc`).
//!
//! Default behaviour of SIGINT (Ctrl+C) is to kill the process immediately: half-written files,
//! temporary files left behind, no summary. A CLI doing real work should:
//! 1. catch the signal,
//! 2. finish (or roll back) the current unit of work,
//! 3. clean up (temporary files, locks),
//! 4. report what was done and exit with 130 (= 128 + SIGINT, the shell convention),
//! 5. on a SECOND Ctrl+C give up immediately – the user is impatient for a reason.

use std::{
    fs,
    path::PathBuf,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

const ARTIFACTS: [&str; 6] = [
    "billing-1.4.2.tar.gz",
    "search-2.0.0.tar.gz",
    "notifications-0.9.3.tar.gz",
    "gateway-3.1.0.tar.gz",
    "auth-1.0.7.tar.gz",
    "reports-0.4.0.tar.gz",
];

/// 128 + signal number (SIGINT = 2) – what shells report for a process killed by Ctrl+C.
const EXIT_INTERRUPTED: u8 = 130;

fn main() -> ExitCode {
    // Number of received interrupt signals, shared between the handler and the main loop.
    // An atomic is enough – no locking, safe to touch from the handler thread.
    let interrupts = Arc::new(AtomicUsize::new(0));

    let handler_interrupts = Arc::clone(&interrupts);
    // `ctrlc::set_handler` runs the closure on a dedicated thread when SIGINT arrives
    // (with the `termination` feature also SIGTERM and SIGHUP; on Windows Ctrl+C / Ctrl+Break).
    // The handler must be quick: set a flag, don't do the cleanup here.
    let result = ctrlc::set_handler(move || {
        let previous = handler_interrupts.fetch_add(1, Ordering::SeqCst);
        if previous == 0 {
            eprintln!(
                "\ninterrupt received – finishing the current artifact (Ctrl+C again to abort)"
            );
        } else {
            // Second signal: exit right away. `process::exit` skips destructors – acceptable
            // here, the user explicitly asked to stop NOW.
            eprintln!("\naborted");
            std::process::exit(i32::from(EXIT_INTERRUPTED));
        }
    });
    if let Err(error) = result {
        eprintln!("error: cannot install signal handler: {error}");
        return ExitCode::FAILURE;
    }

    let staging = std::env::temp_dir().join(format!("platctl-sync-{}", std::process::id()));
    if let Err(error) = fs::create_dir_all(&staging) {
        eprintln!("error: cannot create {}: {error}", staging.display());
        return ExitCode::FAILURE;
    }
    // RAII guard: removes the staging directory when it goes out of scope – on success, on
    // error and on the graceful interrupt path alike.
    let _cleanup = CleanupDir(staging.clone());

    let mut synced = 0;
    for artifact in ARTIFACTS {
        // Check the flag between units of work – the cancellation points of a sync program.
        if interrupts.load(Ordering::SeqCst) > 0 {
            break;
        }
        eprint!("syncing {artifact} ... ");
        // Simulated slow transfer into the staging directory.
        thread::sleep(Duration::from_millis(400));
        let _ = fs::write(staging.join(artifact), b"data");
        eprintln!("done");
        synced += 1;
    }

    // Summary on stdout – useful for scripts even after an interrupt.
    println!("synced {synced}/{} artifacts", ARTIFACTS.len());
    if interrupts.load(Ordering::SeqCst) > 0 {
        eprintln!("interrupted");
        ExitCode::from(EXIT_INTERRUPTED)
    } else {
        ExitCode::SUCCESS
    }
}

/// Deletes the directory on drop.
struct CleanupDir(PathBuf);

impl Drop for CleanupDir {
    fn drop(&mut self) {
        if fs::remove_dir_all(&self.0).is_ok() {
            eprintln!("cleaned up {}", self.0.display());
        }
    }
}
