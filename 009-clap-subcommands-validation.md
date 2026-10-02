# 009 – Subcommands as enums, argument validation, custom value parsers

## Goal

Push validation into the CLI definition: typed values, allowed values, ranges, mutually exclusive
and dependent options, custom formats – so command code receives only valid input and users get
consistent, helpful errors.

Example: `examples/009-clap-subcommands-validation` – `platctl-release create|rollback|list`.

## Key concepts

### Subcommands as an enum, shared options with `flatten`

```rust
#[derive(Subcommand)]
enum Command {
    Create(CreateArgs),                                   // big command → own struct
    Rollback { service: String, #[arg(long, value_parser = parsers::version)] to: Version,
               #[command(flatten)] target: TargetArgs },  // shared group
    List { #[arg(long, value_parser = parsers::duration)] since: Option<Duration> },
}
```

### Validation toolbox

| Need | clap |
|------|------|
| fixed set of values | `#[derive(ValueEnum)]` + `value_enum` – kebab-case names, doc comments in help, typo suggestions, `#[value(alias = "bg")]` |
| numeric range | `value_parser!(u8).range(1..=100)` |
| custom format | `value_parser = my_fn` where `fn(&str) -> Result<T, impl Into<Box<dyn Error>>>` |
| list in one argument | `value_delimiter = ','` (`--region a,b`) |
| repeatable option | `Vec<T>` field (`-l a=1 -l b=2`) |
| exactly one of | `ArgGroup::new("source").required(true).args(["version", "commit"])` |
| A excludes B | `conflicts_with = "yes"` |
| A needs B | `requires = "b"` |
| required when other has value | `required_if_eq("strategy", "canary")` / `required_unless_present` |
| anything else | validate after parsing, report with `Cli::command().error(kind, msg)` → same format, exit 2 |

Validation that depends on the outside world (does the service exist?) belongs in the command
(step 003 errors), not in the parser.

### Custom value parsers

```rust
pub fn duration(input: &str) -> Result<Duration, String> { ... }      // "1h30m" → 5400 s
pub fn key_value(input: &str) -> Result<(String, String), String> { ... }
pub fn version(input: &str) -> Result<semver::Version, String> { ... } // accepts "v1.4.2"

#[arg(long, value_parser = parsers::duration, value_name = "DURATION", default_value = "10m")]
timeout: Duration,
```

```
error: invalid value '5x' for '--timeout <DURATION>': unknown unit `x` in `5x` (use s, m, h)
```

- defaults go through the parser too (`default_value = "10m"`),
- `value_name` documents the format in usage (`<DURATION>`, `<KEY=VALUE>`),
- ready-made: `humantime::parse_duration`, `semver`, `url::Url`, `PathBuf` with `value_parser!(PathBuf)`.

### Consistent errors

All violations use clap's format and exit code 2:

```
error: invalid value 'canray' for '--strategy <STRATEGY>'
  [possible values: rolling, blue-green, canary]
  tip: a similar value exists: 'canary'
```

For post-parse checks build the command (`command.build()`), take the subcommand
(`find_subcommand_mut("create")`) and call `.error(...)` – the usage line then matches the command.

### Testing the rules

`try_parse_from` + `error.kind()` (`MissingRequiredArgument`, `ArgumentConflict`,
`ValueValidation`, `InvalidValue`) – one test per rule documents the CLI contract.

## Example walkthrough

- `src/parsers.rs` – duration, key=value, semver parsers + tests.
- `src/cli.rs` – enum subcommands, `TargetArgs` flatten, `Strategy` (`ValueEnum` + alias), group,
  range, `required_if_eq`, `conflicts_with`, delimiter, post-parse `validate()`; rule tests.
- `src/main.rs` – dispatch; `validate()` errors via `error.exit()`.

## Try it

```bash
cargo test -p ex009-clap-subcommands-validation
cargo build -p ex009-clap-subcommands-validation && P=target/debug/platctl-release
$P create -h
$P create billing --version v1.4.2 --strategy canary --canary-percent 10 -l team=payments --region eu-west-1,us-east-1 --timeout 1h30m
$P create billing                                     # group: one of --version/--commit
$P create billing --version 1.0.0 --commit abc1234    # group: not both
$P create billing --version 1.0.0 --strategy canray   # suggestion
$P create billing --version 1.0.0 --strategy canary   # --canary-percent required
$P create billing --version 1.0.0 --canary-percent 10 # post-parse rule
$P create billing --version 1.0.0 --dry-run --yes     # conflict
$P list --since 2h
```

## Exercises

1. Accept `--since 2026-09-01` (a date) in addition to durations – return an enum from the parser.
2. Restrict `--region` to a `ValueEnum` while keeping comma-separated input.
3. Make `--yes` required when `--env prod` and stdin is not a terminal.
