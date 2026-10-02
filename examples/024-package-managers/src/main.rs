//! `platctl-pkg` – the product of step 024. The interesting part is HOW it reaches users:
//! see `templates/`, `src/bin/render-manifests.rs`, `package.sh`, `Dockerfile`.

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => println!("platctl-pkg {}", env!("CARGO_PKG_VERSION")),
        _ => println!(
            "platctl-pkg {} – installed via a package manager",
            env!("CARGO_PKG_VERSION")
        ),
    }
}
