# 011 – config-rs and figment: combining configuration sources; dotenvy; serde

## Goal

Replace the hand-written merging from step 005 with libraries: load the same nested settings
from defaults, files, profiles, a `.env` file, environment variables and flags – once with
**config-rs**, once with **figment** – and compare.

Example: `examples/011-config-rs-figment` – `platctl-settings [--loader config-rs|figment]`.

## Key concepts

### The model is a serde struct

```rust
#[derive(Serialize, Deserialize, Default)]
pub struct Settings { pub output: String, pub api: Api, pub deploy: Deploy }
pub struct Api { pub url: String, pub timeout_secs: u64, pub token: Option<String> }
```

`Serialize` → defaults become the lowest layer; `Deserialize` → the merged tree becomes typed
data, with errors naming the key. Nested structs = TOML tables = `__` in env var names.

### config-rs

```rust
Config::builder()
    .add_source(Config::try_from(&Settings::default())?)                 // defaults
    .add_source(File::from(dir.join("platctl.toml")).required(false))     // files
    .add_source(File::from(dir.join(format!("platctl.{profile}.toml"))).required(false))
    .add_source(Environment::with_prefix("PLATCTL").prefix_separator("_").separator("__")
        .try_parsing(true).list_separator(",").with_list_parse_key("deploy.regions"))
    .set_override_option("api.timeout_secs", cli.timeout_secs)?          // flags
    .build()?
    .try_deserialize::<Settings>()?
```

### figment

```rust
Figment::from(Serialized::defaults(Settings::default()))
    .merge(Toml::file(dir.join("platctl.toml")))
    .merge(Toml::file(dir.join(format!("platctl.{profile}.toml"))))
    .merge(Env::prefixed("PLATCTL_").split("__"))
    .merge(Flags(overrides))                       // custom Provider
    .extract::<Settings>()?
```

- `merge` (later wins) vs `join` (keep existing) vs `admerge`/`adjoin` (also concatenate arrays),
- every value carries **metadata** → errors and provenance name the source:
  `found string "soon", expected u64 for key "API.TIMEOUT_SECS" in PLATCTL_ environment variable(s)`,
  `figment.find_metadata("api.timeout_secs")` → `command-line flags`,
- custom `Provider` trait (`metadata` + `data`) – flags, remote config services, databases,
- native profiles: `Toml::file(..).nested()` + `[default]`/`[prod]` tables + `.select(profile)`,
- `figment::Jail` (feature `test`) – isolated env vars and files in tests.

### Comparison

| | config-rs | figment |
|---|---|---|
| Model | builder → `Config` tree → deserialize | providers merged → extract |
| Formats | TOML, YAML, JSON, INI, RON, JSON5 | TOML, YAML, JSON, env, serialized values; custom providers |
| Env lists | `list_separator(",")` per key | value syntax `[a,b]` |
| Provenance in errors | key only | key + source (file, env var, provider) |
| Profiles | by convention (file per profile) | built in (`select`, `nested`) |
| Testing helpers | – | `Jail` |
| Used by | many services and CLIs | Rocket |

Both are fine; figment wins on diagnostics, config-rs on format support and familiarity.

### Gotcha: key naming

Env vars are mapped to **lowercase snake_case** keys (`PLATCTL_API__TIMEOUT_SECS` → `api.timeout_secs`).
Keep file keys in snake_case; kebab-case keys (`timeout-secs`) could not be overridden from the
environment without extra mapping.

### dotenvy

```rust
dotenvy::from_path("example.env")?;   // or dotenvy::dotenv() – searches ./.env and parents
```

- sets variables only if not already set – the real environment wins,
- `.env` is for local development; git-ignore it, commit an example (`example.env`),
- load it before reading config (and before spawning threads).

### clap `env`

`#[arg(long, env = "PLATCTL_PROFILE")]` (clap feature `env`): one argument from flag or env var;
shown in `--help` as `[env: PLATCTL_PROFILE=]`.

## Example walkthrough

- `config/platctl.toml`, `config/platctl.prod.toml`, `example.env` – sources.
- `src/settings.rs` – model, defaults, masked TOML output, `LoadOptions`.
- `src/with_config_rs.rs` – config-rs loader.
- `src/with_figment.rs` – figment loader, custom `Flags` provider, provenance.
- `src/main.rs` – flags, dotenvy, loader selection, exit 78; tests with `Jail`: both loaders agree,
  env beats files, errors name the source.

## Try it

```bash
cargo test -p ex011-config-rs-figment
cargo build -p ex011-config-rs-figment && cd examples/011-config-rs-figment && P=../../target/debug/platctl-settings
$P --explain
$P --profile prod --env-file example.env --timeout-secs 7 --explain
PLATCTL_DEPLOY__REGIONS='[eu-central-1]' $P --profile prod --explain        # figment list syntax
PLATCTL_DEPLOY__REGIONS='eu-central-1' $P --loader config-rs --profile prod  # config-rs list syntax
PLATCTL_API__TIMEOUT_SECS=soon $P; PLATCTL_API__TIMEOUT_SECS=soon $P --loader config-rs   # compare errors
```

## Exercises

1. Switch the figment loader to native profiles: one `platctl.toml` with `[default]` and `[prod]` tables.
2. Add YAML support (`config` feature `yaml` / `figment` feature `yaml`) and load `platctl.yaml` if present.
3. Validate semantic rules after loading (`timeout_secs` between 1 and 300, `url` parses as `url::Url`) and report them with the key and source.
