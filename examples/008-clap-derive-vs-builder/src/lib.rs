//! Step 008 – the same CLI implemented with clap's derive API and builder API.
//!
//! Recommended layout of a non-trivial CLI crate:
//!
//! ```text
//! src/
//! ├── bin/platctl-*.rs   thin entry points: parse, call `commands::execute`, map exit code
//! ├── lib.rs             `Action` – what to do, independent of how it was parsed
//! ├── cli/               argument parsing only (clap types live here and nowhere else)
//! │   ├── derive.rs
//! │   └── builder.rs
//! └── commands/          one module per command: the behaviour, testable without clap
//!     ├── scale.rs
//!     └── restart.rs
//! ```
//!
//! Separating "parse" (cli) from "do" (commands) keeps commands testable and lets several
//! front-ends (two parsers here; later maybe a TUI or an HTTP API) share the same logic.

pub mod cli;
pub mod commands;

/// Parsed intent of the user – plain data, no clap types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Scale {
        service: String,
        replicas: u32,
        env: String,
    },
    Restart {
        service: String,
        force: bool,
        env: String,
    },
    /// User-defined alias expanded at runtime (only the builder front-end supports it).
    Alias { name: String, expands_to: String },
}
