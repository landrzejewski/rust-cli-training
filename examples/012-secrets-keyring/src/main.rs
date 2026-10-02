//! `platctl-auth` – step 012: handling secrets in a CLI.
//!
//! ```text
//! platctl-auth login  [--user NAME] [--password-stdin]   prompt (no echo) or read from stdin
//! platctl-auth token                                     print the token (for scripts)
//! platctl-auth status                                    where the token comes from
//! platctl-auth logout
//! global: --store keyring|file, --token TOKEN (discouraged), env PLATCTL_TOKEN
//! ```

mod store;

use std::{
    io::{self, BufRead, IsTerminal},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use secrecy::{ExposeSecret, SecretString};
use store::{CredentialStore, EncryptedFileStore, KeyringStore};

const SERVICE: &str = "platctl-training";

#[derive(Debug, Clone, Copy, ValueEnum)]
enum StoreKind {
    /// OS keychain / credential manager / secret service
    Keyring,
    /// age-encrypted file (passphrase from prompt or PLATCTL_PASSPHRASE)
    File,
}

/// Log in to the platform and manage the stored API token.
#[derive(Debug, Parser)]
#[command(name = "platctl-auth", version)]
struct Cli {
    /// Where the token is stored
    #[arg(long, global = true, value_enum, default_value_t = StoreKind::Keyring)]
    store: StoreKind,

    /// Directory for --store file
    #[arg(long, global = true, default_value = ".platctl-credentials")]
    store_dir: PathBuf,

    /// Account name
    #[arg(long, global = true, default_value = "default")]
    user: String,

    /// Token for this invocation (visible in `ps` and shell history – prefer PLATCTL_TOKEN)
    // `hide_env_values` – `--help` must not print the current value of a secret env var.
    #[arg(long, global = true, env = "PLATCTL_TOKEN", hide_env_values = true)]
    token: Option<SecretString>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Log in and store a token
    Login {
        /// Read the password from stdin instead of prompting (CI: `echo $PW | ... --password-stdin`)
        #[arg(long)]
        password_stdin: bool,
    },
    /// Print the token to stdout (for `$(platctl-auth token)`)
    Token,
    /// Show whether and where a token is available
    Status,
    /// Remove the stored token
    Logout,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let store = open_store(&cli)?;

    match cli.command {
        Command::Login { password_stdin } => {
            let password = if password_stdin {
                read_stdin_line()?
            } else {
                prompt_password(&format!("Password for {}: ", cli.user))?
            };
            let token = exchange_for_token(&cli.user, &password)?;
            store.save(&cli.user, &token)?;
            eprintln!(
                "logged in as {}; token stored in {}",
                cli.user,
                store.name()
            );
        }
        Command::Token => match resolve_token(&cli, store.as_ref())? {
            // The ONLY place where the secret is printed – explicitly requested by the user.
            Some((token, _)) => println!("{}", token.expose_secret()),
            None => {
                eprintln!("not logged in – run `platctl-auth login`");
                return Ok(ExitCode::from(77)); // EX_NOPERM
            }
        },
        Command::Status => match resolve_token(&cli, store.as_ref())? {
            Some((token, source)) => {
                // Show only a fingerprint – enough to compare, useless to steal.
                let secret = token.expose_secret();
                let tail = &secret[secret.len().saturating_sub(4)..];
                println!("token ****{tail} from {source}");
            }
            None => println!("not logged in"),
        },
        Command::Logout => {
            store.delete(&cli.user)?;
            eprintln!("token removed from {}", store.name());
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Precedence: `--token` / `PLATCTL_TOKEN` (explicit, per invocation – CI) beats the stored
/// credential (interactive users).
fn resolve_token(
    cli: &Cli,
    store: &dyn CredentialStore,
) -> anyhow::Result<Option<(SecretString, String)>> {
    if let Some(token) = &cli.token {
        let from_env = std::env::var_os("PLATCTL_TOKEN").is_some();
        let source = if from_env {
            "PLATCTL_TOKEN"
        } else {
            "--token flag"
        };
        if !from_env {
            eprintln!("warning: --token is visible to other users (ps) and in shell history");
        }
        return Ok(Some((token.clone(), source.to_string())));
    }
    Ok(store.load(&cli.user)?.map(|token| (token, store.name())))
}

fn open_store(cli: &Cli) -> anyhow::Result<Box<dyn CredentialStore>> {
    Ok(match cli.store {
        StoreKind::Keyring => Box::new(KeyringStore {
            service: SERVICE.to_string(),
        }),
        StoreKind::File => {
            let passphrase = match std::env::var("PLATCTL_PASSPHRASE") {
                Ok(value) => value,
                Err(_) => prompt_password("Passphrase for the credential file: ")?
                    .expose_secret()
                    .to_string(),
            };
            Box::new(EncryptedFileStore {
                dir: cli.store_dir.clone(),
                passphrase: age::secrecy::SecretString::from(passphrase),
            })
        }
    })
}

/// Prompt WITHOUT echo (rpassword reads from the terminal device, not from stdin – it works even
/// when stdin is redirected). Refuse when there is no terminal: scripts must use --password-stdin.
fn prompt_password(prompt: &str) -> anyhow::Result<SecretString> {
    if !io::stderr().is_terminal() {
        anyhow::bail!("no terminal for a password prompt; use --password-stdin or env variables");
    }
    let password = rpassword::prompt_password(prompt).context("cannot read password")?;
    Ok(SecretString::from(password))
}

fn read_stdin_line() -> anyhow::Result<SecretString> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    // Strip the trailing newline from `echo`.
    let password = line.trim_end_matches(['\r', '\n']).to_string();
    anyhow::ensure!(!password.is_empty(), "empty password on stdin");
    Ok(SecretString::from(password))
}

/// Simulated login API: a real tool would POST credentials and receive a token
/// (or, better, use an OAuth device flow and never see the password at all).
fn exchange_for_token(user: &str, password: &SecretString) -> anyhow::Result<SecretString> {
    anyhow::ensure!(
        password.expose_secret().len() >= 8,
        "invalid credentials (demo: any password with 8+ characters works)"
    );
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (user, password.expose_secret(), std::process::id()).hash(&mut hasher);
    Ok(SecretString::from(format!("plt_{:016x}", hasher.finish())))
}
