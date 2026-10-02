//! `rogue` plugin: misbehaves on purpose to show what the sandbox prevents.
//!   line "loop"  -> never returns           (host: fuel limit stops it)
//!   line "alloc" -> grabs 1 GiB of memory   (host: memory limit, allocation fails -> trap)
//!   line "file"  -> reads /etc/passwd       (host: no preopened dirs, WASI refuses)
//!   line "env"   -> reads HOME              (host: no environment variables granted)

wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit",
});

use platctl::plugin::host::{Level, log};

struct Rogue;

impl Guest for Rogue {
    fn metadata() -> Metadata {
        Metadata {
            name: "rogue".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "misbehaving plugin for sandbox demos".to_string(),
        }
    }

    fn process(line: String) -> Option<String> {
        match line.trim() {
            "loop" => {
                let mut counter: u64 = 0;
                loop {
                    counter = std::hint::black_box(counter.wrapping_add(1));
                }
            }
            "alloc" => {
                // black_box: otherwise the optimizer removes the unused allocation entirely.
                let buffer = std::hint::black_box(vec![1u8; 1 << 30]);
                Some(format!("allocated {} bytes", buffer.len()))
            }
            "file" => Some(match std::fs::read_to_string("/etc/passwd") {
                Ok(text) => format!("stole {} bytes of /etc/passwd", text.len()),
                Err(error) => {
                    log(Level::Warn, "filesystem access denied");
                    format!("cannot read /etc/passwd: {error}")
                }
            }),
            "env" => Some(format!("HOME = {:?}", std::env::var("HOME").ok())),
            _ => Some(line),
        }
    }
}

export!(Rogue);
