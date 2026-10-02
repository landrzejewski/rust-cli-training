//! `platctl-sysinfo` – step 021: code that adapts to the target platform.
//!
//! Prints the platform the binary was BUILT FOR (compile time) and runs on – handy to verify
//! cross-compiled artifacts (`docker run ... platctl-sysinfo`).

use std::path::PathBuf;

fn main() {
    let json = std::env::args().any(|a| a == "--json");

    let info = [
        // Values embedded at compile time by build.rs.
        ("built_for", env!("BUILD_TARGET")),
        ("built_on", env!("BUILD_HOST")),
        ("profile", env!("BUILD_PROFILE")),
        // `std::env::consts` – constants of the target (also compile time).
        ("os", std::env::consts::OS),
        ("arch", std::env::consts::ARCH),
        ("family", std::env::consts::FAMILY),
        ("exe_suffix", std::env::consts::EXE_SUFFIX),
        ("libc", libc_flavor()),
        (
            "pointer_width",
            if cfg!(target_pointer_width = "64") {
                "64"
            } else {
                "32"
            },
        ),
        (
            "endian",
            if cfg!(target_endian = "little") {
                "little"
            } else {
                "big"
            },
        ),
    ];
    let config_dir = config_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    if json {
        let fields: Vec<String> = info
            .iter()
            .map(|(k, v)| format!("\"{k}\":\"{v}\""))
            .chain(std::iter::once(format!(
                "\"config_dir\":\"{}\"",
                config_dir.replace('\\', "\\\\")
            )))
            .collect();
        println!("{{{}}}", fields.join(","));
    } else {
        for (key, value) in info {
            println!("{key:<14} {value}");
        }
        println!("{:<14} {config_dir}", "config_dir");
    }
}

/// `target_env`: gnu (glibc), musl (static-friendly), msvc (Windows), "" on macOS.
fn libc_flavor() -> &'static str {
    if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else {
        "system"
    }
}

// Conditional compilation: only ONE of these functions exists in a given build. Code for other
// platforms is not even type-checked – build every target in CI to catch errors early.

#[cfg(target_os = "windows")]
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("platctl"))
}

#[cfg(target_os = "macos")]
fn config_dir() -> Option<PathBuf> {
    // Many CLI tools prefer XDG-style ~/.config on macOS too (step 005).
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/platctl"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|dir| dir.join("platctl"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn build_script_embeds_the_target() {
        // Native test builds: TARGET == HOST.
        assert_eq!(env!("BUILD_TARGET"), env!("BUILD_HOST"));
        assert!(env!("BUILD_TARGET").contains(std::env::consts::ARCH));
    }

    #[test]
    fn config_dir_ends_with_tool_name() {
        if let Some(dir) = super::config_dir() {
            assert!(dir.ends_with("platctl"));
        }
    }
}
