//! The binary contract between `platctl-ext` and its native plugins (step 028).
//!
//! Only C-compatible things cross the boundary: `#[repr(C)]` structs, integers, raw pointers,
//! `extern "C"` function pointers, NUL-terminated strings. Never `String`, `Vec`, `Result`,
//! `Box<dyn Trait>` or `&str`: their layout is unspecified and may differ between the host and
//! a plugin built with another compiler version.
//!
//! Ownership rule: memory is freed by the side that allocated it. Text returned by a plugin is
//! handed back to the plugin's own `free_text` (it may use a different allocator than the host).

use std::{
    ffi::{CStr, CString, c_char},
    ptr,
};

/// Bump on ANY incompatible change. The host reads it before touching anything else, so it must
/// stay the first field of `PluginVTable` in every future version.
pub const ABI_VERSION: u32 = 1;

/// Name of the exported entry point (NUL-terminated for `dlsym`/`GetProcAddress`).
pub const ENTRY_SYMBOL: &[u8] = b"platctl_ext_entry\0";

/// Signature of the entry point: returns a pointer to a `'static` vtable.
pub type EntryFn = unsafe extern "C" fn() -> *const PluginVTable;

// Status codes are plain integers, not a Rust enum: an out-of-range enum value coming from
// foreign code is undefined behavior, an unexpected integer is just an error to report.
/// `text` = the (possibly modified) line.
pub const STATUS_KEEP: u32 = 0;
/// `text` = null, the line is dropped.
pub const STATUS_DROP: u32 = 1;
/// `text` = error message.
pub const STATUS_ERROR: u32 = 2;

#[repr(C)]
pub struct Output {
    pub status: u32,
    /// Allocated by the plugin; release with `PluginVTable::free_text`.
    pub text: *mut c_char,
}

/// `#[repr(C)]`: fields in declaration order with C alignment rules – a layout both sides agree on.
#[repr(C)]
pub struct PluginVTable {
    pub abi_version: u32,
    pub name: *const c_char,
    pub version: *const c_char,
    pub description: *const c_char,
    /// `line`: NUL-terminated UTF-8, valid only during the call.
    pub process: unsafe extern "C" fn(line: *const c_char) -> Output,
    pub free_text: unsafe extern "C" fn(text: *mut c_char),
}

// SAFETY: a vtable is immutable and only points to 'static strings and functions, so sharing
// it between threads is fine. (Raw pointers are !Sync by default, which a `static` requires.)
unsafe impl Sync for PluginVTable {}

/// Plugin author's side: implement a safe function, let the macro generate the C ABI glue.
///
/// ```ignore
/// fn process(line: &str) -> Result<Option<String>, String> { Ok(Some(line.to_uppercase())) }
/// platctl_ext_api::declare_plugin!(name: "upper", description: "uppercases lines", process: process);
/// ```
#[macro_export]
macro_rules! declare_plugin {
    (name: $name:literal, description: $description:literal, process: $process:path $(,)?) => {
        static VTABLE: $crate::PluginVTable = $crate::PluginVTable {
            abi_version: $crate::ABI_VERSION,
            name: concat!($name, "\0").as_ptr().cast(),
            version: concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast(),
            description: concat!($description, "\0").as_ptr().cast(),
            process: process_c,
            free_text: $crate::support::free_text,
        };

        unsafe extern "C" fn process_c(line: *const ::std::ffi::c_char) -> $crate::Output {
            // SAFETY: the host passes a valid NUL-terminated string (ABI contract).
            unsafe { $crate::support::process($process, line) }
        }

        /// Entry point the host looks up by name – `no_mangle` keeps the symbol name as written.
        #[unsafe(no_mangle)]
        pub extern "C" fn platctl_ext_entry() -> *const $crate::PluginVTable {
            &VTABLE
        }
    };
}

/// Glue used by `declare_plugin!` – not part of the ABI itself.
#[doc(hidden)]
pub mod support {
    use std::panic::{self, AssertUnwindSafe};

    use super::*;

    pub type ProcessFn = fn(&str) -> Result<Option<String>, String>;

    /// Calls a safe Rust function from the C ABI.
    ///
    /// # Safety
    /// `line` must point to a valid NUL-terminated string.
    pub unsafe fn process(function: ProcessFn, line: *const c_char) -> Output {
        // SAFETY: guaranteed by the caller.
        let line = unsafe { CStr::from_ptr(line) }.to_string_lossy();
        // A panic must not unwind out of an `extern "C"` function – Rust aborts the whole
        // process if it tries. Catch it and report it as an error instead.
        match panic::catch_unwind(AssertUnwindSafe(|| function(&line))) {
            Ok(Ok(Some(text))) => output(STATUS_KEEP, text),
            Ok(Ok(None)) => Output {
                status: STATUS_DROP,
                text: ptr::null_mut(),
            },
            Ok(Err(message)) => output(STATUS_ERROR, message),
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(ToString::to_string))
                    .unwrap_or_else(|| "unknown panic".to_string());
                output(STATUS_ERROR, format!("panicked: {message}"))
            }
        }
    }

    /// # Safety
    /// `text` must be null or come from this plugin's `Output::text`, and be freed only once.
    pub unsafe extern "C" fn free_text(text: *mut c_char) {
        if !text.is_null() {
            // SAFETY: guaranteed by the caller – reclaims the CString leaked in `output`.
            drop(unsafe { CString::from_raw(text) });
        }
    }

    fn output(status: u32, text: String) -> Output {
        // C strings can't contain NUL bytes – strip them instead of failing.
        let text = CString::new(text.replace('\0', "")).unwrap_or_default();
        Output {
            status,
            text: text.into_raw(),
        }
    }
}
