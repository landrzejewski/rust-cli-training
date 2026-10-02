//! Integration tests: start the real binaries, send signals, check exit codes and output.
//! Unix only – signals are a Unix concept (`kill`).
#![cfg(unix)]

use std::{
    process::{Command, Stdio},
    thread,
    time::Duration,
};

/// Sends a signal with the `kill` utility (avoids a dependency on `libc`/`nix`).
fn kill(pid: u32, signal: &str) {
    let status = Command::new("kill")
        .args([signal, &pid.to_string()])
        .status()
        .expect("kill");
    assert!(status.success());
}

#[test]
fn sync_tool_finishes_current_item_and_exits_with_130() {
    // Cargo builds the package's binaries for integration tests and exposes their paths.
    let child = Command::new(env!("CARGO_BIN_EXE_platctl-sync"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    thread::sleep(Duration::from_millis(600));
    kill(child.id(), "-INT");
    let output = child.wait_with_output().expect("wait");

    assert_eq!(output.status.code(), Some(130));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.starts_with("synced "), "{stdout}");
    assert!(
        !stdout.contains("synced 6/6"),
        "interrupted before the end: {stdout}"
    );
    assert!(stderr.contains("cleaned up"), "{stderr}");
}

#[test]
fn async_tool_stops_watchers_on_sigterm() {
    let child = Command::new(env!("CARGO_BIN_EXE_platctl-watch"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    thread::sleep(Duration::from_millis(1000));
    kill(child.id(), "-TERM");
    let output = child.wait_with_output().expect("wait");

    assert_eq!(output.status.code(), Some(130));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SIGTERM received"), "{stderr}");
    assert!(stderr.contains("all watchers stopped"), "{stderr}");
    assert_eq!(
        stderr.matches("watcher stopped after").count(),
        3,
        "{stderr}"
    );
}
