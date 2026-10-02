//! Plugin host: loads WebAssembly components implementing `wit/plugin.wit` with wasmtime.
//!
//! Isolation comes from WebAssembly itself (a plugin sees only its own linear memory) plus
//! what we grant: no files, no env, no network, no stdout; a fuel budget per call and a
//! memory cap per plugin.

use std::path::Path;

use anyhow::Context;
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
    component::{Component, HasSelf, Linker, ResourceTable},
};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

// Generates host-side bindings from the WIT world:
// - `Plugin` – typed handle with `call_metadata` / `call_process` for the exports,
// - `platctl::plugin::host::Host` – trait we implement to provide the imports,
// - Rust types for WIT records/enums (`Metadata`, `Level`).
wasmtime::component::bindgen!({
    world: "plugin",
    path: "wit",
});

pub use platctl::plugin::host::Level;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Fuel ≈ executed WebAssembly instructions; refilled before every call.
    pub fuel_per_call: u64,
    pub max_memory_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel_per_call: 50_000_000,
            max_memory_bytes: 64 << 20,
        }
    }
}

/// Per-plugin state stored in its `Store` – host functions receive `&mut State`.
struct State {
    plugin: String,
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
}

/// wasmtime-wasi finds its context through this trait.
impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Implementation of the `host` interface imported by plugins.
impl platctl::plugin::host::Host for State {
    fn log(&mut self, level: Level, message: String) {
        let level = match level {
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warn => "warn",
        };
        eprintln!("[plugin {}] {level}: {message}", self.plugin);
    }
}

pub struct PluginHost {
    engine: Engine,
    linker: Linker<State>,
    limits: Limits,
}

impl PluginHost {
    pub fn new(limits: Limits) -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;

        let mut linker = Linker::new(&engine);
        // Rust's std on wasm32-wasip2 imports WASI interfaces (clocks, random, cli, filesystem)
        // – they must be linkable even if the context grants nothing.
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
        // Our own `host` interface. `HasSelf<State>`: the closure returns the implementor itself.
        Plugin::add_to_linker::<State, HasSelf<State>>(&mut linker, |state| state)?;
        Ok(Self {
            engine,
            linker,
            limits,
        })
    }

    pub fn load(&self, path: &Path) -> anyhow::Result<LoadedPlugin> {
        // Validates and compiles the component to native code (Cranelift). Big plugins:
        // precompile once (`Engine::precompile_component`) and cache the artifact.
        let component = Component::from_file(&self.engine, path)
            // wasmtime 49+ has its own error type; convert to attach anyhow context.
            .map_err(anyhow::Error::from)
            .with_context(|| format!("cannot load plugin {}", path.display()))?;

        // Capability-based: an empty builder grants nothing. Only stderr is inherited so a
        // panicking plugin's message is visible; stdout belongs to our data stream.
        let wasi = WasiCtxBuilder::new().inherit_stderr().build();
        let name = path.file_stem().map_or_else(
            || "plugin".to_string(),
            |s| s.to_string_lossy().into_owned(),
        );
        let state = State {
            plugin: name,
            wasi,
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new()
                .memory_size(self.limits.max_memory_bytes)
                .build(),
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store.set_fuel(self.limits.fuel_per_call)?;

        let bindings = Plugin::instantiate(&mut store, &component, &self.linker)
            .map_err(anyhow::Error::from)
            .with_context(|| format!("{} does not implement platctl:plugin", path.display()))?;
        let metadata = bindings.call_metadata(&mut store)?;
        store.data_mut().plugin.clone_from(&metadata.name);
        Ok(LoadedPlugin {
            store,
            bindings,
            metadata,
            fuel_per_call: self.limits.fuel_per_call,
        })
    }
}

pub struct LoadedPlugin {
    store: Store<State>,
    bindings: Plugin,
    pub metadata: Metadata,
    fuel_per_call: u64,
}

impl LoadedPlugin {
    /// A trap (out of fuel, memory, panic) returns `Err`; the instance must not be used again.
    pub fn process(&mut self, line: &str) -> anyhow::Result<Option<String>> {
        self.store.set_fuel(self.fuel_per_call)?;
        Ok(self.bindings.call_process(&mut self.store, line)?)
    }
}
