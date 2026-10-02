# 028 – Native plugins: dynamic libraries, libloading, C ABI

## Goal

Load plugins as shared libraries (`.so` / `.dylib` / `.dll`) at runtime. Understand why Rust
needs a **C ABI** at the boundary, what `unsafe` really promises there, and how this compares to
WebAssembly plugins (step 027).

Example: `examples/028-plugins-dylib`:

- `platctl-ext` – the host;
- `plugin-api/` – the ABI crate shared by host and plugins;
- `plugins/tag` – a safe Rust function plus `declare_plugin!`;
- `plugins/crash` – hand-written ABI that aborts the host.

## Key concepts

### Rust has no stable ABI

- The layout of `String`, `Vec`, trait objects, enums and `repr(Rust)` structs, and the calling
  convention of `extern "Rust"` functions, may change between compiler versions and even
  between builds.
- `crate-type = "dylib"` (a Rust dylib) only works when host and plugin come from the **exact
  same** compiler and dependency versions. That is impractical for third-party plugins.
- **`cdylib`** produces a C-compatible library. The contract is expressed in C terms:
  - `#[repr(C)]` structs; integers; raw pointers;
  - `extern "C"` functions; NUL-terminated strings;
  - one exported symbol, `#[unsafe(no_mangle)]` (Rust 2024 marks it as an unsafe attribute).

```rust
#[repr(C)]
pub struct PluginVTable {
    pub abi_version: u32,                                          // ALWAYS first, checked first
    pub name: *const c_char,
    pub process: unsafe extern "C" fn(line: *const c_char) -> Output,
    pub free_text: unsafe extern "C" fn(text: *mut c_char),       // plugin frees what it allocated
}
#[unsafe(no_mangle)]
pub extern "C" fn platctl_ext_entry() -> *const PluginVTable { &VTABLE }
```

### Rules for the boundary

| Rule | Why |
|------|-----|
| Put the ABI types in one shared crate (`platctl-ext-api`) | host and plugins compile against the same definitions |
| Make `abi_version` the first field and check it before anything else | an old or new plugin gets refused instead of misread |
| Use integers for statuses, not Rust enums | an invalid enum discriminant from foreign code is UB; an integer can be validated |
| Memory is freed by whoever allocated it | host and plugin may use different allocators |
| Catch panics with `catch_unwind` inside `extern "C"` fns | unwinding out of `extern "C"` aborts the process (Rust ≥ 1.81) |
| Strings are NUL-terminated (`CString` / `CStr`), and inner NULs are handled | C strings can't contain `\0` |
| Keep the `Library` alive while any pointer into it exists | unloading makes vtables and functions dangle |
| Load absolute paths only | relative names trigger the OS search path, which enables DLL hijacking |

### libloading

```rust
// SAFETY: runs the library's initializers with our privileges – trusted plugins only.
let library = unsafe { Library::new(path) }?;
// SAFETY: the symbol has type EntryFn per the ABI contract – the loader checks only the NAME.
let entry: Symbol<EntryFn> = unsafe { library.get(b"platctl_ext_entry\0") }?;
let vtable = unsafe { entry() };
```

- `Symbol<'lib, T>` borrows the `Library`, so the compiler prevents using a symbol after unload.
  Raw pointers obtained through it (the vtable) are your responsibility: store the `Library` next
  to them.
- `library_filename("tag")` maps to `libtag.dylib` / `libtag.so` / `tag.dll`.
- Unloading is fragile: thread-local destructors, threads started by the plugin, and macOS
  `dlclose` being a no-op for Rust libraries. Most hosts load once and never unload.
- Workspace lints: this crate needs `unsafe`, so it does **not** inherit `unsafe_code = "forbid"`.
  It enables `clippy::undocumented_unsafe_blocks` instead: every `unsafe { }` needs a
  `// SAFETY:` comment.

### Making it less painful

- **A macro on the plugin side** (`declare_plugin!`): plugin authors write
  `fn(&str) -> Result<Option<String>, String>`; the macro generates the vtable, the C glue and
  `catch_unwind`.
- **`abi_stable`**: Rust-to-Rust FFI with ABI-stable equivalents (`RString`, `RVec`, `RResult`,
  `#[sabi_trait]` trait objects), plus load-time layout checking of the whole interface.
- **`stabby`**: a similar idea, with stable layouts for niche-optimized types.
- C header generation (`cbindgen`) when plugins may be written in C/C++.

### Native vs WebAssembly plugins

| | Native (cdylib) | WebAssembly component (027) |
|---|---|---|
| Speed | native, zero-copy possible | near-native, copies at the boundary |
| Access | everything the host can do | only granted capabilities |
| A bug in the plugin | can crash or corrupt the host (`crash`: SIGABRT, exit 134, buffered output lost) | trap, host continues |
| Builds | one per OS/arch | one `.wasm` everywhere |
| Interface | C ABI by hand / abi_stable | WIT, generated bindings, rich types |
| Languages | C ABI capable (Rust, C, C++, Zig, Go with cgo) | anything compiling to components |
| Good for | trusted, performance-critical extensions (codecs, DB drivers, IDE engines) | third-party, untrusted, portable plugins |

## Example walkthrough

- `plugin-api/src/lib.rs`:
  - `PluginVTable`, `Output`, status codes, `ABI_VERSION`, `ENTRY_SYMBOL`;
  - `declare_plugin!` and the `support` module (`catch_unwind`, `CString` ownership).
- `plugins/tag` – reads `PLATCTL_REGION` (full privileges); a `boom` line panics, which is caught
  and reported as an error.
- `plugins/crash` – the same ABI written by hand (`c"..."` literals). A `crash` line calls
  `abort()`.
- `src/native.rs` – `NativePlugin::load`:
  1. canonicalize the path;
  2. `Library::new`;
  3. look up the symbol;
  4. check the ABI version;
  5. copy the metadata.

  Then `process` handles the `CString` round trip, calls `free_text`, and validates the status.
- `src/main.rs` – `list`, `run -p a -p b [FILE]`; unit tests (build plugins, round trip,
  panic → error, non-plugin rejected).
- `tests/crash.rs` – integration test: the real binary dies with SIGABRT.

## Try it

```bash
cargo build -p ex028-plugins-dylib -p platctl-ext-tag -p platctl-ext-crash
target/debug/platctl-ext list
printf 'deploy ok\n\nscale api=3\n' | PLATCTL_REGION=us-east-2 target/debug/platctl-ext run -p tag
printf 'one\nboom\n'  | target/debug/platctl-ext run -p tag            # panic -> error, exit 1
printf 'one\ncrash\n' | target/debug/platctl-ext run -p crash; echo $? # 134, even "one" is lost
nm -gU target/debug/libtag.dylib | grep platctl                       # Linux: nm -D --defined-only libtag.so
```

## Exercises

1. Bump the ABI to v2 by adding `init(config: *const c_char) -> u32`, and make the host refuse v1 plugins with a clear message (or support both).
2. Generate a C header for the ABI with `cbindgen` and write a plugin in C.
3. Port the interface to `abi_stable` (`#[sabi_trait]`, `RString`) and compare the amount of `unsafe` code.
