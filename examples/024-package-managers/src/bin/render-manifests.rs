//! `render-manifests <ARCHIVE_DIR> <OUT_DIR> [--base-url URL]`
//!
//! Release helper: computes SHA-256 of every archive in ARCHIVE_DIR and renders the package
//! manager templates (embedded with `include_str!`, step 022) into OUT_DIR.
//!
//! Placeholders:
//!   {{version}} {{description}} {{homepage}} {{repository}}
//!   {{url:<target>}}  {{sha256:<target>}}  {{SHA256:<target>}} (uppercase, winget)
//!
//! In a real pipeline this runs in CI after the build job (cargo-dist does the Homebrew part
//! itself – step 023); the outputs are committed/PR'd to the tap, scoop bucket, winget-pkgs, AUR.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &str = "Platform CLI distributed through package managers";
const HOMEPAGE: &str = env!("CARGO_PKG_HOMEPAGE");
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

const TARGETS: [(&str, &str); 5] = [
    ("aarch64-apple-darwin", "tar.gz"),
    ("x86_64-apple-darwin", "tar.gz"),
    ("aarch64-unknown-linux-musl", "tar.gz"),
    ("x86_64-unknown-linux-musl", "tar.gz"),
    ("x86_64-pc-windows-gnu", "zip"),
];

const TEMPLATES: [(&str, &str); 6] = [
    (
        "homebrew/platctl-pkg.rb",
        include_str!("../../templates/homebrew/platctl-pkg.rb"),
    ),
    (
        "scoop/platctl-pkg.json",
        include_str!("../../templates/scoop/platctl-pkg.json"),
    ),
    (
        "winget/ExampleOrg.PlatctlPkg.yaml",
        include_str!("../../templates/winget/ExampleOrg.PlatctlPkg.yaml"),
    ),
    (
        "winget/ExampleOrg.PlatctlPkg.installer.yaml",
        include_str!("../../templates/winget/ExampleOrg.PlatctlPkg.installer.yaml"),
    ),
    (
        "winget/ExampleOrg.PlatctlPkg.locale.en-US.yaml",
        include_str!("../../templates/winget/ExampleOrg.PlatctlPkg.locale.en-US.yaml"),
    ),
    ("aur/PKGBUILD", include_str!("../../templates/aur/PKGBUILD")),
];

pub fn archive_name(target: &str, extension: &str) -> String {
    format!("platctl-pkg-v{VERSION}-{target}.{extension}")
}

fn sha256_hex(path: &Path) -> anyhow::Result<String> {
    let data = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(Sha256::digest(&data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Replaces `{{key}}`; fails on any placeholder left over (typos must not reach users).
fn render(template: &str, values: &HashMap<String, String>) -> anyhow::Result<String> {
    let mut output = template.to_string();
    for (key, value) in values {
        output = output.replace(&format!("{{{{{key}}}}}"), value);
    }
    if let Some(start) = output.find("{{") {
        let end = output[start..]
            .find("}}")
            .map_or(output.len(), |e| start + e + 2);
        bail!("unresolved placeholder {}", &output[start..end]);
    }
    Ok(output)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(archive_dir), Some(out_dir)) = (args.first(), args.get(1)) else {
        bail!("usage: render-manifests <ARCHIVE_DIR> <OUT_DIR> [--base-url URL]");
    };
    let base_url = args
        .iter()
        .position(|a| a == "--base-url")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| format!("{REPOSITORY}/releases/download/v{VERSION}"));

    let mut values = HashMap::from([
        ("version".to_string(), VERSION.to_string()),
        ("description".to_string(), DESCRIPTION.to_string()),
        ("homepage".to_string(), HOMEPAGE.to_string()),
        ("repository".to_string(), REPOSITORY.to_string()),
    ]);
    for (target, extension) in TARGETS {
        let name = archive_name(target, extension);
        let sha = sha256_hex(&PathBuf::from(archive_dir).join(&name))?;
        values.insert(format!("url:{target}"), format!("{base_url}/{name}"));
        values.insert(format!("SHA256:{target}"), sha.to_uppercase());
        values.insert(format!("sha256:{target}"), sha);
    }

    for (path, template) in TEMPLATES {
        let target = Path::new(out_dir).join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, render(template, &values)?)?;
        println!("rendered {}", target.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_and_rejects_leftover_placeholders() {
        let values = HashMap::from([("version".to_string(), "1.2.0".to_string())]);

        assert_eq!(render("v{{version}}", &values).unwrap(), "v1.2.0");
        assert!(render("{{verison}}", &values).is_err());
    }

    #[test]
    fn every_template_uses_only_known_placeholders() {
        let mut values = HashMap::from([
            ("version".to_string(), String::new()),
            ("description".to_string(), String::new()),
            ("homepage".to_string(), String::new()),
            ("repository".to_string(), String::new()),
        ]);
        for (target, _) in TARGETS {
            for prefix in ["url", "sha256", "SHA256"] {
                values.insert(format!("{prefix}:{target}"), String::new());
            }
        }
        for (path, template) in TEMPLATES {
            assert!(render(template, &values).is_ok(), "{path}");
        }
    }
}
