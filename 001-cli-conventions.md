# 001 – CLI ergonomics and POSIX, GNU and docopt conventions

## Goal

Know the conventions users and scripts expect from a command-line tool – by implementing them
once by hand, without a parsing library. From step 002 on, clap does this work for us.

Example: `examples/001-cli-conventions` – `platctl-logs`, prints the last lines of platform log
files, optionally only for one service.

## Key concepts

### Anatomy of a command line

```
platctl-logs  -vq  --lines=20  -s billing  app.log  -  --  -odd-name.log
└── program   │    │           │           │        │  │   └ positional after `--`
              │    │           │           │        │  └ end of options
              │    │           │           │        └ positional "-" = standard input
              │    │           │           └ positional argument (operand)
              │    │           └ short option with a value
              │    └ long option with an attached value
              └ bundled short flags (-v -q)
```

| Term | Examples | Notes |
|------|----------|-------|
| flag (switch) | `-q`, `--quiet` | boolean, no value |
| option with value | `-n 5`, `-n5`, `--lines 5`, `--lines=5` | all four forms equivalent |
| repeatable flag | `-v`, `-vv`, `-v -v` | counts (verbosity) |
| positional / operand | `app.log` | order matters, often variadic (`[FILE]...`) |
| subcommand | `git commit`, `platctl deploy` | step 002 |
| global option | `platctl -v deploy` | valid for every subcommand (step 002) |

### POSIX utility conventions (IEEE Std 1003.1, "Utility Syntax Guidelines")

- options are single letters preceded by `-`; flags can be grouped (`-abc`),
- an option's value may be attached (`-n5`) or separate (`-n 5`),
- options come before operands; the first operand ends option parsing,
- `--` ends options – everything after it is an operand (`rm -- -file`),
- `-` as an operand means standard input/output.

### GNU extensions

- long options `--name`, `--name=value`, `--name value`; unambiguous prefixes often accepted (`--lin`),
- **argument permutation**: options may follow operands (`ls dir -l`); set `POSIXLY_CORRECT` to get strict POSIX behaviour,
- every program supports `--help` and `--version`, printing to **stdout** and exiting with 0.

### docopt – usage text as a contract

Help text follows a recognisable layout (also used by clap):

```
Usage: platctl-logs [OPTIONS] [FILE]...

Options:
  -n, --lines <N>   Number of lines to print [default: 10]
```

`[x]` optional, `<x>` placeholder for a value, `...` repeatable, `a | b` alternatives.

### Streams and exit codes

| Channel | What goes there |
|---------|-----------------|
| stdout | the result – data another program may consume (`| grep`, `> file`) |
| stderr | diagnostics, progress, warnings, errors – never mixed into data |
| exit code | 0 = success, 1 = runtime failure, 2 = usage error (shell/POSIX convention) |

Usage errors: one-line message + hint (`Try 'platctl-logs --help'`), not the full help.
Partial failures (one of several files missing): report each on stderr, continue, exit 1 at the end – like `cat`, `tail`.

### Ergonomics checklist (clig.dev)

- `--help` is the documentation users actually read: examples, defaults, short lines,
- predictable names (`-v/--verbose`, `-q/--quiet`, `-o/--output`, `-f/--force`, `-n/--dry-run` – know common meanings),
- quiet on success, helpful on failure; error messages say what went wrong and what to do,
- works in pipes: reads stdin when no file is given, never needs a TTY for basic operation,
- no surprising destructive defaults; confirm or require `--force`.

### Rust specifics

- `std::env::args_os()` – arguments as `OsString`; `args()` panics on non-UTF-8 input.
  File names on Unix are arbitrary bytes → keep them as `PathBuf`.
- `fn main() -> ExitCode` – explicit exit status without `process::exit` (which skips destructors).
- `env!("CARGO_PKG_VERSION")` – version from `Cargo.toml` at compile time.
- `io::stdout().lock()` once for many writes; check write errors (closed pipe).
- Keep parsing a pure function → unit tests without spawning processes.

## Example walkthrough

- `src/args.rs` – `parse(args, mode) -> Result<Command, ArgError>`: long/short options, bundling,
  attached values, `--`, `-`, GNU permutation vs `POSIXLY_CORRECT`, typed errors; 8 unit tests.
- `src/main.rs` – exit codes, help/version on stdout, diagnostics on stderr, streaming tail with a
  bounded `VecDeque`, "report and continue" for missing files, OS error messages without `(os error N)`.

## Try it

```bash
cargo test -p ex001-cli-conventions
cargo build -p ex001-cli-conventions && alias pl=target/debug/platctl-logs
printf 'ts=1 service=billing msg=start\nts=2 service=search msg=start\nts=3 service=billing msg=ready\n' > /tmp/app.log

pl --help; echo "exit=$?"                     # stdout, 0
pl -vqn1 -s billing /tmp/app.log              # bundled flags; "reading ..." goes to stderr
pl /tmp/app.log -n 1                          # GNU: option after operand
POSIXLY_CORRECT=1 pl /tmp/app.log -n 1        # POSIX: "-n" and "1" become file names
cat /tmp/app.log | pl -n 2 -s billing         # stdin
pl -n1 /tmp/app.log - <<< 'from stdin'        # file + stdin, headers like tail
pl --follow; echo "exit=$?"                   # usage error -> stderr, 2
pl /tmp/app.log missing.log 2>/dev/null; echo "exit=$?"   # data still printed, exit 1
```

## Exercises

1. Accept unambiguous long-option prefixes (`--lin 3`, `--serv billing`) and reject ambiguous ones.
2. Add `-c, --count` printing only the number of matching lines; make `--count` conflict with `--lines`.
3. Make the program name in messages come from `argv[0]` (`std::env::args_os().next()`), so a renamed binary reports its own name.
