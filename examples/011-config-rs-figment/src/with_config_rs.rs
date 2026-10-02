//! Loading with `config` (config-rs): builder of sources, merged into a tree, deserialized.

use config::{Config, Environment, File};

use crate::settings::{LoadOptions, Settings};

pub fn load(options: &LoadOptions) -> anyhow::Result<Settings> {
    let mut builder = Config::builder()
        // 1. defaults: `Config::try_from` serializes our `Default` impl into a source.
        .add_source(Config::try_from(&Settings::default())?)
        // 2. files – `required(false)`: a missing file is fine. The format is guessed from the
        //    extension (feature `toml`); `File::with_name` would also try .yaml/.json etc.
        .add_source(File::from(options.dir.join("platctl.toml")).required(false));

    if let Some(profile) = &options.profile {
        builder = builder.add_source(
            File::from(options.dir.join(format!("platctl.{profile}.toml"))).required(false),
        );
    }

    builder = builder
        // 3. environment: PLATCTL_API__TIMEOUT_SECS -> api.timeout_secs
        //    `prefix_separator("_")` after PLATCTL, `separator("__")` between nesting levels.
        //    Lists: comma-separated, only for keys listed in `with_list_parse_key`.
        .add_source(
            Environment::with_prefix("PLATCTL")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true)
                .list_separator(",")
                .with_list_parse_key("deploy.regions")
                // PLATCTL_PROFILE selects the profile; it's not a setting.
                .ignore_empty(true),
        )
        // 4. flags: `set_override_option` – ignored when `None`.
        .set_override_option("api.url", options.overrides.api_url.clone())?
        .set_override_option("api.timeout_secs", options.overrides.timeout_secs)?
        .set_override_option("deploy.default_env", options.overrides.env.clone())?;

    // Merge everything and deserialize into the typed struct.
    Ok(builder.build()?.try_deserialize()?)
}
