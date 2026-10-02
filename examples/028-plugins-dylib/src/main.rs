//! `platctl-ext` – step 028: native plugins loaded from shared libraries.
//!
//! ```text
//! platctl-ext list                              plugins (*.dylib / *.so / *.dll) in the plugin dir
//! platctl-ext run -p tag [-p other] [FILE]      pipe log lines through a plugin chain
//! ```
//!
//! Same idea as step 027, opposite trade-offs: native speed and full access to the system,
//! but no isolation, per-platform builds, and an ABI you must keep stable yourself.
//! Build the example plugins: `cargo build -p platctl-ext-tag -p platctl-ext-crash`.

mod native;

use std::{
    env::consts::DLL_EXTENSION,
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::Context;
use clap::{Parser, Subcommand};

use crate::native::NativePlugin;

/// Log processing with native (shared library) plugins.
#[derive(Debug, Parser)]
#[command(name = "platctl-ext", version)]
struct Cli {
    /// Directory with plugin libraries (`cargo build` puts them in target/debug)
    #[arg(
        long,
        global = true,
        env = "PLATCTL_EXT_DIR",
        default_value = "target/debug"
    )]
    plugin_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List plugins in the plugin directory
    List,
    /// Process log lines (FILE or stdin) through plugins, in the given order
    Run {
        /// Plugin name (`tag` -> libtag.dylib / libtag.so / tag.dll) or a path
        #[arg(short, long = "plugin", required = true)]
        plugins: Vec<String>,
        /// Input file, `-` = stdin
        #[arg(default_value = "-")]
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::List => {
            let mut paths: Vec<PathBuf> = fs::read_dir(&cli.plugin_dir)
                .with_context(|| format!("cannot read {}", cli.plugin_dir.display()))?
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == DLL_EXTENSION))
                .collect();
            paths.sort();
            for path in paths {
                match NativePlugin::load(&path) {
                    Ok(p) => println!("{:<8} {:<8} {}", p.name, p.version, p.description),
                    Err(error) => eprintln!("skipped: {error:#}"),
                }
            }
        }
        Command::Run { plugins, file } => {
            let chain = plugins
                .iter()
                .map(|name| NativePlugin::load(&resolve(&cli.plugin_dir, name)))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let input: Box<dyn BufRead> = if file.as_os_str() == "-" {
                Box::new(io::stdin().lock())
            } else {
                Box::new(BufReader::new(File::open(&file)?))
            };
            let mut out = BufWriter::new(io::stdout().lock());
            'lines: for line in input.lines() {
                let mut current = line?;
                for plugin in &chain {
                    match plugin.process(&current)? {
                        Some(next) => current = next,
                        None => continue 'lines,
                    }
                }
                writeln!(out, "{current}")?;
            }
            out.flush()?;
        }
    }
    Ok(())
}

/// `tag` -> `<dir>/libtag.dylib` (platform-specific name via `libloading::library_filename`).
fn resolve(dir: &Path, name: &str) -> PathBuf {
    let as_path = PathBuf::from(name);
    if as_path.extension().is_some() || as_path.components().count() > 1 {
        as_path
    } else {
        dir.join(libloading::library_filename(name))
    }
}

#[cfg(test)]
mod tests {
    use std::{process::Command as Process, sync::Once};

    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// Builds the plugin crates once (separate target dir: the outer `cargo test` holds a lock
    /// on the main one).
    fn plugin(name: &str) -> PathBuf {
        static BUILD: Once = Once::new();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target_dir = root.join("target/dylib-plugins");
        BUILD.call_once(|| {
            let status = Process::new(env!("CARGO"))
                .args(["build", "-p", "platctl-ext-tag", "-p", "platctl-ext-crash"])
                .arg("--manifest-path")
                .arg(root.join("Cargo.toml"))
                .arg("--target-dir")
                .arg(&target_dir)
                .status()
                .unwrap();
            assert!(status.success(), "building plugins failed");
        });
        resolve(&target_dir.join("debug"), name)
    }

    #[test]
    fn tag_plugin_round_trip() {
        let tag = NativePlugin::load(&plugin("tag")).unwrap();

        assert_eq!(tag.name, "tag");
        let tagged = tag.process("deploy ok").unwrap().unwrap();
        assert!(
            tagged.starts_with('[') && tagged.ends_with("] deploy ok"),
            "{tagged}"
        );
        assert_eq!(tag.process("   ").unwrap(), None);
    }

    #[test]
    fn plugin_panic_becomes_an_error() {
        let tag = NativePlugin::load(&plugin("tag")).unwrap();

        let error = tag.process("boom").unwrap_err().to_string();
        assert!(error.contains("panicked: cannot tag"), "{error}");
        assert!(tag.process("still alive").is_ok());
    }

    #[test]
    fn rejects_files_that_are_not_plugins() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

        assert!(NativePlugin::load(&manifest).is_err());
    }
}
