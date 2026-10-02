//! Exit codes – part of the CLI's API for scripts and CI.
//!
//! Conventions:
//! - `0` success, `1` generic failure, `2` usage error (clap does this itself),
//! - `126`/`127` (shell: not executable / not found) and `128 + N` (killed by signal N,
//!   e.g. 130 = Ctrl+C) are reserved – don't use them for application errors,
//! - BSD `sysexits.h` defines a small vocabulary in the range 64–78, used by mail servers,
//!   package managers and many CLIs. Document whichever codes you return.

use std::io::ErrorKind;

use crate::{manifest::ManifestError, platform::PlatformError};

/// Generic failure.
pub const FAILURE: u8 = 1;
/// EX_DATAERR – input data was incorrect (bad manifest content).
pub const DATA_ERR: u8 = 65;
/// EX_NOINPUT – an input file did not exist or was not readable.
pub const NO_INPUT: u8 = 66;
/// EX_UNAVAILABLE – a service is unavailable / unknown.
pub const UNAVAILABLE: u8 = 69;
/// EX_TEMPFAIL – temporary failure; the caller is invited to retry.
pub const TEMP_FAIL: u8 = 75;
/// EX_NOPERM – insufficient permission.
pub const NO_PERM: u8 = 77;

/// Maps the FIRST typed error found in the chain to an exit code.
pub fn for_error(error: &anyhow::Error) -> u8 {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<ManifestError>() {
            return match error {
                ManifestError::Read { source, .. } if source.kind() == ErrorKind::NotFound => {
                    NO_INPUT
                }
                ManifestError::Read { .. } => NO_INPUT,
                _ => DATA_ERR,
            };
        }
        if let Some(error) = cause.downcast_ref::<PlatformError>() {
            return match error {
                PlatformError::UnknownService(_) => UNAVAILABLE,
                PlatformError::PermissionDenied(_) => NO_PERM,
                PlatformError::Timeout(_) => TEMP_FAIL,
            };
        }
    }
    FAILURE
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use super::*;

    #[test]
    fn typed_error_is_found_behind_context() {
        let error = Err::<(), _>(PlatformError::Timeout(30))
            .context("failed to deploy")
            .unwrap_err();

        assert_eq!(for_error(&error), TEMP_FAIL);
    }

    #[test]
    fn untyped_errors_are_generic_failures() {
        assert_eq!(for_error(&anyhow::anyhow!("something else")), FAILURE);
    }
}
