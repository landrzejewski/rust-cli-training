//! `platctl-verify` – step 025: verifying downloaded release artifacts.
//!
//! ```text
//! platctl-verify checksum <FILE> [--sums sha256.sum]       integrity: does the file match the published hash?
//! platctl-verify signature <FILE> --bundle <B> --key <K>   authenticity: signed with our key? (cosign)
//! platctl-verify signature <FILE> --bundle <B> --identity <I> --issuer <O>   keyless (Sigstore)
//! ```
//!
//! A checksum only proves the download wasn't corrupted – an attacker who replaces the archive can
//! replace the checksum too. A SIGNATURE proves who produced the artifact.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command as Process, ExitCode},
};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};

/// Verify integrity and authenticity of platctl release artifacts.
#[derive(Debug, Parser)]
#[command(name = "platctl-verify", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Compare the SHA-256 of FILE with a checksums file
    Checksum {
        file: PathBuf,
        /// `sha256sum`-style file ("<hex>  <name>" per line) [default: <FILE>.sha256]
        #[arg(long)]
        sums: Option<PathBuf>,
    },
    /// Verify a cosign signature bundle (requires `cosign` on PATH)
    Signature {
        file: PathBuf,
        /// Bundle written by `cosign sign-blob --bundle`
        #[arg(long)]
        bundle: PathBuf,
        /// Public key (key-based signing)
        #[arg(long, conflicts_with_all = ["identity", "issuer"])]
        key: Option<PathBuf>,
        /// Expected signer identity for keyless signing (e.g. the CI workflow URL)
        #[arg(long, requires = "issuer")]
        identity: Option<String>,
        /// Expected OIDC issuer for keyless signing
        #[arg(long, requires = "identity")]
        issuer: Option<String>,
        /// Skip the transparency-log check (only for artifacts signed offline, e.g. local demos)
        #[arg(long)]
        no_tlog: bool,
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
        Command::Checksum { file, sums } => {
            let sums = sums.unwrap_or_else(|| {
                let mut path = file.clone().into_os_string();
                path.push(".sha256");
                PathBuf::from(path)
            });
            let actual = sha256_hex(&file)?;
            let expected = expected_hash(&sums, &file)?;
            // Compare in constant time is not needed here (public values), but compare
            // case-insensitively: some tools publish uppercase hex (winget).
            if actual.eq_ignore_ascii_case(&expected) {
                println!("OK  {}  sha256:{actual}", file.display());
                Ok(())
            } else {
                bail!(
                    "checksum mismatch for {}\n  expected {expected}\n  actual   {actual}",
                    file.display()
                )
            }
        }
        Command::Signature {
            file,
            bundle,
            key,
            identity,
            issuer,
            no_tlog,
        } => {
            // Shelling out to the reference implementation instead of re-implementing crypto
            // verification – a deliberate choice for security-critical code.
            let mut cosign = Process::new("cosign");
            cosign.arg("verify-blob").arg("--bundle").arg(&bundle);
            match (key, identity, issuer) {
                (Some(key), _, _) => {
                    cosign.arg("--key").arg(key);
                }
                (None, Some(identity), Some(issuer)) => {
                    cosign
                        .args(["--certificate-identity", &identity])
                        .args(["--certificate-oidc-issuer", &issuer]);
                }
                _ => bail!("use --key, or --identity together with --issuer"),
            }
            if no_tlog {
                // Without Rekor there is no public, tamper-evident record of the signature.
                cosign.arg("--insecure-ignore-tlog=true");
            }
            let status = cosign
                .arg(&file)
                .status()
                .context("cannot run `cosign` – install it (brew install cosign)")?;
            if !status.success() {
                bail!("signature verification FAILED for {}", file.display());
            }
            println!("OK  {}  signature verified", file.display());
            Ok(())
        }
    }
}

fn sha256_hex(path: &Path) -> anyhow::Result<String> {
    let data = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(Sha256::digest(&data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Finds the hash for `file` in a `sha256sum`-format file: `<hex>  <name>` (or a lone hex).
fn expected_hash(sums: &Path, file: &Path) -> anyhow::Result<String> {
    let text =
        fs::read_to_string(sums).with_context(|| format!("cannot read {}", sums.display()))?;
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    parse_sums(&text, &name).with_context(|| format!("no entry for {name} in {}", sums.display()))
}

fn parse_sums(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        match parts.next() {
            // `*name` = binary mode marker in sha256sum output.
            Some(entry) if entry.trim_start_matches('*') == name => Some(hash.to_string()),
            None if hash.len() == 64 => Some(hash.to_string()),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_sha256sum_formats() {
        let sums = "aaaa  other.tar.gz\nbbbb *platctl.tar.gz\n";

        assert_eq!(parse_sums(sums, "platctl.tar.gz").as_deref(), Some("bbbb"));
        assert_eq!(parse_sums(sums, "missing"), None);
        let single = "c".repeat(64);
        assert_eq!(parse_sums(&single, "any").as_deref(), Some(single.as_str()));
    }
}
