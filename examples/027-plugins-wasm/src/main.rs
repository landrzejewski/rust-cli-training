//! `platctl-plug` – step 027: WebAssembly plugins.
//!
//! ```text
//! platctl-plug list                               plugins in the plugin dir + their metadata
//! platctl-plug run -p redact [-p other] [FILE]    pipe log lines through a plugin chain
//! platctl-plug wit                                print the plugin contract for plugin authors
//! ```
//!
//! Plugins are WebAssembly components (`wit/plugin.wit`) – any language, one binary for every
//! OS/CPU, sandboxed. Build the examples: `examples/027-plugins-wasm/build-plugins.sh`.

mod host;

use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};

use crate::host::{Limits, LoadedPlugin, PluginHost};

/// The contract, embedded so plugin authors can get it from the binary itself (step 022).
const WIT: &str = include_str!("../wit/plugin.wit");

/// Log processing with sandboxed WebAssembly plugins.
#[derive(Debug, Parser)]
#[command(name = "platctl-plug", version)]
struct Cli {
    /// Directory with *.wasm plugins
    #[arg(
        long,
        global = true,
        env = "PLATCTL_PLUGIN_DIR",
        default_value = "target/plugins"
    )]
    plugin_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List installed plugins
    List,
    /// Process log lines (FILE or stdin) through plugins, in the given order
    Run {
        /// Plugin name (from the plugin dir) or path to a .wasm file
        #[arg(short, long = "plugin", required = true)]
        plugins: Vec<String>,
        /// Instruction budget per line and plugin
        #[arg(long, default_value_t = Limits::default().fuel_per_call)]
        fuel: u64,
        /// Memory cap per plugin in MiB
        #[arg(long, default_value_t = 64)]
        max_memory_mib: usize,
        /// Input file, `-` = stdin
        #[arg(default_value = "-")]
        file: PathBuf,
    },
    /// Print the WIT interface plugins must implement
    Wit,
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
        Command::Wit => print!("{WIT}"),
        Command::List => {
            let host = PluginHost::new(Limits::default())?;
            let mut paths: Vec<PathBuf> = fs::read_dir(&cli.plugin_dir)
                .with_context(|| format!("cannot read {}", cli.plugin_dir.display()))?
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == "wasm"))
                .collect();
            paths.sort();
            for path in paths {
                // One broken plugin must not hide the others.
                match host.load(&path) {
                    Ok(plugin) => {
                        let m = &plugin.metadata;
                        println!("{:<10} {:<8} {}", m.name, m.version, m.description);
                    }
                    Err(error) => eprintln!("{}: {error:#}", path.display()),
                }
            }
        }
        Command::Run {
            plugins,
            fuel,
            max_memory_mib,
            file,
        } => {
            let host = PluginHost::new(Limits {
                fuel_per_call: fuel,
                max_memory_bytes: max_memory_mib << 20,
            })?;
            let mut chain = plugins
                .iter()
                .map(|name| host.load(&resolve(&cli.plugin_dir, name)))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let input: Box<dyn BufRead> = if file.as_os_str() == "-" {
                Box::new(io::stdin().lock())
            } else {
                Box::new(BufReader::new(File::open(&file)?))
            };
            let mut out = BufWriter::new(io::stdout().lock());
            for line in input.lines() {
                if let Some(line) = apply(&mut chain, line?)? {
                    writeln!(out, "{line}")?;
                }
            }
            out.flush()?;
        }
    }
    Ok(())
}

/// `redact` -> `<dir>/redact.wasm`; anything with a path separator or extension is a path.
fn resolve(dir: &std::path::Path, name: &str) -> PathBuf {
    let as_path = PathBuf::from(name);
    if as_path.extension().is_some() || as_path.components().count() > 1 {
        as_path
    } else {
        dir.join(format!("{name}.wasm"))
    }
}

fn apply(chain: &mut [LoadedPlugin], line: String) -> anyhow::Result<Option<String>> {
    let mut current = line;
    for plugin in chain {
        match plugin.process(&current) {
            Ok(Some(next)) => current = next,
            Ok(None) => return Ok(None),
            // A trapped instance is unusable – fail loudly instead of passing data through
            // unprocessed (a redaction plugin that silently stops redacting is a leak).
            Err(error) => bail!("plugin {} failed: {error:#}", plugin.metadata.name),
        }
    }
    Ok(Some(current))
}

#[cfg(test)]
mod tests {
    use std::{path::Path, process::Command as Process, sync::Once};

    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn resolves_plugin_names_and_paths() {
        let dir = Path::new("plugins");
        assert_eq!(resolve(dir, "redact"), dir.join("redact.wasm"));
        assert_eq!(resolve(dir, "./x.wasm"), PathBuf::from("./x.wasm"));
    }

    /// Builds the guest plugins once per test run (needs `rustup target add wasm32-wasip2`).
    fn plugin(name: &str) -> PathBuf {
        static BUILD: Once = Once::new();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target_dir = root.join("target/wasm-plugins");
        BUILD.call_once(|| {
            let status = Process::new(env!("CARGO"))
                .args([
                    "build",
                    "--release",
                    "--target",
                    "wasm32-wasip2",
                    "--manifest-path",
                ])
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins/Cargo.toml"))
                .arg("--target-dir")
                .arg(&target_dir)
                .status()
                .unwrap();
            assert!(status.success(), "building plugins failed");
        });
        target_dir.join(format!("wasm32-wasip2/release/{name}.wasm"))
    }

    fn load(name: &str) -> LoadedPlugin {
        let limits = Limits {
            fuel_per_call: 5_000_000,
            max_memory_bytes: 16 << 20,
        };
        PluginHost::new(limits)
            .unwrap()
            .load(&plugin(name))
            .unwrap()
    }

    #[test]
    fn redact_plugin_masks_and_drops() {
        let mut redact = load("redact");

        assert_eq!(redact.metadata.name, "redact");
        assert_eq!(
            redact
                .process("login ok user=ann@example.com token=abc123")
                .unwrap(),
            Some("login ok user=***@example.com token=***".to_string())
        );
        assert_eq!(redact.process("GET /healthz 200").unwrap(), None);
    }

    #[test]
    fn sandbox_denies_files_and_environment() {
        let mut rogue = load("rogue");

        let file = rogue.process("file").unwrap().unwrap();
        assert!(file.starts_with("cannot read /etc/passwd"), "{file}");
        assert_eq!(rogue.process("env").unwrap().unwrap(), "HOME = None");
    }

    #[test]
    fn infinite_loop_runs_out_of_fuel() {
        let error = load("rogue").process("loop").unwrap_err();

        assert!(format!("{error:?}").contains("fuel"), "{error:?}");
    }

    #[test]
    fn memory_limit_stops_allocation() {
        assert!(load("rogue").process("alloc").is_err());
    }
}
