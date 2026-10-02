# 019 – Progress bars and spinners (indicatif); prompts (dialoguer, inquire)

## Goal

Give feedback during long operations and ask the user for input – without breaking scripts:
progress on stderr, prompts only when a human is present, flags for everything.

Example: `examples/019-progress-prompts` – `platctl-setup init` (prompts) and `platctl-setup rollout` (progress).

## Key concepts

### Rules for interactive features

| Rule | Why |
|------|-----|
| every prompt has a flag equivalent (`--name`, `--template`, `--yes`) | CI, scripts, reproducibility |
| prompt only if stdin AND stderr are terminals | a CI job waiting for input hangs forever |
| without a terminal: fail fast, list **all** missing flags | one round trip instead of N |
| progress → stderr, results → stdout | `ID=$(tool deploy)` keeps working |
| hide progress when stderr is not a terminal / `--quiet` | no escape-code garbage in CI logs |
| confirmations default to "no" for destructive actions; `--yes` to skip | safety |

Separate "what to ask" from "how to ask": the decision logic (`setup::resolve`) takes a `Prompter`
trait – dialoguer, inquire, or a fake in tests.

### indicatif

```rust
let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr());   // or hidden()
let bar = multi.add(ProgressBar::new(100));
bar.set_style(ProgressStyle::with_template("{prefix:<14} [{bar:30.cyan/blue}] {pos:>3}% {msg}")?
    .progress_chars("=> "));
bar.set_prefix("billing"); bar.set_position(42); bar.set_message("starting replicas");
bar.finish_with_message("✔ done");          // or abandon_with_message("✖ failed")
spinner.enable_steady_tick(Duration::from_millis(100));   // animate while waiting
multi.println("warning: ...")?;              // log lines above the bars
```

- template keys: `{spinner}`, `{bar}`, `{wide_bar}`, `{pos}`, `{len}`, `{percent}`, `{eta}`,
  `{per_sec}`, `{bytes}`, `{elapsed_precise}`, `{prefix}`, `{msg}`,
- `ProgressBar` is `Clone + Send` – update from worker threads/tasks,
- iterators: `.progress()` / `.progress_count(n)` (`ProgressIterator`), readers: `bar.wrap_read(file)`,
- draw rate is limited (default 20 Hz) – updating a million times is cheap,
- indicatif hides itself when the target isn't a terminal; messages printed via `multi.println`
  are hidden too → print warnings with `eprintln!` when `multi.is_hidden()`.

### dialoguer

```rust
Input::<String>::with_theme(&theme).with_prompt("Service name").default("my-service".into())
    .validate_with(|s: &String| -> Result<(), &str> { ... }).interact_text()?;
Select::with_theme(&theme).items(&templates).default(0).interact()?;          // index
MultiSelect::with_theme(&theme).items(&features).interact()?;                 // Vec<index>
Confirm::with_theme(&theme).with_prompt("create?").default(false).interact()?;
Password::with_theme(&theme).with_prompt("token").interact()?;                // no echo
```

`ColorfulTheme` vs `SimpleTheme`; `interact_opt()` returns `None` on Esc/q; `FuzzySelect` (feature `fuzzy-select`).

### inquire

```rust
Text::new("Service name").with_autocomplete(ExistingNames).with_validator(...).with_help_message("...").prompt()?;
Select::new("Template", templates).prompt()?;           // type to filter
MultiSelect::new("Features", features).prompt()?;
Confirm::new("create?").with_default(false).prompt()?;
Password::new("token").with_display_mode(PasswordDisplayMode::Masked).without_confirmation().prompt()?;
```

Richer than dialoguer: autocompletion (`Autocomplete` trait), filtering, help lines, `DateSelect`,
`Editor`, custom types (`CustomType::<u32>`). Errors: `InquireError::OperationCanceled` (Esc),
`OperationInterrupted` (Ctrl+C).

Prompt libraries handle raw mode themselves; don't mix them with a running ratatui app.

## Example walkthrough

- `src/setup.rs` – `Prompter` trait, `resolve()` (flags → prompts → confirmation, non-interactive
  rules); tests with a scripted fake recording asked questions.
- `src/dialoguer_prompts.rs`, `src/inquire_prompts.rs` – the same questions with both libraries
  (validation, select, multi-select, confirm, password, autocomplete).
- `src/rollout.rs` – `MultiProgress`, overall spinner bar, per-service bars from threads,
  abandon on failure, warnings above bars / on stderr when hidden; test with a hidden target.
- `src/main.rs` – TTY detection, prompt library choice, `--quiet`, results on stdout, exit codes.

## Try it

```bash
cargo test -p ex019-progress-prompts
cargo build -p ex019-progress-prompts && P=target/debug/platctl-setup
$P init                                    # dialoguer
$P init --prompts inquire                  # inquire: type "bil" in the name prompt
$P init --name billing --template worker --yes < /dev/null    # non-interactive
$P init < /dev/null; echo $?               # lists missing flags
$P rollout --fail search; echo $?          # bars, failure, exit 1
$P rollout 2>/dev/null                     # only the result line
$P rollout --fail search 2>&1 | cat        # piped: no bars, warning still visible
```

## Exercises

1. Add `--json` to `rollout` printing one JSON line per finished service (stdout) while bars run on stderr.
2. Show download progress for a file with `bar.wrap_read(reader)` and `{bytes}/{total_bytes} {binary_bytes_per_sec}`.
3. Ask for the template with `FuzzySelect` (dialoguer feature `fuzzy-select`).
