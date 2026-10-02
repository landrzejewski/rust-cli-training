//! `crash` plugin: written directly against the C ABI (what `declare_plugin!` generates for
//! you), and a reminder that native plugins are NOT isolated: on a line `crash` it aborts –
//! taking the whole host process down with it.

use std::ffi::{CStr, CString, c_char};

use platctl_ext_api::{ABI_VERSION, Output, PluginVTable, STATUS_KEEP};

static VTABLE: PluginVTable = PluginVTable {
    abi_version: ABI_VERSION,
    // C string literals (`c"..."`) are NUL-terminated `&'static CStr`.
    name: c"crash".as_ptr(),
    version: c"0.1.0".as_ptr(),
    description: c"passes lines through, aborts the host on a line `crash`".as_ptr(),
    process,
    free_text,
};

unsafe extern "C" fn process(line: *const c_char) -> Output {
    // SAFETY: ABI contract – the host passes a valid NUL-terminated string.
    let line = unsafe { CStr::from_ptr(line) };
    if line.to_bytes() == b"crash" {
        // Same address space, same process: nothing the host can catch.
        std::process::abort();
    }
    Output {
        status: STATUS_KEEP,
        text: line.to_owned().into_raw(),
    }
}

unsafe extern "C" fn free_text(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: `text` came from `CString::into_raw` in `process` (ABI contract).
        drop(unsafe { CString::from_raw(text) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn platctl_ext_entry() -> *const PluginVTable {
    &VTABLE
}
