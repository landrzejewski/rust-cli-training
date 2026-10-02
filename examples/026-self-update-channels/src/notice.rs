//! "A new version is available" notice – polite by design:
//! - at most once per interval (stamp file, `self_update::check_interval::UpdateCheckGuard`),
//! - never in CI, never when stderr is not a terminal (scripts, pipes), opt-out via env/flag,
//! - runs in a background thread and never fails or slows down the actual command.

use std::{
    env,
    io::{self, IsTerminal},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use self_update::check_interval::UpdateCheckGuard;
use semver::Version;

use crate::{BIN, VERSION, channel::State, source::Source};

const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// How long the command waits for the check after finishing its own work.
const GRACE: Duration = Duration::from_millis(500);

pub struct PendingNotice {
    result: Receiver<Version>,
}

pub fn start(source: &Source, state: &State, opted_out: bool) -> Option<PendingNotice> {
    if opted_out || env::var_os("CI").is_some() || !io::stderr().is_terminal() {
        return None;
    }
    let guard = UpdateCheckGuard::new(state.last_check_stamp(), CHECK_INTERVAL);
    if !guard.should_check().unwrap_or(false) {
        return None;
    }
    let (sender, result) = mpsc::channel();
    let source = source.clone();
    let channel = state.channel();
    thread::spawn(move || {
        // Stamp even if the check fails: an offline laptop must not retry on every run.
        let _ = guard.record_check();
        if let Ok(Some(newest)) = source.newest(channel) {
            let _ = sender.send(newest);
        }
    });
    Some(PendingNotice { result })
}

impl PendingNotice {
    /// Too slow → no notice this time. (A sturdier variant stores the result in the state dir
    /// and prints it on the next run.)
    pub fn print_if_ready(self) {
        if let Ok(newest) = self.result.recv_timeout(GRACE) {
            eprintln!(
                "\nA new release of {BIN} is available: {VERSION} -> {newest}\n\
                 Run `{BIN} update` to install it (PLATCTL_NO_UPDATE_CHECK=1 disables this check)."
            );
        }
    }
}
