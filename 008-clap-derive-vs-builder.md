# 008 – Structure of a clap project: derive vs builder

## Goal

Know both clap APIs, when each fits, and how to structure a CLI crate so that parsing, behaviour
and entry points stay separate.

Example: `examples/008-clap-derive-vs-builder` – one library, two binaries: `platctl-derive` and
`platctl-builder` define the same CLI and produce the same `Action`.

## Key concepts

### Same CLI, two APIs

```rust
// derive
#[derive(Parser)]
#[command(name = "platctl", version)]
struct Cli {
    #[arg(short, long, global = true, default_value = "dev")] env: String,
    #[command(subcommand)] command: Command,
}
#[derive(Subcommand)]
enum Command { Scale { service: String, #[arg(short, long)] replicas: u32 }, ... }

// builder
Command::new("platctl")
    .arg(Arg::new("env").short('e').long("env").value_name("ENV").global(true).default_value("dev"))
    .subcommand(Command::new("scale")
        .arg(Arg::new("service").required(true))
        .arg(Arg::new("replicas").short('r').long("replicas").required(true)
             .value_parser(value_parser!(u32))))
// ...
let replicas = sub.get_one::<u32>("replicas");     // lookup by id + type
```

| | derive | builder |
|---|---|---|
| definition | types + attributes | runtime value (`clap::Command`) |
| access to values | typed fields | `get_one::<T>("id")`, `get_flag`, `get_many` |
| typos in ids / types | compile error | runtime panic (caught by `debug_assert` tests) |
| dynamic structure (aliases, plugins, config-defined commands) | no (extend `Cli::command()`) | yes |
| verbosity | low | high |
| help text | doc comments | `.help()`, `.about()` |
| value names | uppercased automatically | set `value_name` yourself |

Recommendation: **derive by default**; builder (or derive + `Cli::command()` modified at runtime)
when the command tree depends on data: user aliases (`git`), plugins (`cargo` subcommands),
generated APIs. Both produce the same `Command`, so features like completions (step 010) work for both.

Runtime-built names/help (`String` instead of `&'static str`) need clap feature **`string`**.

### Project layout

```
src/
├── bin/platctl-*.rs   entry points: parse → execute → print/exit code (a few lines)
├── lib.rs             Action – parser-independent intent
├── cli/               clap types only; conversion into Action
│   ├── derive.rs
│   └── builder.rs
└── commands/          behaviour, one module per command; no clap, returns Result
    ├── scale.rs
    └── restart.rs
```

- **parse ≠ execute**: commands take plain values → unit tests without clap, reuse from a TUI/API,
- `lib.rs` + thin binaries: integration tests and other binaries reuse the library,
- validation of *syntax/types* in the parser (`u32`), *business rules* in commands (no zero
  replicas in prod),
- one dispatch point (`commands::execute`).

For a single small binary: `main.rs` + `cli.rs` + `commands/` is enough.

### Derive gotchas

- `#[command(about)]` without a value means "the `description` from Cargo.toml" and overrides the
  doc comment – omit it (or give a value) when the doc comment should be used,
- `#[command(name = "...")]` sets the name in help/version; the usage line shows the actual binary name,
- `Cli::command().debug_assert()` in a test catches definition errors early.

## Example walkthrough

- `src/lib.rs` – layout description, `Action`.
- `src/cli/derive.rs` – derive definition + `into_action`.
- `src/cli/builder.rs` – builder definition, runtime alias subcommands, `action()` from `ArgMatches`.
- `src/cli/mod.rs` – tests: both definitions valid, same `Action` for the same input, runtime subcommands, type validation.
- `src/commands/` – behaviour with a business rule and its test.
- `src/bin/` – two thin binaries.

## Try it

```bash
cargo test -p ex008-clap-derive-vs-builder
cargo build -p ex008-clap-derive-vs-builder
diff <(target/debug/platctl-derive scale -h) <(target/debug/platctl-builder scale -h) && echo same
target/debug/platctl-builder --help          # extra alias commands from runtime data
target/debug/platctl-derive scale billing -r 3 -e prod
target/debug/platctl-builder scale billing -r 0 -e prod; echo $?   # business rule, exit 1
target/debug/platctl-builder bounce
```

## Exercises

1. Implement alias expansion: re-parse `["platctl", ...expands_to.split_whitespace()]` and execute the resulting action.
2. Add the aliases to the derive version via `Cli::command()` + `Cli::from_arg_matches`.
3. Add a third front-end: read actions as JSON lines from stdin (`{"scale": {...}}`) using the same `commands::execute`.
