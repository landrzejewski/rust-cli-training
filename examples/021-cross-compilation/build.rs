//! Build script: runs on the HOST before compiling the crate. Cargo passes information about
//! the build in environment variables; `cargo:rustc-env=NAME=value` makes a value available to
//! the crate via `env!("NAME")` at compile time.

fn main() {
    // TARGET – the triple we compile FOR (may differ from HOST when cross-compiling).
    for name in ["TARGET", "HOST", "PROFILE"] {
        let value = std::env::var(name).unwrap_or_else(|_| "unknown".into());
        println!("cargo:rustc-env=BUILD_{name}={value}");
    }
    // Re-run only when this file changes (default: on every change in the package).
    println!("cargo:rerun-if-changed=build.rs");
}
