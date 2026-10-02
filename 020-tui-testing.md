# 020 – Testing interactions with TestBackend and insta snapshots; CLI tests

## Goal

Test terminal applications without a terminal: drive state with key scripts, render into an
in-memory backend and compare with reviewed snapshots; test the binary end-to-end with
`assert_cmd` and executable documentation with `trycmd`.

Example: `examples/020-tui-testing` – `platctl-incidents` (TUI + `list`/`ack` subcommands) with
three kinds of tests.

## Key concepts

### Testable structure

```
App::handle_key(KeyEvent)   state transitions       → unit tests, no terminal
App::render(&mut Frame)     pure drawing from state → TestBackend + snapshots
main()                      terminal + event loop   → thin, covered by CLI tests / manual runs
```

Expose the app as a library (`src/lib.rs`) so integration tests in `tests/` can use it.

### Key scripts

```rust
pub fn keys("jj<enter>a<esc>") -> Vec<KeyEvent>      // tiny DSL, readable tests

let mut app = App::new(sample_incidents());
for key in keys("ja") { app.handle_key(key); }
assert!(app.incidents[1].acknowledged);
```

### TestBackend

```rust
let mut terminal = Terminal::new(TestBackend::new(60, 7))?;     // fixed size → deterministic
terminal.draw(|frame| app.render(frame))?;
let cell = &terminal.backend().buffer()[(5, 1)];                // (column, row)
assert_eq!(cell.fg, Color::Red);                                 // styles: targeted assertions
```

Test several sizes (80×24, very narrow) – layout bugs show up at the edges.

### insta snapshots

```rust
insta::assert_snapshot!(terminal.backend());    // TestBackend: Display = screen text
```

```
"┌ Incidents ───────────────────────────────────────────────┐"
"│> ! #102 search         p95 latency above 1s              │"
```

- first run writes `tests/snapshots/<file>__<test>.snap` (commit it, review it like code),
- a later difference fails the test and shows a diff (`-old snapshot / +new results`),
- update intentionally: `cargo insta test` + `cargo insta review` (install `cargo-insta`) or
  `INSTA_UPDATE=always cargo test`,
- text snapshots don't contain colors → assert important styles separately,
- insta also snapshots JSON/YAML/Debug output (`assert_json_snapshot!`, `assert_debug_snapshot!`).

### assert_cmd – the binary as a black box

```rust
Command::cargo_bin("platctl-incidents")?
    .args(["ack", "999"])
    .assert()
    .code(1)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("#999 not found"));
```

Verifies the CLI contract from steps 001–013: exit codes, stdout vs stderr, JSON validity.
Also: `.write_stdin(..)`, `.env(..)`, `.current_dir(..)`, `.timeout(..)`.

### trycmd – executable documentation

````markdown
```console
$ platctl-incidents ack 999
? 1
error: incident #999 not found
```
````

Markdown/TOML files describe commands and expected output (`? 1` = exit code; `...` and
`[..]` wildcards for variable parts). Great for help text and examples in docs;
`TRYCMD=overwrite cargo test` refreshes after intended changes. `snapbox` is the underlying library.

### What else

- PTY-level tests (`expectrl`, `portable-pty`) for real terminal behavior – slower, use sparingly,
- `cargo nextest` for faster parallel test runs,
- CI: snapshots must exist (`INSTA_UPDATE=no` / `CI=true` makes insta fail instead of writing).

## Example walkthrough

- `src/lib.rs` – `App` (list/details views, acknowledge), `render`, `keys()` helper.
- `src/main.rs` – TUI by default, `list [-o json]`, `ack <ID>` with exit codes.
- `tests/tui.rs` – key-script state tests, two insta snapshots, style assertion.
- `tests/snapshots/*.snap` – reviewed snapshots.
- `tests/cli.rs` – assert_cmd: JSON, error exit code and streams, usage error.
- `tests/cmd/incidents.md` + `tests/trycmd.rs` – executable command reference.

## Try it

```bash
cargo test -p ex020-tui-testing
cat examples/020-tui-testing/tests/snapshots/*.snap
# change a label in render(), run the tests again -> snapshot diff
cargo install cargo-insta && cargo insta review
cargo run -p ex020-tui-testing             # the TUI itself
```

## Exercises

1. Add a snapshot of the list view at 30 columns and fix any truncation problems you find.
2. Add an `assert_json_snapshot!` test for `list -o json` output.
3. Add a trycmd case for `list -o json` using `...` wildcards for the middle of the document.
