//! `gen-docs <OUT_DIR>` – generates release assets from the CLI definition:
//!
//! ```text
//! OUT_DIR/man/plat.1, plat-deploy.1, ...   man pages (clap_mangen)
//! OUT_DIR/completions/plat.bash, _plat, plat.fish, _plat.ps1, plat.elv
//! OUT_DIR/CLI.md                            Markdown reference (clap-markdown)
//! ```
//!
//! A separate binary (or an `xtask`) is preferred over `build.rs`: build scripts should only
//! write into `OUT_DIR` and run on every build; documentation is needed only when packaging.

use std::{fs, path::PathBuf, process::ExitCode};

use clap::{CommandFactory, ValueEnum};
use clap_complete::Shell;
use ex010_completions_man_pages::Cli;

fn main() -> ExitCode {
    let Some(out_dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: gen-docs <OUT_DIR>");
        return ExitCode::from(2);
    };
    match generate(&out_dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn generate(out_dir: &std::path::Path) -> std::io::Result<()> {
    let man_dir = out_dir.join("man");
    let completions_dir = out_dir.join("completions");
    fs::create_dir_all(&man_dir)?;
    fs::create_dir_all(&completions_dir)?;

    // One page for the tool plus one per subcommand (`plat-deploy.1`), section 1 = user commands.
    clap_mangen::generate_to(Cli::command(), &man_dir)?;

    for shell in Shell::value_variants() {
        let path =
            clap_complete::generate_to(*shell, &mut Cli::command(), "plat", &completions_dir)?;
        eprintln!("wrote {}", path.display());
    }

    fs::write(
        out_dir.join("CLI.md"),
        clap_markdown::help_markdown::<Cli>(),
    )?;
    eprintln!(
        "wrote man pages to {} and {}",
        man_dir.display(),
        out_dir.join("CLI.md").display()
    );
    Ok(())
}
