//! Release sources and `self_update` configuration.

use std::{path::Path, process::Command, time::Duration};

use anyhow::bail;
use self_update::{
    UpdateStrategy,
    backends::{github, manifest},
    update::ReleaseUpdate,
};
use semver::Version;

use crate::{BIN, VERSION, channel::Channel};

#[derive(Debug, Clone)]
pub enum Source {
    /// `manifest.json` on any static host (S3, GitHub Pages, nginx, an internal mirror).
    Feed { manifest_url: String },
    /// GitHub Releases API. Unauthenticated: 60 requests/hour per IP – hence throttled checks
    /// and GH_TOKEN/GITHUB_TOKEN support.
    GitHub { owner: String, name: String },
}

/// Settings shared by both backends. A macro because the two builders are different types
/// with the same setter names.
macro_rules! configure_common {
    ($builder:ident, $pin:expr, $yes:expr, $quiet:expr) => {{
        $builder
            .bin_name(BIN)
            .current_version(VERSION)
            .timeout(Duration::from_secs(10))
            // Without a pin, pick the newest release even across a major version.
            .update_strategy(UpdateStrategy::Latest)
            .show_download_progress(!$quiet)
            .show_output(!$quiet)
            .no_confirm($yes)
            // Last gate before the running binary is replaced.
            .verify_binary(smoke_test);
        if let Some(tag) = $pin {
            $builder.release_tag(tag);
        }
        Box::new($builder.build()?)
    }};
}

impl Source {
    pub fn from_args(feed: Option<&str>, repo: &str) -> anyhow::Result<Self> {
        if let Some(feed) = feed {
            let manifest_url = format!("{}/manifest.json", feed.trim_end_matches('/'));
            return Ok(Self::Feed { manifest_url });
        }
        let Some((owner, name)) = repo.split_once('/') else {
            bail!("--repo must look like owner/name, got {repo:?}");
        };
        Ok(Self::GitHub {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }

    /// Newer releases in the channel that ship an archive for this platform, newest first.
    pub fn releases(&self, channel: Channel) -> anyhow::Result<Vec<Version>> {
        let target = self_update::get_target();
        let mut versions: Vec<Version> = self
            .updater(None, true, true)?
            .get_newer_releases()?
            .all()
            .iter()
            .filter(|release| release.has_target_asset(target))
            .filter_map(|release| Version::parse(release.version()).ok())
            .filter(|version| channel.accepts(version))
            .collect();
        versions.sort_by(|a, b| b.cmp(a));
        Ok(versions)
    }

    pub fn newest(&self, channel: Channel) -> anyhow::Result<Option<Version>> {
        Ok(self.releases(channel)?.into_iter().next())
    }

    /// The channel decides WHICH version; the updater is then pinned to exactly that version,
    /// so `self_update` never picks a beta for a stable user on its own.
    pub fn installer(
        &self,
        version: &Version,
        yes: bool,
    ) -> anyhow::Result<Box<dyn ReleaseUpdate>> {
        self.updater(Some(version), yes, false)
    }

    fn updater(
        &self,
        pin: Option<&Version>,
        yes: bool,
        quiet: bool,
    ) -> anyhow::Result<Box<dyn ReleaseUpdate>> {
        Ok(match self {
            Self::Feed { manifest_url } => {
                let pin = pin.map(Version::to_string);
                let mut builder = manifest::Update::configure();
                builder.manifest_url(manifest_url);
                configure_common!(builder, pin, yes, quiet)
            }
            Self::GitHub { owner, name } => {
                // GitHub tags are `v1.2.3`.
                let pin = pin.map(|version| format!("v{version}"));
                let mut builder = github::Update::configure();
                builder
                    .repo_owner(owner)
                    .repo_name(name)
                    .auth_token_from_env();
                configure_common!(builder, pin, yes, quiet)
            }
        })
    }
}

/// Runs the downloaded binary before it replaces the current one: a broken build (wrong
/// architecture, missing dynamic library, crash at startup) is rejected and nothing changes.
fn smoke_test(new_exe: &Path) -> self_update::Result<()> {
    let output = Command::new(new_exe)
        .arg("--version")
        .output()
        .map_err(|e| self_update::Error::verification_rejected(format!("cannot run: {e}")))?;
    if output.status.success() && String::from_utf8_lossy(&output.stdout).starts_with(BIN) {
        Ok(())
    } else {
        Err(self_update::Error::verification_rejected(
            "new binary failed `--version`",
        ))
    }
}

/// Binaries installed by a package manager must be updated by it: replacing the file would
/// desync its database (and is often impossible without root).
pub fn package_manager() -> Option<&'static str> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    package_manager_for(&exe.to_string_lossy())
}

fn package_manager_for(path: &str) -> Option<&'static str> {
    const MARKERS: [(&str, &str); 4] = [
        ("/Cellar/", "Homebrew"), // /opt/homebrew/bin/x -> ../Cellar/x/1.2.0/bin/x
        ("/nix/store/", "Nix"),
        ("\\scoop\\apps\\", "Scoop"),
        ("/usr/bin/", "the system package manager"),
    ];
    MARKERS
        .into_iter()
        .find(|(marker, _)| path.contains(marker))
        .map(|(_, manager)| manager)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_package_manager_installs() {
        assert_eq!(
            package_manager_for("/opt/homebrew/Cellar/platctl-up/0.2.0/bin/platctl-up"),
            Some("Homebrew")
        );
        assert_eq!(package_manager_for("/home/dev/.local/bin/platctl-up"), None);
    }

    #[test]
    fn repo_must_have_owner_and_name() {
        assert!(Source::from_args(None, "platctl").is_err());
        assert!(matches!(
            Source::from_args(Some("http://localhost:8765/"), "ignored").unwrap(),
            Source::Feed { manifest_url } if manifest_url == "http://localhost:8765/manifest.json"
        ));
    }
}
