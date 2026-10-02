# 003 – Error handling with Result, anyhow and thiserror; exit codes

## Goal

Report failures so that humans understand what went wrong and how to fix it, and scripts can
react to *which* failure happened – via the exit code.

Example: `examples/003-errors-exit-codes` – `platctl-deploy <MANIFEST>` reads a manifest and
"deploys" it; each failure type has its own message, hint and exit code.

## Key concepts

### Two kinds of error types

| | `thiserror` | `anyhow` |
|---|---|---|
| What | derive for **your own** error enums | one dynamic error type `anyhow::Error` |
| Caller can `match` | yes | only via `downcast_ref` |
| Use in | modules with a contract (`manifest`, `platform`), libraries | application glue, `main`, commands |
| Extra | `#[source]`/`#[from]` build the cause chain | `.context()` adds "while doing X" layers |

```rust
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("cannot read manifest {path}")]
    Read { path: PathBuf, #[source] source: std::io::Error },
    #[error("{path}:{line}: invalid number for `{key}`")]
    InvalidNumber { path: PathBuf, line: usize, key: String, #[source] source: ParseIntError },
}

fn run(cli: &Cli) -> anyhow::Result<()> {
    let manifest = manifest::load(&cli.manifest)
        .with_context(|| format!("invalid deployment manifest {}", cli.manifest.display()))?;
    ...
}
```

Don't format the cause into the message (`"cannot read: {source}"`) **and** expose it as `source` –
it gets printed twice. Either one or the other.

### Printing errors for humans

```
error: invalid deployment manifest manifests/broken.manifest
  caused by: manifests/broken.manifest:2: invalid number for `replicas`
  caused by: invalid digit found in string
  hint: ...
```

- stderr, one cause per line (`error.chain()`), lowercase, no trailing period (cargo/git style),
- location for data errors (`file:line`), the offending value, and an actionable **hint**,
- `--verbose` / `RUST_BACKTRACE=1` for developer details (`{error:?}` on `anyhow::Error`),
- no panics for expected failures; a panic means a bug.

Returning `anyhow::Result<()>` from `main` also works (prints `Error: ...` + `Caused by:` with
exit code 1) – fine for prototypes; real tools want control over format and exit code.
Fancier reporters: `miette` (diagnostics with source snippets), `color-eyre`.

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | success |
| 1 | generic failure |
| 2 | usage error (clap) |
| 64–78 | `sysexits.h` vocabulary – here: 65 DATAERR, 66 NOINPUT, 69 UNAVAILABLE, 75 TEMPFAIL, 77 NOPERM |
| 126 / 127 | shell: not executable / command not found – don't use |
| 128 + N | terminated by signal N (130 = Ctrl+C, step 004) – don't use |

Only the low 8 bits reach the parent (`exit(256)` = 0!). Document your codes; distinguish at least
"fix your input" from "retry later" (TEMPFAIL lets CI retry automatically).

Mapping typed errors behind `anyhow`:

```rust
for cause in error.chain() {
    if let Some(e) = cause.downcast_ref::<PlatformError>() { return match e { ... } }
}
```

### `ExitCode` vs `process::exit`

`fn main() -> ExitCode` returns normally – destructors run, buffers flush. `std::process::exit(n)`
terminates immediately (no destructors) – avoid it except at the very end.

## Example walkthrough

- `src/manifest.rs` – `ManifestError` (thiserror) with paths, line numbers and sources; tests check `source()`.
- `src/platform.rs` – `PlatformError` (unknown service, permission denied, transient timeout).
- `src/main.rs` – `run() -> anyhow::Result` with context, `report()` printing the chain + hints.
- `src/exit_codes.rs` – documented codes, mapping via `downcast_ref`; tests.

## Try it

```bash
cd examples/003-errors-exit-codes && cargo build
P=../../target/debug/platctl-deploy
$P manifests/billing.manifest; echo $?                     # 0, deployment id on stdout
$P manifests/missing.manifest; echo $?                     # 66 + hint
$P manifests/broken.manifest; echo $?                      # 65, file:line
$P manifests/unknown.manifest; echo $?                     # 69
$P manifests/billing.manifest -e prod; echo $?             # 77 + hint
$P manifests/billing.manifest --simulate-timeout; echo $?  # 75 (retryable)
RUST_BACKTRACE=1 $P manifests/broken.manifest -v           # debug output
until $P manifests/billing.manifest --simulate-timeout; [ $? -ne 75 ]; do sleep 1; done   # retry loop pattern
```

## Exercises

1. Add `#[error(transparent)]` variant `Io(#[from] std::io::Error)` and discuss when transparent errors lose useful context.
2. Replace `report` with `miette` and show the broken manifest line with a pointer to the bad value.
3. Add `--retries <N>` that retries only `PlatformError::Timeout`.
