//! Deployment manifest – parsing with precise, typed errors (`thiserror`).

use std::{
    fs,
    num::ParseIntError,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub struct Manifest {
    pub service: String,
    pub tag: String,
    pub replicas: u32,
}

/// Errors of this module, as an enum the caller can `match` on.
///
/// `thiserror` derives `Display` (from `#[error(...)]`) and `std::error::Error`,
/// including `source()` for `#[source]` / `#[from]` fields – the *cause chain*.
/// Library-style code (reusable modules) should expose such concrete error types.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("cannot read manifest {path}")]
    Read {
        path: PathBuf,
        // The underlying cause is kept, not formatted into the message: callers/reporters
        // decide how deep to print the chain.
        #[source]
        source: std::io::Error,
    },

    #[error("{path}:{line}: expected `key = value`")]
    Syntax { path: PathBuf, line: usize },

    #[error("{path}:{line}: invalid number for `{key}`")]
    InvalidNumber {
        path: PathBuf,
        line: usize,
        key: String,
        #[source]
        source: ParseIntError,
    },

    #[error("{path}: missing required key `{0}`", path = .1.display())]
    MissingKey(&'static str, PathBuf),
}

pub fn load(path: &Path) -> Result<Manifest, ManifestError> {
    let text = fs::read_to_string(path).map_err(|source| ManifestError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&text, path)
}

pub fn parse(text: &str, path: &Path) -> Result<Manifest, ManifestError> {
    let (mut service, mut tag, mut replicas) = (None, None, None);

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(ManifestError::Syntax {
                path: path.to_path_buf(),
                line: index + 1,
            });
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "service" => service = Some(value.to_string()),
            "tag" => tag = Some(value.to_string()),
            "replicas" => {
                replicas = Some(
                    value
                        .parse()
                        .map_err(|source| ManifestError::InvalidNumber {
                            path: path.to_path_buf(),
                            line: index + 1,
                            key: key.to_string(),
                            source,
                        })?,
                );
            }
            // Unknown keys are ignored – forward compatibility with newer manifest versions.
            _ => {}
        }
    }

    Ok(Manifest {
        service: service.ok_or_else(|| ManifestError::MissingKey("service", path.into()))?,
        tag: tag.unwrap_or_else(|| "latest".into()),
        replicas: replicas.unwrap_or(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_with_defaults_and_comments() {
        let manifest = parse("# comment\nservice = billing\n", Path::new("m")).unwrap();

        assert_eq!(manifest.tag, "latest");
        assert_eq!(manifest.replicas, 1);
    }

    #[test]
    fn reports_line_numbers_and_keeps_the_cause() {
        let error = parse("service = a\nreplicas = three\n", Path::new("m")).unwrap_err();

        assert_eq!(error.to_string(), "m:2: invalid number for `replicas`");
        // The cause is reachable through `std::error::Error::source`.
        let source = std::error::Error::source(&error).unwrap();
        assert_eq!(source.to_string(), "invalid digit found in string");
    }
}
