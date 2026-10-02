//! Behavior in pipes – tested by running the real binary.

use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};

fn logq() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_platctl-logq"));
    // Paths in tests are relative to the package directory.
    command.current_dir(Path::new(env!("CARGO_MANIFEST_DIR")));
    command
}

#[test]
fn filters_stdin_and_reports_match_via_exit_code() {
    let mut child = logq()
        .args(["--level", "error"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"level=info service=a\nlevel=error service=b\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "level=error service=b\n"
    );
}

#[test]
fn no_match_is_exit_code_1_and_missing_file_is_reported() {
    let output = logq()
        .args(["--service", "nope", "logs/billing.log", "missing.log"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing.log"));
}

#[test]
fn files_with_matches_separated_by_nul() {
    let output = logq()
        .args([
            "-l",
            "-0",
            "--level",
            "error",
            "logs/billing.log",
            "logs/search.log",
            "logs/gateway.log",
        ])
        .output()
        .unwrap();

    assert_eq!(output.stdout, b"logs/billing.log\0logs/gateway.log\0");
}

#[test]
fn closed_pipe_is_not_an_error() {
    // Produce a lot of output and close the read end after the first bytes – like `| head -1`.
    let mut child = logq()
        .args(["-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let line = b"level=info service=billing msg=\"filler\"\n".repeat(20_000);
        let _ = stdin.write_all(&line);
    });
    let mut first = [0u8; 16];
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut first)
        .unwrap();
    drop(child.stdout.take()); // close the pipe
    let _ = writer.join();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success(), "{:?}", output.status);
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
