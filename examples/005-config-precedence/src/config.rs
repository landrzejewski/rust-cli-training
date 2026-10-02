//! Layered configuration, merged by hand – to see exactly what config libraries (step 011) do.
//!
//! Precedence (later wins), 12-factor style:
//!
//! ```text
//! 1. built-in defaults
//! 2. config files: system (/etc) < user (~/.config) < project (./.platctl.toml)
//!    or ONLY the file given with --config
//! 3. environment variables PLATCTL_*
//! 4. command-line flags
//! ```
//!
//! Every value remembers WHERE it came from – invaluable when a user asks "why is it using
//! staging?" (`platctl-config show`).

use std::{fmt, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Human,
    Json,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Human => "human",
            Self::Json => "json",
        })
    }
}

/// One layer: every field optional – a layer sets only what it knows.
/// The same shape is deserialized from files and filled from env vars / flags.
#[derive(Debug, Default, Clone, Deserialize)]
// `deny_unknown_fields` – a typo in a config file (`timout-secs`) is an error, not silently ignored.
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Layer {
    pub api_url: Option<String>,
    pub env: Option<String>,
    pub timeout_secs: Option<u64>,
    pub output: Option<OutputFormat>,
}

/// Where a value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Default,
    File(PathBuf),
    Env(&'static str),
    Flag(&'static str),
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
            Self::File(path) => write!(f, "file {}", path.display()),
            Self::Env(name) => write!(f, "env {name}"),
            Self::Flag(name) => write!(f, "flag {name}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sourced<T> {
    pub value: T,
    pub source: Source,
}

/// The effective configuration – no `Option`s left, every value has a source.
#[derive(Debug, Clone)]
pub struct Config {
    pub api_url: Sourced<String>,
    pub env: Sourced<String>,
    pub timeout_secs: Sourced<u64>,
    pub output: Sourced<OutputFormat>,
}

impl Default for Config {
    fn default() -> Self {
        let default = |value| Sourced {
            value,
            source: Source::Default,
        };
        Self {
            api_url: default("http://localhost:8080/api".to_string()),
            env: default("dev".to_string()),
            timeout_secs: Sourced {
                value: 10,
                source: Source::Default,
            },
            output: Sourced {
                value: OutputFormat::Human,
                source: Source::Default,
            },
        }
    }
}

impl Config {
    /// Applies one layer on top. `source` tells which origin to record for a given field –
    /// for files the same for all fields, for env vars/flags field-specific.
    pub fn apply(&mut self, layer: Layer, source: impl Fn(Field) -> Source) {
        // Small helper: overwrite only when the layer has a value.
        fn set<T>(target: &mut Sourced<T>, value: Option<T>, source: Source) {
            if let Some(value) = value {
                *target = Sourced { value, source };
            }
        }
        set(&mut self.api_url, layer.api_url, source(Field::ApiUrl));
        set(&mut self.env, layer.env, source(Field::Env));
        set(
            &mut self.timeout_secs,
            layer.timeout_secs,
            source(Field::Timeout),
        );
        set(&mut self.output, layer.output, source(Field::Output));
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Field {
    ApiUrl,
    Env,
    Timeout,
    Output,
}

impl Field {
    /// Environment variable for the field: prefix + SCREAMING_SNAKE_CASE name.
    pub fn env_var(self) -> &'static str {
        match self {
            Self::ApiUrl => "PLATCTL_API_URL",
            Self::Env => "PLATCTL_ENV",
            Self::Timeout => "PLATCTL_TIMEOUT_SECS",
            Self::Output => "PLATCTL_OUTPUT",
        }
    }

    pub fn flag(self) -> &'static str {
        match self {
            Self::ApiUrl => "--api-url",
            Self::Env => "--env",
            Self::Timeout => "--timeout-secs",
            Self::Output => "--output",
        }
    }
}

/// Parses a TOML config file into a layer. Errors mention the file.
pub fn parse_file(text: &str) -> Result<Layer, toml::de::Error> {
    toml::from_str(text)
}

/// Reads `PLATCTL_*` variables through a lookup function – the real environment in `main`,
/// a map in tests (no global state, no `unsafe` `set_var` in tests).
pub fn layer_from_env(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Layer> {
    let timeout_secs = match lookup(Field::Timeout.env_var()) {
        Some(raw) => Some(raw.parse().map_err(|_| {
            anyhow::anyhow!(
                "{}: expected a number of seconds, got `{raw}`",
                Field::Timeout.env_var()
            )
        })?),
        None => None,
    };
    let output = match lookup(Field::Output.env_var()) {
        Some(raw) => Some(
            <OutputFormat as clap::ValueEnum>::from_str(&raw, true).map_err(|_| {
                anyhow::anyhow!(
                    "{}: expected human|json, got `{raw}`",
                    Field::Output.env_var()
                )
            })?,
        ),
        None => None,
    };
    Ok(Layer {
        api_url: lookup(Field::ApiUrl.env_var()),
        env: lookup(Field::Env.env_var()),
        timeout_secs,
        output,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn later_layers_win_and_sources_are_recorded() {
        let mut config = Config::default();
        let file = parse_file("env = \"staging\"\ntimeout-secs = 20\n").unwrap();
        config.apply(file, |_| Source::File("user.toml".into()));

        let env = HashMap::from([("PLATCTL_ENV", "prod")]);
        let env_layer = layer_from_env(|k| env.get(k).map(|v| v.to_string())).unwrap();
        config.apply(env_layer, |f| Source::Env(f.env_var()));

        let flags = Layer {
            timeout_secs: Some(5),
            ..Layer::default()
        };
        config.apply(flags, |f| Source::Flag(f.flag()));

        assert_eq!(config.env.value, "prod");
        assert_eq!(config.env.source, Source::Env("PLATCTL_ENV"));
        assert_eq!(config.timeout_secs.value, 5);
        assert_eq!(config.timeout_secs.source, Source::Flag("--timeout-secs"));
        assert_eq!(config.api_url.source, Source::Default);
    }

    #[test]
    fn unknown_keys_in_files_are_errors() {
        let error = parse_file("timout-secs = 3").unwrap_err();

        assert!(error.to_string().contains("unknown field"), "{error}");
    }

    #[test]
    fn invalid_env_values_name_the_variable() {
        let error =
            layer_from_env(|k| (k == "PLATCTL_TIMEOUT_SECS").then(|| "soon".into())).unwrap_err();

        assert!(error.to_string().starts_with("PLATCTL_TIMEOUT_SECS"));
    }
}
