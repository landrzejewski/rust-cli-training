# 004 – System signals and cancellation: ctrlc and tokio::signal

## Goal

Stop long-running CLI work cleanly on Ctrl+C or `kill`: finish or roll back the current step,
clean up, report, exit with the conventional code – and still let an impatient user abort at once.

Example: `examples/004-signals-cancellation` with two binaries:
- `platctl-sync` – synchronous loop + `ctrlc`,
- `platctl-watch` – async watchers + `tokio::signal` + `CancellationToken`.

## Key concepts

### Signals a CLI meets

| Signal | Sent by | Default action |
|--------|---------|----------------|
| SIGINT (2) | Ctrl+C in the terminal (to the whole foreground process group) | terminate |
| SIGTERM (15) | `kill PID`, `docker stop`, systemd, Kubernetes | terminate |
| SIGHUP (1) | terminal closed | terminate |
| SIGPIPE (13) | writing to a closed pipe (step 013) | terminate (Rust ignores it → `EPIPE` error) |
| SIGKILL (9) | `kill -9` | terminate – cannot be caught |
| SIGQUIT (3) | Ctrl+\ | core dump |

Windows has no signals; Ctrl+C / Ctrl+Break / console close are console events – both `ctrlc` and
`tokio::signal::ctrl_c` handle them.

Exit status after an interrupt: **128 + signal number** (130 for SIGINT, 143 for SIGTERM) – what a
shell reports for a killed process and what scripts expect.

### Graceful vs forced

1. first signal → set a flag / cancel a token, stop starting new work,
2. finish or roll back the current unit, clean up (RAII guards: `Drop` of temp dirs, lock files),
3. print a summary, exit 130,
4. second signal or a deadline → exit immediately (`process::exit` – destructors don't run).

The handler itself only sets state; the work loop checks it at safe points (cancellation points).

### Sync: `ctrlc`

```rust
let interrupts = Arc::new(AtomicUsize::new(0));
let flag = Arc::clone(&interrupts);
ctrlc::set_handler(move || {
    if flag.fetch_add(1, Ordering::SeqCst) > 0 { std::process::exit(130); }  // second Ctrl+C
})?;

for item in items {
    if interrupts.load(Ordering::SeqCst) > 0 { break; }   // cancellation point
    process(item);
}
```

Feature `termination` adds SIGTERM/SIGHUP. Only one handler per process.

### Async: `tokio::signal` + `CancellationToken`

```rust
let token = CancellationToken::new();
tasks.spawn(watch(service, token.clone()));
shutdown_signal().await;                 // ctrl_c() or unix SignalKind::terminate()
token.cancel();                          // all clones see it

tokio::select! {                         // graceful phase with escape hatches
    () = join_all_tasks => {},
    _ = shutdown_signal() => {},          // second signal
    () = sleep(deadline) => {},           // stuck task
}

// inside a task
tokio::select! { biased;
    () = token.cancelled() => break,
    _ = interval.tick() => do_work().await,
}
```

Futures are cancelled by dropping them – keep state consistent between `.await` points
(cancel-safety). `child_token()` creates scoped cancellation (cancel a sub-tree only).

### Testing signal handling

Integration tests in `tests/` start the real binary (`env!("CARGO_BIN_EXE_<name>")`), send a
signal with `kill`, and assert exit code and output.

## Example walkthrough

- `src/bin/platctl-sync.rs` – atomic counter, quick handler, cancellation point per artifact,
  RAII `CleanupDir`, summary on stdout, exit 130; second Ctrl+C → immediate exit.
- `src/bin/platctl-watch.rs` – `JoinSet` of watchers, `CancellationToken`, SIGINT/SIGTERM future,
  graceful phase with second-signal and deadline branches.
- `tests/signals.rs` – SIGINT to the sync tool, SIGTERM to the async tool.

## Try it

```bash
cargo test -p ex004-signals-cancellation
cargo run -p ex004-signals-cancellation --bin platctl-sync      # press Ctrl+C once, then try twice
echo $?                                                          # 130
cargo run -p ex004-signals-cancellation --bin platctl-watch     # Ctrl+C -> graceful stop of 3 watchers
target/debug/platctl-watch & sleep 2; kill -TERM $!; wait $!; echo $?
target/debug/platctl-sync | head -1     # what happens to the summary? (preview of step 013)
```

## Exercises

1. Make `platctl-sync` resumable: write progress to a state file on interrupt and skip done artifacts on the next run.
2. In `platctl-watch` use `token.child_token()` so that `s` + Enter stops only one watcher.
3. Return 143 when stopped by SIGTERM and 130 for SIGINT.
