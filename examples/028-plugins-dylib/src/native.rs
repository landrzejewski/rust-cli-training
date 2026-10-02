//! Loading native plugins with `libloading` (dlopen/dlsym on Unix, LoadLibrary/GetProcAddress on
//! Windows). Every `unsafe` block below is a promise the compiler cannot check.

use std::{
    ffi::{CStr, CString, c_char},
    path::Path,
};

use anyhow::{Context, bail};
use libloading::{Library, Symbol};
use platctl_ext_api::{
    ABI_VERSION, ENTRY_SYMBOL, EntryFn, PluginVTable, STATUS_DROP, STATUS_ERROR, STATUS_KEEP,
};

pub struct NativePlugin {
    pub name: String,
    pub version: String,
    pub description: String,
    /// Points into the library's memory: valid only while `_library` is loaded.
    vtable: *const PluginVTable,
    /// Owned so the library stays loaded as long as this value lives. Dropping it unloads the
    /// code; any pointer into it (vtable, strings, function pointers) would then dangle.
    _library: Library,
}

impl NativePlugin {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        // Absolute path: a bare or relative name makes the OS loader search its own paths
        // (LD_LIBRARY_PATH, system dirs, on Windows even the current directory) – a classic
        // way to get a different library injected.
        let path = &path
            .canonicalize()
            .with_context(|| format!("plugin not found: {}", path.display()))?;
        // SAFETY: loading runs the library's initialization code with our privileges. We only
        // load files from the plugin directory, i.e. code the user chose to trust.
        let library = unsafe { Library::new(path) }
            .with_context(|| format!("cannot load {}", path.display()))?;

        // SAFETY: the ABI contract says this symbol has type `EntryFn`. A wrong type here is
        // undefined behavior – the loader only checks the NAME, never the signature.
        let entry: Symbol<EntryFn> = unsafe { library.get(ENTRY_SYMBOL) }.with_context(|| {
            format!(
                "{} is not a platctl-ext plugin (no `platctl_ext_entry` symbol)",
                path.display()
            )
        })?;
        // SAFETY: calling the plugin's entry point; it returns a pointer to its static vtable.
        let vtable = unsafe { entry() };
        if vtable.is_null() {
            bail!("{}: entry point returned null", path.display());
        }
        // SAFETY: non-null and pointing into the loaded library. Only the first field is read
        // before the version check – it has the same position in every ABI version.
        let abi_version = unsafe { (*vtable).abi_version };
        if abi_version != ABI_VERSION {
            bail!(
                "{} uses plugin ABI v{abi_version}, this platctl-ext supports v{ABI_VERSION}",
                path.display()
            );
        }
        // SAFETY: the ABI version matches, so the layout is the one we were compiled against.
        let table = unsafe { &*vtable };
        // SAFETY: the vtable points to static NUL-terminated strings.
        let (name, version, description) = unsafe {
            (
                owned(table.name),
                owned(table.version),
                owned(table.description),
            )
        };
        Ok(Self {
            name,
            version,
            description,
            vtable,
            _library: library,
        })
    }

    pub fn process(&self, line: &str) -> anyhow::Result<Option<String>> {
        let line = CString::new(line).context("line contains a NUL byte")?;
        // SAFETY: checked in `load`; the library is alive because `self` owns it.
        let table = unsafe { &*self.vtable };
        // SAFETY: `line` is a valid C string that outlives the call.
        let output = unsafe { (table.process)(line.as_ptr()) };
        let text = if output.text.is_null() {
            None
        } else {
            // SAFETY: non-null text is a NUL-terminated string allocated by the plugin.
            let text = unsafe { owned(output.text) };
            // SAFETY: handed back to the allocator that created it, exactly once.
            unsafe { (table.free_text)(output.text) };
            Some(text)
        };
        match (output.status, text) {
            (STATUS_KEEP, Some(line)) => Ok(Some(line)),
            (STATUS_DROP, _) => Ok(None),
            (STATUS_ERROR, message) => {
                bail!(
                    "plugin {} failed: {}",
                    self.name,
                    message.unwrap_or_default()
                )
            }
            (status, _) => bail!("plugin {} returned invalid status {status}", self.name),
        }
    }
}

/// Copies a C string into an owned `String` (invalid UTF-8 replaced).
///
/// # Safety
/// `text` must be null or a valid NUL-terminated string.
unsafe fn owned(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: guaranteed by the caller.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}
