//! `platctl-rel` – step 023: the binary itself is trivial; this example is about RELEASING it.
//! See `023-release-automation.md`, `dist-workspace.toml` (workspace root), `release.toml`,
//! `cliff.toml` and `CHANGELOG.md` next to this crate.

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--version" | "-V") => println!("platctl-rel {}", env!("CARGO_PKG_VERSION")),
        _ => println!(
            "platctl-rel {}: release me with `dist build`",
            env!("CARGO_PKG_VERSION")
        ),
    }
}
