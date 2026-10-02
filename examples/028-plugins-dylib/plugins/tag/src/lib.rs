//! `tag` plugin: prefixes log lines with the region, drops empty lines. Written as a safe Rust
//! function; `declare_plugin!` generates the C ABI glue.
//!
//! Unlike a WebAssembly plugin (step 027) it runs with the host's full privileges: it reads the
//! host's environment (`PLATCTL_REGION`) – and could read files, open sockets, spawn processes.

fn process(line: &str) -> Result<Option<String>, String> {
    if line.contains("boom") {
        // Caught by the macro's catch_unwind and reported to the host as an error.
        panic!("cannot tag {line:?}");
    }
    if line.trim().is_empty() {
        return Ok(None);
    }
    let region = std::env::var("PLATCTL_REGION").unwrap_or_else(|_| "eu-west-1".to_string());
    Ok(Some(format!("[{region}] {line}")))
}

platctl_ext_api::declare_plugin!(
    name: "tag",
    description: "prefixes lines with $PLATCTL_REGION, drops empty lines",
    process: process,
);
