//! Custom value parsers.
//!
//! A value parser is any `fn(&str) -> Result<T, E>` where `E: Into<Box<dyn Error + Send + Sync>>`
//! (a `String` works). clap calls it while parsing; an `Err` becomes a usage error naming the
//! argument and the value:
//!
//! ```text
//! error: invalid value '5x' for '--timeout <DURATION>': unknown unit `x` in `5x` (use s, m, h)
//! ```
//!
//! The handler then receives a typed, already validated value.

use std::time::Duration;

/// `30s`, `5m`, `1h30m`, `90` (seconds).
pub fn duration(input: &str) -> Result<Duration, String> {
    if let Ok(seconds) = input.parse::<u64>() {
        return Ok(Duration::from_secs(seconds));
    }
    let mut total = 0u64;
    let mut number = String::new();
    for c in input.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let value: u64 = number
            .parse()
            .map_err(|_| format!("expected a number before `{c}` in `{input}`"))?;
        let unit = match c {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            _ => return Err(format!("unknown unit `{c}` in `{input}` (use s, m, h)")),
        };
        total += value * unit;
        number.clear();
    }
    if !number.is_empty() {
        return Err(format!("missing unit after `{number}` in `{input}`"));
    }
    if total == 0 {
        return Err("duration must be greater than zero".into());
    }
    Ok(Duration::from_secs(total))
}

/// `key=value` pairs, e.g. `--label team=payments`.
pub fn key_value(input: &str) -> Result<(String, String), String> {
    let (key, value) = input
        .split_once('=')
        .ok_or_else(|| format!("expected KEY=VALUE, got `{input}`"))?;
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("invalid label key `{key}`"));
    }
    Ok((key.to_string(), value.to_string()))
}

/// Semantic version via the `semver` crate; a leading `v` is accepted (`v1.4.2`).
pub fn version(input: &str) -> Result<semver::Version, String> {
    semver::Version::parse(input.strip_prefix('v').unwrap_or(input))
        .map_err(|error| format!("`{input}` is not a semantic version: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration("90"), Ok(Duration::from_secs(90)));
        assert_eq!(duration("1h30m"), Ok(Duration::from_secs(5400)));
        assert!(duration("5x").is_err());
        assert!(duration("5m3").is_err());
        assert!(duration("0s").is_err());
    }

    #[test]
    fn labels() {
        assert_eq!(
            key_value("team=payments"),
            Ok(("team".into(), "payments".into()))
        );
        assert_eq!(key_value("note=a=b"), Ok(("note".into(), "a=b".into())));
        assert!(key_value("team").is_err());
        assert!(key_value("bad key=x").is_err());
    }

    #[test]
    fn versions() {
        assert_eq!(version("v1.4.2").unwrap().to_string(), "1.4.2");
        assert!(version("1.4").is_err());
    }
}
