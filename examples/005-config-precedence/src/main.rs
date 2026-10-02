//! `platctl-config` – step 005: configuration sources and their precedence.
//!
//! `platctl-config show`  – effective configuration with the source of every value
//! `platctl-config paths` – config files that are searched, in precedence order

mod config;

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use config::{Config, Layer, OutputFormat, Source};

#[derive(Debug, Parser)]
#[command(
    name = "platctl-config",
    version,
    about = "Show how platctl resolves its configuration"
)]
struct Cli {
    /// Use only this config file (skips the system/user/project files)
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,

    // Flags are the highest layer. They are `Option`s so we can tell "not given" from
    // "given with the default value" – a plain default would always override lower layers.
    /// Platform API URL
    #[arg(long, global = true)]
    api_url: Option<String>,
    /// Target environment
    #[arg(long, global = true)]
    env: Option<String>,
    /// Request timeout in seconds
    #[arg(long, global = true)]
    timeout_secs: Option<u64>,
    /// Output format
    #[arg(long, global = true, value_enum)]
    output: Option<OutputFormat>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the effective configuration and where each value comes from (default)
    Show,
    /// Print the config file search path
    Paths,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            // EX_CONFIG (78) – configuration error, see step 003.
            ExitCode::from(78)
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let files = match &cli.config {
        Some(path) => vec![path.clone()],
        None => search_path(),
    };

    if let Some(Command::Paths) = cli.command {
        for path in &files {
            let state = if path.exists() { "found" } else { "missing" };
            println!("{state:<8} {}", path.display());
        }
        return Ok(());
    }

    // 1. defaults
    let mut config = Config::default();

    // 2. files – implicit ones are optional, an explicit `--config` must exist.
    for path in &files {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if cli.config.is_none() && error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => {
                return Err(error).with_context(|| format!("cannot read {}", path.display()));
            }
        };
        let layer = config::parse_file(&text)
            .with_context(|| format!("invalid config file {}", path.display()))?;
        config.apply(layer, |_| Source::File(path.clone()));
    }

    // 3. environment
    let env_layer = config::layer_from_env(|name| std::env::var(name).ok())?;
    config.apply(env_layer, |field| Source::Env(field.env_var()));

    // 4. flags
    let flags = Layer {
        api_url: cli.api_url,
        env: cli.env,
        timeout_secs: cli.timeout_secs,
        output: cli.output,
    };
    config.apply(flags, |field| Source::Flag(field.flag()));

    print_config(&config);
    Ok(())
}

/// System < user < project. The user file follows the XDG Base Directory spec
/// (`$XDG_CONFIG_HOME`, default `~/.config`) – the convention for CLI tools on Linux and
/// common on macOS too (instead of `~/Library/Application Support`).
fn search_path() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/etc/platctl/config.toml")];
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")));
    if let Some(dir) = config_home {
        paths.push(dir.join("platctl").join("config.toml"));
    }
    paths.push(PathBuf::from(".platctl.toml"));
    paths
}

fn print_config(config: &Config) {
    let rows = [
        (
            "api-url",
            config.api_url.value.clone(),
            &config.api_url.source,
        ),
        ("env", config.env.value.clone(), &config.env.source),
        (
            "timeout-secs",
            config.timeout_secs.value.to_string(),
            &config.timeout_secs.source,
        ),
        (
            "output",
            config.output.value.to_string(),
            &config.output.source,
        ),
    ];
    for (key, value, source) in rows {
        println!("{key:<13} = {value:<32} # {source}");
    }
}
