//! `redact` plugin: masks secrets and e-mail addresses in log lines, drops health-check noise.
//! Compiled to a WebAssembly component; the host never trusts it with anything but strings.

// Generates Rust bindings for the `plugin` world: a `Guest` trait for our exports, functions
// for the host imports, and the `export!` macro wiring the trait impl into the component ABI.
wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit",
});

use platctl::plugin::host::{Level, log};

const SECRET_KEYS: [&str; 4] = ["token", "password", "secret", "api_key"];

struct Redact;

impl Guest for Redact {
    fn metadata() -> Metadata {
        Metadata {
            name: "redact".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "masks key=value secrets and e-mails, drops health checks".to_string(),
        }
    }

    fn process(line: String) -> Option<String> {
        if line.contains("GET /healthz") {
            return None;
        }
        let mut masked = 0;
        let words: Vec<String> = line
            .split(' ')
            .map(|word| match mask(word) {
                Some(replacement) => {
                    masked += 1;
                    replacement
                }
                None => word.to_string(),
            })
            .collect();
        if masked > 0 {
            // Calls back into the host through the imported `host.log` function.
            log(Level::Debug, &format!("masked {masked} value(s)"));
        }
        Some(words.join(" "))
    }
}

/// `token=abc` -> `token=***`, `ann@example.com` / `user=ann@example.com` -> `***@example.com`.
fn mask(word: &str) -> Option<String> {
    let (key, value) = match word.split_once('=') {
        Some((key, value)) => (Some(key), value),
        None => (None, word),
    };
    if let Some(key) = key
        && SECRET_KEYS.contains(&key.to_ascii_lowercase().as_str())
    {
        return Some(format!("{key}=***"));
    }
    let (_, domain) = value.split_once('@')?;
    if !domain.contains('.') {
        return None;
    }
    Some(match key {
        Some(key) => format!("{key}=***@{domain}"),
        None => format!("***@{domain}"),
    })
}

export!(Redact);
