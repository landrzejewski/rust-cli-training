# 005 – Config files, environment variables and their precedence (12-factor)

## Goal

Combine defaults, config files, environment variables and flags into one effective configuration
with a predictable precedence – and be able to explain where each value came from.

Example: `examples/005-config-precedence` – `platctl-config show|paths`, merging by hand
(libraries follow in step 011).

## Key concepts

### Precedence (lowest → highest)

```
defaults  <  /etc/platctl/config.toml  <  ~/.config/platctl/config.toml  <  ./.platctl.toml
          <  PLATCTL_* environment variables  <  command-line flags
```

- more specific / closer to the invocation wins: a flag typed now beats an env var set an hour ago,
  which beats a file written last month,
- `--config FILE` replaces the file discovery (explicit beats implicit) – it must exist,
- implicit files are optional; missing ones are skipped silently.

### 12-factor (factor III: config)

- configuration that varies between deploys lives in the **environment**, not in code,
- strict separation of config and code; same binary everywhere,
- for CLIs: env vars are ideal for CI/containers, files for humans, flags for one-off overrides.

### Naming

| Source | Convention | Example |
|--------|------------|---------|
| file key | kebab-case (TOML/YAML) | `timeout-secs = 20` |
| env var | `PREFIX_SCREAMING_SNAKE` | `PLATCTL_TIMEOUT_SECS=20` |
| flag | `--kebab-case` | `--timeout-secs 20` |
| nested | `__` or `_` separator | `PLATCTL_API__URL` (libraries) |

Keep the same name in all three forms – users guess the others from one.

### File locations

| Scope | Linux / macOS (XDG) | Windows |
|-------|---------------------|---------|
| system | `/etc/platctl/config.toml` | `%PROGRAMDATA%\platctl\` |
| user | `$XDG_CONFIG_HOME/platctl/config.toml` (default `~/.config`) | `%APPDATA%\platctl\` |
| project | `./.platctl.toml` (optionally searched upwards to the repo root) | same |

Crates: `dirs` / `directories` / `etcetera` give platform paths (`etcetera` lets you choose XDG on macOS).

### Merging by hand

```rust
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Layer { env: Option<String>, timeout_secs: Option<u64>, ... }   // every field optional

let mut config = Config::default();                                    // 1. defaults
for file in files { config.apply(parse_file(&read(file)?)?, ...); }    // 2. files
config.apply(layer_from_env(|k| std::env::var(k).ok())?, ...);         // 3. env
config.apply(Layer { env: cli.env, ... }, ...);                        // 4. flags
```

- flags must be `Option<T>` (no clap default) – otherwise the default always overrides lower layers;
  defaults live in ONE place (`Config::default`),
- `deny_unknown_fields` catches typos in files,
- errors name the source: file + line/column, variable name, flag,
- injecting the env lookup (`impl Fn(&str) -> Option<String>`) keeps tests free of global state
  (`std::env::set_var` is `unsafe` in edition 2024).

### Provenance – "why is it using staging?"

```
env           = prod                             # env PLATCTL_ENV
timeout-secs  = 3                                # flag --timeout-secs
output        = json                             # file platctl.example.toml
```

`git config --show-origin`, `kubectl config view`, `cargo config get --show-origin` – mature tools
can explain their configuration. Cheap to implement, saves hours of support.

clap shortcut: `#[arg(long, env = "PLATCTL_ENV")]` (feature `env`) merges env + flag for a single
argument; `ArgMatches::value_source()` tells which one was used. Files still need a separate layer.

## Example walkthrough

- `src/config.rs` – `Layer` (optional fields), `Config` of `Sourced<T>`, `apply`, env parsing with
  named errors, file parsing with `deny_unknown_fields`; precedence tests.
- `src/main.rs` – search path (system/user/project or `--config`), merge order, `show`/`paths`,
  exit code 78 (EX_CONFIG) for configuration errors.
- `platctl.example.toml` – sample file.

## Try it

```bash
cargo test -p ex005-config-precedence
cd examples/005-config-precedence && cargo build && P=../../target/debug/platctl-config
$P paths
$P                                                       # all defaults
$P --config platctl.example.toml                         # file
PLATCTL_ENV=prod $P --config platctl.example.toml --timeout-secs 3   # file < env < flag
printf 'env = "qa"\n' > .platctl.toml && $P; rm .platctl.toml       # project file
PLATCTL_TIMEOUT_SECS=soon $P; echo $?                    # 78, names the variable
printf 'timout-secs = 3\n' > /tmp/bad.toml && $P --config /tmp/bad.toml   # typo detected
```

## Exercises

1. Search `.platctl.toml` upwards from the current directory to the git repository root.
2. Add `--no-config` that disables all files (useful in CI for reproducibility).
3. Add profiles: `[profiles.prod]` sections selected with `--profile prod` / `PLATCTL_PROFILE`, between files and env in precedence.
