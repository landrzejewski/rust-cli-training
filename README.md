# Rust – building CLI tools: examples

Practical examples for the training *"Rust – budowanie narzędzi CLI"*: professional command-line
applications, terminal UIs and platform tooling in Rust.

All examples revolve around a fictional internal platform tool, **`platctl`** – it manages
services, environments, deployments and logs of an Internal Developer Platform. Each example is
**small and self-contained**: it shows one topic, nothing more.

## How to use this repository

```bash
git branch -a                          # one branch per topic, each builds on the previous one
git switch 006-output-formats          # contains examples 001..006
cargo run -p ex006-output-formats -- --help
cargo test -p ex006-output-formats
git diff 005-config-precedence 006-output-formats   # what a topic added
```

Each topic consists of:

- `examples/NNN-topic/` – a standalone crate (package `exNNN-topic`), commented for self-study,
- `NNN-topic.md` – theory: key concepts, walkthrough of the example, "Try it" commands, exercises.

## Workspace

```
Cargo.toml            workspace: shared metadata, dependency versions, lints
examples/NNN-topic/   one crate per topic
NNN-topic.md          theory for the topic
```

Dependencies are declared once in `[workspace.dependencies]`; examples refer to them with
`{ workspace = true }`.

## Requirements

- Rust stable (edition 2024), installed with `rustup`
- a terminal (examples are tested on macOS/Linux; Windows notes where relevant)
- Docker for a few distribution topics

## Topics

| Branch | Topic |
|--------|-------|
| **Module 1 – Designing CLI tools** | |
| `001-cli-conventions` | CLI ergonomics, POSIX/GNU/docopt conventions – a parser written by hand |
| `002-commands-and-arguments` | Commands, subcommands, flags, positional and global arguments (clap derive) |
| `003-errors-exit-codes` | Errors with thiserror and anyhow, reporting, exit codes |
| `004-signals-cancellation` | Signals and cancellation: ctrlc, tokio::signal, CancellationToken |
| `005-config-precedence` | Config files, environment variables, flags – precedence and provenance (12-factor) |
| `006-output-formats` | Predictable output: human, table, JSON, JSON Lines, YAML, TSV, TTY detection |
| `007-colors` | Colors with owo-colors and nu-ansi-term, NO_COLOR, CLICOLOR_FORCE, --color |
| **Module 2 – clap and configuration in practice** | |
| `008-clap-derive-vs-builder` | Project structure, clap derive vs builder, runtime subcommands |
| `009-clap-subcommands-validation` | Subcommand enums, ValueEnum, ranges, groups, conflicts, custom value parsers |
| `010-completions-man-pages` | Shell completions (static and dynamic), man pages, Markdown docs |
| `011-config-rs-figment` | Layered configuration with config-rs and figment, dotenvy, serde |
| `012-secrets-keyring` | Secrets: OS keychain (keyring), age-encrypted files, secrecy, safe input |
| `013-files-stdin-pipes` | Files, stdin and pipes: streaming, buffering, broken pipes, NUL separators |
| **Module 3 – Interactive TUIs with ratatui and crossterm** | |
| `014-tui-immediate-mode` | ratatui basics: terminal setup/restore, immediate-mode render loop |
| `015-tui-widgets` | Widgets: Block, Tabs, List, Table, Paragraph, Chart, Gauge, Sparkline, BarChart |
| `016-tui-layouts` | Layout, Constraint, Direction, Flex; responsive layouts; components |
| `017-tui-async-events` | Keyboard and mouse, async event loop with tokio, background tasks |
| `018-tui-themes-keymaps` | Themes, accessibility, vim/emacs keymaps, configurable bindings |
| `019-progress-prompts` | Progress bars and spinners (indicatif), prompts (dialoguer, inquire), non-interactive mode |
| `020-tui-testing` | Testing: key scripts, TestBackend, insta snapshots, assert_cmd, trycmd |
| **Module 4 – Distribution, extensibility, maintenance** | |
| `021-cross-compilation` | Cross compilation: targets, cfg, build.rs, cargo-zigbuild, cross, CI matrix |
| `022-embedded-assets` | Embedded resources: include_str!/include_bytes!, rust-embed, build.rs metadata |
| `023-release-automation` | cargo-dist, cargo-release, git-cliff: archives, checksums, installers, changelog |
| `024-package-managers` | Homebrew, Scoop, Winget, AUR, cargo install/binstall, Docker images |
| `025-signing-sbom` | cosign signatures (key-based, keyless), cargo-auditable, cargo audit, CycloneDX SBOM, attestations |
| `026-self-update-channels` | self_update (GitHub, manifest feed), stable/beta channels, throttled update notices, opt-out |
| `027-plugins-wasm` | WebAssembly component plugins: WIT, wasmtime host, wit-bindgen guests, sandbox, fuel and memory limits |
| `028-plugins-dylib` | Native plugins: cdylib, C ABI vtable, libloading, panics across FFI, abi_stable |
