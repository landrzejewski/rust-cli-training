# 027 – Plugins with WebAssembly: wasmtime, WIT, components, sandboxing

## Goal

Let other people extend your CLI without recompiling it and without trusting their code:
plugins as WebAssembly components with a typed contract, loaded at runtime, sandboxed and
resource-limited.

Example: `examples/027-plugins-wasm`:

- `platctl-plug` – the host (wasmtime);
- `wit/plugin.wit` – the contract;
- `plugins/` – a separate workspace with two guest plugins: `redact` (useful) and `rogue`
  (misbehaves on purpose).

## Key concepts

### Plugin architectures for CLIs

| Approach | Example | Isolation | Languages | Distribution |
|----------|---------|-----------|-----------|--------------|
| external executables on PATH | `git foo` → `git-foo`, `cargo foo`, `kubectl` plugins | process | any | per OS/arch |
| scripting engine | Lua (`mlua`), Rhai | interpreter | one | source |
| **WebAssembly components** | Zellij, Spin, Extism users | sandbox + limits | any → wasm | **one `.wasm` for all platforms** |
| native dynamic libraries | step 028 | none (same process) | Rust/C ABI | per OS/arch + compiler version |

The simplest option, PATH executables, is often enough. Choose Wasm when plugins are untrusted,
must be cross-platform, or are called many times per run (no process spawn per call).

### Component model and WIT

```wit
package platctl:plugin@0.1.0;

interface host {                       // what the HOST gives plugins
    enum level { debug, info, warn }
    log: func(level: level, message: string);
}

world plugin {                         // the full contract of one plugin
    import host;
    record metadata { name: string, version: string, description: string }
    export metadata: func() -> metadata;
    export process: func(line: string) -> option<string>;
}
```

- **Core Wasm** only knows numbers and linear memory, so strings mean manual pointer/length ABI.
  The **component model** adds rich types (string, list, record, variant, option, result,
  resources) and a canonical ABI generated for both sides.
- **WIT** (WebAssembly Interface Types) is the IDL. The package is versioned (`@0.1.0`).
  Compatible evolution means adding new functions or interfaces; changing a signature means a
  new version.
- **Bindings:**
  - host side: `wasmtime::component::bindgen!` generates the `Plugin` struct with
    `call_metadata` / `call_process`, a `Host` trait for the imports, and Rust types;
  - guest side: `wit_bindgen::generate!` generates a `Guest` trait, the imported functions, and
    `export!`.

### Guest: building a component

```toml
[lib]
crate-type = ["cdylib"]          # + --target wasm32-wasip2 → rustc emits a component directly
```

```bash
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2      # redact.wasm ≈ 55 kB (opt-level "s", LTO, strip)
```

- Plugins live in their **own workspace**: a different target, and they could live in another
  repository or be written in another language. Other toolchains: `componentize-py`
  (Python), `jco` (JS), TinyGo, C via wit-bindgen.
- `wasm-tools component wit redact.wasm` prints the interface embedded in a binary.

### Host: wasmtime

```rust
let mut config = Config::new();
config.consume_fuel(true);                                   // count executed instructions
let engine = Engine::new(&config)?;                          // one per process, shared
let mut linker = Linker::new(&engine);
wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;         // WASI imports used by Rust std
Plugin::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)?;   // our `host` interface

let component = Component::from_file(&engine, "redact.wasm")?;         // validate + compile
let mut store = Store::new(&engine, State { wasi, table, limits, .. }); // one per plugin instance
store.limiter(|s| &mut s.limits);                            // memory cap
store.set_fuel(50_000_000)?;                                 // CPU budget (refilled per call)
let plugin = Plugin::instantiate(&mut store, &component, &linker)?;
let out = plugin.call_process(&mut store, "line")?;
```

- **Store** = an instance's world: its memories, its host state `T`, its fuel. Plugins never
  share a store.
- **Compilation cost:** `Component::from_file` runs Cranelift (milliseconds to seconds for big
  components). To cache it, use `Engine::precompile_component` + `Component::deserialize`
  (`unsafe`: trust the cache file) or the `cache` feature.
- **wasmtime 49+ uses its own `wasmtime::Error`.** With the `anyhow` feature, `?` converts it
  into `anyhow::Error`.

### Sandboxing: capabilities, not permissions

- WASI grants nothing by default. `WasiCtxBuilder::new()` gives no preopened directories,
  no environment variables or arguments, no network and null stdio. You add capabilities
  explicitly: `.preopened_dir(path, "/data", DirPerms::READ, FilePerms::READ)`, `.env(k, v)`,
  `.inherit_network()`.
- **CPU:** fuel (deterministic) or epoch interruption (wall-clock deadline via a timer thread,
  cheaper). Either way a trap ends the call.
- **Memory:** `StoreLimitsBuilder::memory_size`. The guest's allocation fails, Rust aborts, and
  the trap is reported.
- **After a trap the instance is poisoned.** Drop it. Here the CLI fails loudly instead of passing
  lines through unprocessed: a redaction plugin that silently stops redacting is a data leak.
- **Not covered by the sandbox:** what you hand the plugin through your own imports. Every host
  function is attack surface, so validate its inputs.

Observed in the demo (`rogue` plugin):

| Line | Plugin tries | Result |
|------|--------------|--------|
| `file` | `fs::read_to_string("/etc/passwd")` | `No such file or directory` – there is no filesystem |
| `env` | `env::var("HOME")` | `None` |
| `loop` | infinite loop | `wasm trap: all fuel consumed` |
| `alloc` | 1 GiB `Vec` with a 16 MiB cap | `memory allocation failed` → `unreachable` trap |

## Example walkthrough

- `wit/plugin.wit` – the contract; `platctl-plug wit` prints it (embedded with `include_str!`).
- `src/host.rs`:
  - `bindgen!`, `State` (WASI ctx, resource table, limits);
  - `WasiView`, the `Host` impl (`log`);
  - `PluginHost::new/load`, `LoadedPlugin::process` (fuel refill).
- `src/main.rs`:
  - `list`, `run -p a -p b [FILE]` (plugin chain over lines), `wit`;
  - tests build the guests and check masking, the sandbox, fuel and memory.
- `plugins/redact`, `plugins/rogue` – guests with `wit_bindgen::generate!`.
- `build-plugins.sh` – builds the components into `target/plugins/`.

## Try it

```bash
rustup target add wasm32-wasip2
examples/027-plugins-wasm/build-plugins.sh
cargo run -p ex027-plugins-wasm -- list
cargo run -p ex027-plugins-wasm -- run -p redact examples/027-plugins-wasm/sample.log
printf 'file\nenv\nloop\n' | cargo run -p ex027-plugins-wasm -- run -p rogue
echo alloc | cargo run -p ex027-plugins-wasm -- run -p rogue --max-memory-mib 16
cargo run -p ex027-plugins-wasm -- wit
```

## Exercises

1. Add a `config: func(settings: list<tuple<string, string>>)` export and pass plugin settings from a TOML file.
2. Grant the plugin read-only access to one directory (`preopened_dir`) and write a plugin that enriches lines from a lookup file.
3. Replace fuel with epoch interruption (`Config::epoch_interruption`, `Store::set_epoch_deadline`, a ticker thread) and compare the overhead on a large log.
