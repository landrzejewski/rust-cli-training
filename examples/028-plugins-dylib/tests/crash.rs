//! Integration test: runs the real `platctl-ext` binary (Cargo builds it for integration tests
//! and exposes its path as CARGO_BIN_EXE_<name>) with a plugin that aborts.

use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

#[test]
fn crashing_plugin_takes_the_host_down() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target_dir = root.join("target/dylib-plugins");
    let built = Command::new(env!("CARGO"))
        .args(["build", "-p", "platctl-ext-crash", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .unwrap();
    assert!(built.success());
    let plugin = target_dir
        .join("debug")
        .join(libloading::library_filename("crash"));

    let mut child = Command::new(env!("CARGO_BIN_EXE_platctl-ext"))
        .arg("run")
        .arg("-p")
        .arg(plugin)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"fine\ncrash\n")
        .unwrap();
    let status = child.wait().unwrap();

    // No error message, no cleanup, no exit code chosen by the host: the process was killed.
    assert!(!status.success());
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(6), "SIGABRT");
    }
}
