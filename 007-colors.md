# 007 – Colors: owo-colors, nu-ansi-term, NO_COLOR and terminals without ANSI

## Goal

Use color to make output easier to scan – without breaking pipes, logs, accessibility or
terminals that don't support it.

Example: `examples/007-colors` – `platctl-status [--color auto|always|never]`.

## Key concepts

### How terminal colors work

ANSI escape sequences embedded in the text: `ESC[32m` green, `ESC[1m` bold, `ESC[0m` reset.
The terminal interprets them; a file or another program receives the raw bytes:

```
^[[32m✔ running ^[[0m      <- `platctl-status --color always | cat -v`
```

Color depth: 16 basic colors (safe everywhere), 256 colors, truecolor (`COLORTERM=truecolor`).
Prefer the basic palette – users' terminal themes remap it to readable shades.

### When to emit colors

| Rule | Convention |
|------|-----------|
| `--color always\|never\|auto` | explicit choice wins (git, cargo, ripgrep, ls) |
| `NO_COLOR=<non-empty>` | disable colors (no-color.org) – respect it in `auto` |
| `CLICOLOR_FORCE=<non-zero>` | force colors into pipes (e.g. `| less -R`) |
| `TERM=dumb` | terminal without escape sequence support (Emacs shell, some CI) |
| stream is not a TTY | no colors by default (files, pipes, CI logs) |

Decide **per stream**: `platctl-status > out.txt` writes a plain file while warnings on stderr
stay colored in the terminal.

```rust
let stdout_color = use_color(cli.color, &Environment::detect(io::stdout().is_terminal()));
let stderr_color = use_color(cli.color, &Environment::detect(io::stderr().is_terminal()));
```

### Painting: owo-colors vs nu-ansi-term

```rust
// owo-colors – extension trait on any Display value, no allocation
use owo_colors::{OwoColorize, Style};
println!("{}", "running".green());
let style = if enabled { Style::new().yellow().bold() } else { Style::new() };
println!("{}", "degraded".style(style));

// nu-ansi-term – Style/Color values paint strings (used by nushell)
use nu_ansi_term::{Color, Style};
let label = if enabled { Color::Yellow.bold() } else { Style::default() };
eprintln!("{} ...", label.paint("warning:"));
```

Both are small and dependency-free. Alternatives: `anstream`/`anstyle` (used by clap and cargo –
strips escape codes automatically when the stream doesn't support them), `colored`, `console`.
owo-colors also has `if_supports_color` (feature `supports-colors`) with built-in detection.

Pitfall: format width counts escape bytes – pad (`{:<10}`) **before** styling.

### Accessibility

- color reinforces, never carries meaning alone: `✔ running`, `⚠ degraded`, `✖ stopped`,
- ~8 % of men have a color vision deficiency – red/green alone is ambiguous,
- dim text may be unreadable on some themes; keep important data in normal weight,
- screen readers ignore color; symbols need a word next to them.

### Windows

Windows 10+ supports ANSI after enabling virtual terminal processing
(`nu_ansi_term::enable_ansi_support()`, or `anstream`/`crossterm`, which handle it). Windows Terminal
enables it by default; legacy `cmd.exe` consoles may not.

## Example walkthrough

- `src/color.rs` – `ColorChoice`, `Environment` (inputs), `use_color` (pure decision); decision-table test.
- `src/main.rs` – separate decisions for stdout/stderr, owo-colors table with padding before styling,
  nu-ansi-term warnings, symbols + words, Windows ANSI enablement.

## Try it

```bash
cargo test -p ex007-colors
cargo build -p ex007-colors && P=target/debug/platctl-status
$P                                   # colors in the terminal
$P | cat -v                          # piped: plain text
$P --color always | cat -v           # escape codes visible
NO_COLOR=1 $P                        # disabled
CLICOLOR_FORCE=1 $P | less -R        # forced into a pager
TERM=dumb $P                         # dumb terminal
$P > /tmp/status.txt                 # plain file, warnings on stderr still colored
```

## Exercises

1. Add `--color` to step 006's table output (`comfy-table` supports cell colors) using the same decision function.
2. Replace manual detection with `anstream::stdout()` and compare behavior.
3. Add an ASCII fallback for symbols (`[OK]`, `[WARN]`, `[FAIL]`) when the locale is not UTF-8 (`LANG`/`LC_ALL`).
