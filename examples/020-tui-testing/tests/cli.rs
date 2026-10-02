//! CLI tests with assert_cmd: run the real binary, assert exit code, stdout and stderr.

use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    // Finds the binary built by cargo for this package.
    Command::cargo_bin("platctl-incidents").expect("binary is built by cargo test")
}

#[test]
fn list_json_is_valid_and_complete() {
    let output = cmd()
        .args(["list", "-o", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let incidents: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(incidents.as_array().map(Vec::len), Some(3));
    assert_eq!(incidents[0]["severity"], "critical");
}

#[test]
fn unknown_incident_fails_with_message_on_stderr() {
    cmd()
        .args(["ack", "999"])
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("#999 not found"));
}

#[test]
fn usage_error_exit_code() {
    cmd()
        .args(["ack", "abc"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("invalid value"));
}
