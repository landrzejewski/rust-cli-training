//! Loading with `figment`: providers merged into a `Figment`, extracted into a type.
//! Figment tracks the origin (metadata) of every value – great error messages and provenance.

use figment::{
    Figment, Metadata, Profile, Provider,
    providers::{Env, Format, Serialized, Toml},
    value::{Dict, Map},
};

use crate::settings::{LoadOptions, Overrides, Settings};

/// A custom `Provider`: anything that can describe itself (`metadata`) and produce a dictionary
/// of values per profile (`data`) can be merged into a `Figment` – e.g. flags, a remote config
/// service, a database.
struct Flags(Overrides);

impl Provider for Flags {
    fn metadata(&self) -> Metadata {
        Metadata::named("command-line flags")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        // Only flags that were given, as a nested dictionary: {api: {url, timeout_secs}, ...}.
        let mut api = Dict::new();
        if let Some(url) = &self.0.api_url {
            api.insert("url".into(), url.clone().into());
        }
        if let Some(timeout) = self.0.timeout_secs {
            api.insert("timeout_secs".into(), timeout.into());
        }
        let mut deploy = Dict::new();
        if let Some(env) = &self.0.env {
            deploy.insert("default_env".into(), env.clone().into());
        }

        let mut root = Dict::new();
        for (key, section) in [("api", api), ("deploy", deploy)] {
            if !section.is_empty() {
                root.insert(key.into(), section.into());
            }
        }
        // Values for the default profile.
        Ok(Profile::Default.collect(root))
    }
}

pub fn figment(options: &LoadOptions) -> Figment {
    let mut figment = Figment::from(Serialized::defaults(Settings::default()))
        // `merge` – later providers override earlier ones (`join` would keep existing values).
        // `Toml::file` – a missing file is simply empty.
        .merge(Toml::file(options.dir.join("platctl.toml")));

    if let Some(profile) = &options.profile {
        figment = figment.merge(Toml::file(
            options.dir.join(format!("platctl.{profile}.toml")),
        ));
    }

    figment = figment
        // PLATCTL_API__TIMEOUT_SECS -> api.timeout_secs. Values are parsed: `30` is a number,
        // `[eu-west-1,us-east-1]` an array, `true` a bool.
        .merge(Env::prefixed("PLATCTL_").split("__").ignore(&["profile"]));

    // Flags: a custom provider (below) – shows up as "command-line flags" in provenance.
    figment.merge(Flags(options.overrides.clone()))
}

// `Box` – `figment::Error` is large (>200 bytes, carries metadata); boxing keeps `Result` small
// (clippy `result_large_err`).
pub fn load(options: &LoadOptions) -> Result<Settings, Box<figment::Error>> {
    // Errors name the key AND the source, e.g.
    // invalid type: found string "soon", expected u64 for key "API.TIMEOUT_SECS"
    // in `PLATCTL_` environment variable(s)
    figment(options).extract().map_err(Box::new)
}

/// Where did each value come from? (`find_metadata` returns the provider that set it.)
pub fn provenance(options: &LoadOptions) -> Vec<(String, String)> {
    let figment = figment(options);
    [
        "output",
        "api.url",
        "api.timeout_secs",
        "api.token",
        "deploy.default_env",
        "deploy.regions",
    ]
    .iter()
    .map(|key| {
        let source = figment
            .find_metadata(key)
            .map(|metadata| match &metadata.source {
                Some(source) => format!("{} ({source})", metadata.name),
                None => metadata.name.to_string(),
            })
            .unwrap_or_else(|| "-".into());
        ((*key).to_string(), source)
    })
    .collect()
}
