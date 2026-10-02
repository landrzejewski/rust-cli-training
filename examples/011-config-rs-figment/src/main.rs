//! `platctl-settings` – step 011: the same layered configuration loaded with config-rs and
//! with figment.
//!
//! Layers (later wins): defaults < config/platctl.toml < config/platctl.<profile>.toml
//! < .env file (dotenvy, only fills variables not already set) < PLATCTL_* env < flags.

mod settings;
mod with_config_rs;
mod with_figment;

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, ValueEnum};
use settings::{LoadOptions, Overrides};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Loader {
    ConfigRs,
    Figment,
}

/// Show the effective platctl settings.
#[derive(Debug, Parser)]
#[command(name = "platctl-settings", version)]
struct Cli {
    /// Library used to load the configuration
    #[arg(long, value_enum, default_value_t = Loader::Figment)]
    loader: Loader,

    /// Directory with platctl.toml
    #[arg(long, default_value = "config")]
    config_dir: PathBuf,

    /// Profile overlay: loads platctl.<PROFILE>.toml
    // `env` (clap feature) – the flag can also come from PLATCTL_PROFILE.
    #[arg(long, env = "PLATCTL_PROFILE")]
    profile: Option<String>,

    /// Load environment variables from this file first (dotenv format)
    #[arg(long)]
    env_file: Option<PathBuf>,

    /// Override api.url
    #[arg(long)]
    api_url: Option<String>,

    /// Override api.timeout_secs
    #[arg(long)]
    timeout_secs: Option<u64>,

    /// Override deploy.default_env
    #[arg(long)]
    env: Option<String>,

    /// Show where every value comes from (figment only)
    #[arg(long)]
    explain: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // dotenvy: `from_path` sets variables from the file only if they are NOT already set –
    // the real environment keeps priority (same as `dotenv` in Node, Ruby, Python).
    if let Some(path) = &cli.env_file
        && let Err(error) = dotenvy::from_path(path)
    {
        eprintln!("error: cannot load {}: {error}", path.display());
        return ExitCode::from(78);
    }

    let options = LoadOptions {
        dir: cli.config_dir.clone(),
        profile: cli.profile.clone(),
        overrides: Overrides {
            api_url: cli.api_url.clone(),
            timeout_secs: cli.timeout_secs,
            env: cli.env.clone(),
        },
    };

    let loaded = match cli.loader {
        Loader::ConfigRs => with_config_rs::load(&options),
        Loader::Figment => {
            with_figment::load(&options).map_err(|error| anyhow::Error::from(*error))
        }
    };
    match loaded {
        Ok(settings) => {
            print!("{}", settings.to_display_toml());
            if cli.explain {
                println!();
                for (key, source) in with_figment::provenance(&options) {
                    println!("# {key:<20} <- {source}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: invalid configuration: {error:#}");
            ExitCode::from(78)
        }
    }
}

#[cfg(test)]
// `Jail::expect_with` closures must return `figment::Result<()>` – the large error type is
// dictated by the library here.
#[allow(clippy::result_large_err)]
mod tests {
    use std::path::Path;

    use figment::Jail;

    use super::*;
    use crate::settings::Settings;

    fn options(profile: Option<&str>) -> LoadOptions {
        LoadOptions {
            // Tests run with the package directory as working directory.
            dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("config"),
            profile: profile.map(Into::into),
            overrides: Overrides::default(),
        }
    }

    fn both(options: &LoadOptions) -> (Settings, Settings) {
        (
            with_config_rs::load(options).unwrap(),
            with_figment::load(options).unwrap(),
        )
    }

    // `Jail::expect_with` runs the closure with a private copy of the environment and a temp
    // working directory – env vars set here don't leak into other (parallel) tests.
    #[test]
    fn both_loaders_agree_on_files_profiles_and_flags() {
        Jail::expect_with(|_jail| {
            let mut options = options(Some("prod"));
            options.overrides.timeout_secs = Some(99);

            let (from_config_rs, from_figment) = both(&options);

            assert_eq!(from_config_rs, from_figment);
            assert_eq!(from_figment.deploy.default_env, "prod");
            assert_eq!(from_figment.deploy.regions, ["eu-west-1", "us-east-1"]);
            assert_eq!(from_figment.api.timeout_secs, 99, "flag beats profile file");
            assert_eq!(from_figment.output, "table", "base file beats default");
            Ok(())
        });
    }

    #[test]
    fn environment_beats_files() {
        Jail::expect_with(|jail| {
            jail.set_env("PLATCTL_API__TIMEOUT_SECS", "42");
            jail.set_env("PLATCTL_API__TOKEN", "secret");

            let (from_config_rs, from_figment) = both(&options(None));

            assert_eq!(from_config_rs, from_figment);
            assert_eq!(from_figment.api.timeout_secs, 42);
            assert_eq!(from_figment.api.token.as_deref(), Some("secret"));
            Ok(())
        });
    }

    #[test]
    fn figment_errors_name_the_source() {
        Jail::expect_with(|jail| {
            jail.set_env("PLATCTL_API__TIMEOUT_SECS", "soon");

            let error = with_figment::load(&options(None)).unwrap_err().to_string();

            assert!(error.to_lowercase().contains("timeout_secs"), "{error}");
            assert!(error.contains("PLATCTL_"), "{error}");
            Ok(())
        });
    }
}
