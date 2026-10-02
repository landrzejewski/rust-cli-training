//! Update channels and the small persistent state (selected channel, last check time).

use std::{env, fmt, fs, io, path::PathBuf};

use clap::ValueEnum;
use semver::Version;

/// A channel is a filter over ONE release list: stable = versions without a pre-release part
/// (`1.4.0`), beta = everything (`1.5.0-beta.2` too). SemVer orders `1.5.0-beta.2 < 1.5.0`, so a
/// beta user automatically moves to the final release once it ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Channel {
    Stable,
    Beta,
}

impl Channel {
    pub fn accepts(self, version: &Version) -> bool {
        match self {
            Self::Stable => version.pre.is_empty(),
            Self::Beta => true,
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        })
    }
}

pub fn is_prerelease(version: &str) -> bool {
    Version::parse(version).is_ok_and(|v| !v.pre.is_empty())
}

/// Per-user state directory – state (not config, not cache): it's written by the tool itself.
///   PLATCTL_UP_STATE_DIR (tests/demo) > $XDG_STATE_HOME/platctl-up > %LOCALAPPDATA%\platctl-up
///   > ~/.local/state/platctl-up
#[derive(Debug, Clone)]
pub struct State {
    dir: PathBuf,
}

impl State {
    pub fn load() -> Self {
        let dir = env::var_os("PLATCTL_UP_STATE_DIR")
            .map(PathBuf::from)
            .or_else(|| env::var_os("XDG_STATE_HOME").map(|d| PathBuf::from(d).join("platctl-up")))
            .or_else(|| env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("platctl-up")))
            .unwrap_or_else(|| {
                env::home_dir()
                    .unwrap_or_default()
                    .join(".local/state/platctl-up")
            });
        Self { dir }
    }

    #[cfg(test)]
    pub fn in_dir(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Default: the channel of the running build (a beta build keeps following betas).
    pub fn channel(&self) -> Channel {
        match fs::read_to_string(self.dir.join("channel")) {
            Ok(text) => Channel::from_str(text.trim(), true).unwrap_or(Channel::Stable),
            Err(_) if is_prerelease(crate::VERSION) => Channel::Beta,
            Err(_) => Channel::Stable,
        }
    }

    pub fn set_channel(&self, channel: Channel) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        fs::write(self.dir.join("channel"), format!("{channel}\n"))
    }

    pub fn last_check_stamp(&self) -> PathBuf {
        self.dir.join("last-update-check")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_channel_skips_prereleases() {
        let beta = Version::parse("1.5.0-beta.2").unwrap();
        let final_release = Version::parse("1.5.0").unwrap();

        assert!(!Channel::Stable.accepts(&beta));
        assert!(Channel::Beta.accepts(&beta));
        assert!(Channel::Stable.accepts(&final_release));
        assert!(beta < final_release, "final release supersedes its betas");
    }

    #[test]
    fn channel_round_trips_through_state_dir() {
        let dir = std::env::temp_dir().join(format!("platctl-up-test-{}", std::process::id()));
        let state = State::in_dir(dir.clone());

        state.set_channel(Channel::Beta).unwrap();
        assert_eq!(state.channel(), Channel::Beta);
        fs::remove_dir_all(dir).unwrap();
    }
}
