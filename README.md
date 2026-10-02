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
