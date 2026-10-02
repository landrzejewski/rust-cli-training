# 021 – Compiling for different systems (cross compilation)

## Goal

Produce binaries for Linux, macOS and Windows on x86_64 and ARM from one codebase – locally and
in CI – and write code that adapts to the target platform.

Example: `examples/021-cross-compilation` – `platctl-sysinfo` prints the target it was built for;
`build-all.sh` builds 5 artifacts on one Mac.

## Key concepts

### Target triples

`<arch>-<vendor>-<os>-<env>`:

| Triple | Notes |
|--------|-------|
| `x86_64-unknown-linux-gnu` | glibc – dynamic, needs a compatible glibc on the host |
| `x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl` | static by default – runs on any distro, in `scratch` images |
| `x86_64-apple-darwin` / `aarch64-apple-darwin` | Intel / Apple Silicon; combine with `lipo` into a universal binary |
| `x86_64-pc-windows-msvc` / `aarch64-pc-windows-msvc` | standard on Windows, needs MSVC (or `cargo-xwin`) |
| `x86_64-pc-windows-gnu` | MinGW ABI – easy to cross-compile, slightly less common |

```bash
rustup target add aarch64-unknown-linux-musl      # standard library for the target
cargo build --release --target aarch64-unknown-linux-musl
# artifacts: target/<triple>/release/
```

### The real problem: linking and C code

Rust compiles to any target; the **linker** and any **C dependencies** need a toolchain for the
target. Options:

| Tool | How | Pros / cons |
|------|-----|-------------|
| native CI runners | build each OS on its own runner | most reliable; macOS/Windows runners cost more |
| `cargo-zigbuild` | zig as C compiler + linker for every target | no Docker, fast, glibc version selectable (`--target x86_64-unknown-linux-gnu.2.17`); macOS targets need the SDK |
| `cross` | cargo inside Docker images with toolchains (`Cross.toml`) | handles C deps (`pre-build` apt installs), can run tests in QEMU; needs Docker |
| system cross toolchain | `apt install gcc-aarch64-linux-gnu` + `linker = ...` in `.cargo/config.toml` | no extra tools; one toolchain per target |
| `cargo-xwin` | MSVC targets from Linux/macOS | downloads Windows SDK headers |

Keep dependencies pure Rust where possible: `rustls` instead of OpenSSL, `rusqlite` with `bundled` etc.

### Platform-specific code

```rust
#[cfg(target_os = "windows")]   fn config_dir() -> Option<PathBuf> { ... %APPDATA% ... }
#[cfg(target_os = "macos")]     fn config_dir() -> Option<PathBuf> { ... }
#[cfg(all(unix, not(target_os = "macos")))] fn config_dir() -> Option<PathBuf> { ... }

if cfg!(target_env = "musl") { ... }          // expression form: both branches type-checked
std::env::consts::{OS, ARCH, FAMILY, EXE_SUFFIX}
```

`#[cfg]` removes code for other targets – it isn't even type-checked, so build all targets in CI.

### build.rs and target information

```rust
// build.rs runs on the HOST; TARGET/HOST/PROFILE describe the build
println!("cargo:rustc-env=BUILD_TARGET={}", std::env::var("TARGET")?);
// main.rs
env!("BUILD_TARGET")
```

### Static linking

- musl targets: static by default (`+crt-static`), ideal for containers and old distros,
- glibc: static linking is discouraged (NSS, `dlopen`); target an old glibc instead (zigbuild `.2.17`),
- Windows MSVC: `-C target-feature=+crt-static` avoids requiring the VC++ redistributable,
- macOS: system libraries are always dynamic; static Rust code is the norm.

### CI matrix

`ci/github-actions-build.yml` – native runners for macOS/Windows, zigbuild for Linux musl,
`rust-cache`, artifacts per target. Step 023 replaces this with `cargo-dist`, which generates such
workflows (plus archives, checksums, installers).

## Example walkthrough

- `build.rs` – embeds `TARGET`, `HOST`, `PROFILE`.
- `src/main.rs` – target info (consts, `cfg!`), per-OS `config_dir` with `#[cfg]`, `--json`.
- `build-all.sh` – macOS arm64 + x86_64 → universal via `lipo`; Linux musl (arm64, x86_64) and
  Windows GNU via `cargo zigbuild`.
- `Cross.toml`, `cargo-config.example.toml`, `ci/github-actions-build.yml` – reference configs.

## Try it

```bash
rustup target add x86_64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl x86_64-pc-windows-gnu
pip install ziglang && cargo install cargo-zigbuild --locked
examples/021-cross-compilation/build-all.sh
file dist/platctl-sysinfo/*
dist/platctl-sysinfo/platctl-sysinfo-macos-universal
arch -x86_64 dist/platctl-sysinfo/platctl-sysinfo-macos-universal          # Rosetta (Apple Silicon)
docker run --rm -v "$PWD/dist/platctl-sysinfo":/b alpine /b/platctl-sysinfo-aarch64-unknown-linux-musl --json
docker run --rm --platform linux/amd64 -v "$PWD/dist/platctl-sysinfo":/b busybox /b/platctl-sysinfo-x86_64-unknown-linux-musl
```

## Exercises

1. Build `x86_64-unknown-linux-gnu.2.17` with zigbuild and check the required glibc with `objdump -T | grep GLIBC`.
2. Install `cross` and build `aarch64-unknown-linux-gnu`; run the tests under QEMU with `cross test`.
3. Add a dependency with C code (e.g. `rusqlite` with `bundled`) and see which tools still cross-compile it.
