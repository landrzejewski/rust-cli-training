# 010 – Shell completions (clap_complete) and man pages (clap_mangen)

## Goal

Generate completions for bash, zsh, fish and PowerShell, man pages and Markdown docs from the
clap definition – one source of truth for help, completion and documentation.

Example: `examples/010-completions-man-pages` – the tool `plat` (library CLI + two binaries:
`plat` and `gen-docs`).

## Key concepts

### Two kinds of completion

| | Static (AOT) | Dynamic |
|---|---|---|
| How | script generated once: `clap_complete::generate(shell, &mut cmd, "plat", &mut out)` | shell calls the binary on every TAB: `CompleteEnv` |
| Completes | subcommands, flags, `ValueEnum` values, `ValueHint` (files, dirs, hosts) | everything static + values computed at runtime (service names from API/cache) |
| Install | file in the shell's completion dir (packages do this) | one line in shell rc: `source <(COMPLETE=bash plat)` |
| Upgrade | regenerate on new version | always matches the binary |
| Status | stable | `unstable-dynamic` feature (API may change) |

```rust
fn main() {
    CompleteEnv::with_factory(Cli::command).complete();   // first line of main: handles COMPLETE=...
    let cli = Cli::parse();
    ...
    Command::Completions { shell } =>
        clap_complete::generate(shell, &mut Cli::command(), "plat", &mut io::stdout()),
}
```

Registering:

```bash
# static
plat completions bash > ~/.local/share/bash-completion/completions/plat
plat completions zsh  > ~/.zfunc/_plat            # fpath+=~/.zfunc; autoload -U compinit; compinit
plat completions fish > ~/.config/fish/completions/plat.fish
plat completions powershell >> $PROFILE

# dynamic
echo 'source <(COMPLETE=bash plat)' >> ~/.bashrc
echo 'source <(COMPLETE=zsh plat)'  >> ~/.zshrc
echo 'COMPLETE=fish plat | source'  >> ~/.config/fish/config.fish
```

### Making values completable

```rust
#[arg(value_enum)] env: Env,                                       // enum values
#[arg(value_hint = ValueHint::FilePath)] manifest: Option<PathBuf>, // files (also DirPath, Hostname, Url, CommandName...)
#[arg(add = ArgValueCompleter::new(complete_service))] service: String,  // dynamic

fn complete_service(current: &OsStr) -> Vec<CompletionCandidate> {
    SERVICES.iter().filter(|(n, _)| n.starts_with(prefix))
        .map(|(n, help)| CompletionCandidate::new(*n).help(Some((*help).into()))).collect()
}
```

Completers run on every keypress-TAB: keep them fast (local cache, short timeouts), never prompt,
never print to stdout except candidates.

### Man pages – clap_mangen

```rust
clap_mangen::generate_to(Cli::command(), "out/man")?;   // plat.1 + plat-deploy.1 + ...
// or: clap_mangen::Man::new(cmd).render(&mut file)?
```

Sections: NAME, SYNOPSIS, DESCRIPTION (long about), OPTIONS (with possible values, defaults),
SUBCOMMANDS, VERSION, AUTHORS. Packages install them into `share/man/man1/`.
View locally: `man ./out/man/plat-deploy.1` (macOS) / `man -l ...` (Linux).

### Markdown – clap-markdown

`clap_markdown::help_markdown::<Cli>()` – a reference page for a docs site/README.

### Where to generate

- release step / `xtask` / helper binary (`gen-docs`) – recommended,
- `build.rs` only into `OUT_DIR` (cargo forbids writing elsewhere by convention, runs on every build),
- at runtime (`plat completions zsh`) – always available, users can regenerate after upgrades.

## Example walkthrough

- `src/lib.rs` – `Cli` with `ValueEnum`, `ValueHint`, `ArgValueCompleter`; `complete_service`;
  tests: definition, completer, static scripts for 4 shells.
- `src/bin/plat.rs` – `CompleteEnv` hook + `completions <SHELL>` subcommand.
- `src/bin/gen-docs.rs` – man pages, completion files for all shells, `CLI.md`.

## Try it

```bash
cargo test -p ex010-completions-man-pages
cargo build -p ex010-completions-man-pages && export PATH="$PWD/target/debug:$PATH"
plat completions zsh | head
source <(COMPLETE=zsh plat)          # in zsh; or bash: source <(COMPLETE=bash plat)
plat de<TAB>  plat deploy <TAB>  plat deploy billing --env <TAB>  plat deploy billing -m <TAB>
gen-docs /tmp/plat-docs && ls /tmp/plat-docs/*
man /tmp/plat-docs/man/plat-deploy.1
```

## Exercises

1. Complete `--manifest` only with `*.manifest` files using a custom `ArgValueCompleter` over `std::fs::read_dir`.
2. Cache service names in `~/.cache/plat/services.json` and refresh it in the background when older than 1 h.
3. Add an `EXAMPLES` section to the man page (`Man::new(cmd)` + `render_extra_section`, or `after_long_help`).
