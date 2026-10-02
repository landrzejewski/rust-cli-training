# 002 – Commands, subcommands, flags, positional arguments, global options

## Goal

Model a multi-command tool (`git`/`kubectl` style) with clap's derive API: subcommands, nested
subcommands, flags, options with defaults, positional arguments and global options.

Example: `examples/002-commands-and-arguments` – `platctl deploy|status|logs|env`.

## Key concepts

### Command structure

```
platctl [GLOBAL OPTIONS] <COMMAND> [COMMAND OPTIONS] [ARGS]
platctl -v --env prod deploy billing --tag 1.4.2 --replicas 3 --dry-run
platctl env show prod                       # nested subcommand
```

Design rules:
- **noun-verb or verb-noun – pick one and stay consistent** (`platctl env list` vs `platctl list envs`),
- commands are verbs for actions (`deploy`, `status`), groups are nouns (`env`, `service`),
- global options (`-v`, `--env`, `--output`, `--config`) mean the same in every command,
- positional arguments for the main object(s) (`deploy <SERVICE>`), options for modifiers,
- no more than 2–3 positionals; their order must be obvious,
- aliases for frequent commands (`st` = `status`), but don't invent cryptic abbreviations.

### clap derive in a nutshell

```rust
#[derive(Parser)]
#[command(name = "platctl", version, about, propagate_version = true)]
struct Cli {
    #[command(flatten)] global: GlobalArgs,       // reusable group of options
    #[command(subcommand)] command: Command,      // required subcommand
}

#[derive(Subcommand)]
enum Command {
    Deploy(DeployArgs),                               // arguments in a separate struct
    #[command(visible_alias = "st")]
    Status { services: Vec<String> },                 // inline fields
    #[command(subcommand)] Env(EnvCommand),           // nested subcommands
}
```

| Field type | Becomes |
|------------|---------|
| `bool` | flag `--dry-run` |
| `T` (positional) | required `<SERVICE>` |
| `Option<T>` | optional `[SERVICE]` / optional option |
| `Vec<T>` | zero or more `[SERVICE]...` |
| `#[arg(short, long)]` | `-r`, `--replicas` (names derived from the field) |
| `#[arg(default_value_t = 1)]` | typed default, shown in help |
| `#[arg(action = ArgAction::Count)]` | `-vvv` → 3 |
| `#[arg(global = true)]` | accepted before and after the subcommand |

Values are parsed into the field type (`u32`, `usize`, `PathBuf`...) – invalid input is a usage
error before your code runs.

Doc comments become help: `///` on the struct → about (first paragraph for `-h`, all for `--help`),
`///` on fields → argument help. Use `//` for comments meant for developers.

### What clap gives for free

- `-h` (short) / `--help` (long) help, `help <command>`, `-V/--version`,
- errors with suggestions (`unrecognized subcommand 'statsu' … tip: 'st', 'status'`),
- exit code **2** on usage errors, **0** on help/version, output to the right stream,
- `--` and POSIX/GNU syntax from step 001.

### Testing the CLI definition

```rust
#[test] fn cli_definition_is_valid() { Cli::command().debug_assert(); }   // catches clashes
let cli = Cli::try_parse_from(["platctl", "status", "-vv"])?;              // no process exit
```

`debug_assert` found a real bug while writing this example: `propagate_version` adds `--version` to
every subcommand, so `deploy --version <VER>` clashed – the option is `--tag` now.

## Example walkthrough

- `src/cli.rs` – `Cli`, `GlobalArgs` (flattened, global), `Command` enum with a struct variant
  (`DeployArgs`), inline variants, alias, nested `EnvCommand`; 5 parser tests.
- `src/main.rs` – `Cli::parse()` and an exhaustive `match` dispatching to one function per command.

## Try it

```bash
cargo test -p ex002-commands-and-arguments
cargo build -p ex002-commands-and-arguments && alias platctl=target/debug/platctl
platctl -h; platctl --help; platctl help deploy
platctl -v deploy billing --tag 1.4.2 -r 3 --dry-run -e prod
platctl status --env prod -vv billing        # global options after the subcommand
platctl st billing search                    # alias, several positionals
platctl env show prod
platctl statsu; echo "exit=$?"               # suggestion, exit 2
platctl deploy; echo "exit=$?"               # missing <SERVICE>, exit 2
```

## Exercises

1. Add `platctl rollback <SERVICE> [--to <TAG>]` and make `--to` required when `--env prod` (hint: step 009 covers `required_if_eq`).
2. Add a hidden subcommand (`#[command(hide = true)]`) for internal diagnostics.
3. Move `deploy`, `status` under a `service` group (`platctl service deploy ...`) – what changes for users and scripts?
