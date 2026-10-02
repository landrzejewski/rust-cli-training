//! Minimal parser for `key=value` log lines (logfmt), e.g.
//! `ts=2026-10-01T10:00:02Z level=error service=billing msg="payment declined" id=2`

/// Ordered key/value pairs of one line (borrowed from the line – no allocations for values).
pub fn parse(line: &str) -> Vec<(&str, &str)> {
    let mut fields = Vec::new();
    let mut rest = line.trim();
    while !rest.is_empty() {
        let Some((key, after)) = rest.split_once('=') else {
            break;
        };
        let (value, remaining) = if let Some(quoted) = after.strip_prefix('"') {
            match quoted.find('"') {
                Some(end) => (&quoted[..end], &quoted[end + 1..]),
                None => (quoted, ""),
            }
        } else {
            match after.find(' ') {
                Some(end) => (&after[..end], &after[end..]),
                None => (after, ""),
            }
        };
        fields.push((key.trim(), value));
        rest = remaining.trim_start();
    }
    fields
}

pub fn get<'a>(fields: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    fields.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_and_plain_values() {
        let fields = parse(r#"level=error service=billing msg="payment declined" id=2"#);

        assert_eq!(get(&fields, "level"), Some("error"));
        assert_eq!(get(&fields, "msg"), Some("payment declined"));
        assert_eq!(get(&fields, "id"), Some("2"));
        assert_eq!(get(&fields, "missing"), None);
    }
}
