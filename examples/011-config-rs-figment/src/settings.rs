//! The configuration model – plain serde structs, shared by both loaders.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// `Serialize` – the defaults are fed into the libraries as the lowest layer.
/// `Deserialize` – the merged result is deserialized into this type; a wrong type in any
/// source (`timeout_secs = "soon"`) is reported with the key name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub output: String,
    pub api: Api,
    pub deploy: Deploy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Api {
    pub url: String,
    pub timeout_secs: u64,
    /// A secret – comes from the environment / `.env`, never from committed files.
    /// (Proper secret handling: step 012.)
    pub token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deploy {
    pub default_env: String,
    pub regions: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output: "human".into(),
            api: Api {
                url: "http://localhost:8080/api".into(),
                timeout_secs: 5,
                token: None,
            },
            deploy: Deploy {
                default_env: "dev".into(),
                regions: vec!["eu-west-1".into()],
            },
        }
    }
}

impl Settings {
    /// TOML for display with the secret masked.
    pub fn to_display_toml(&self) -> String {
        let mut masked = self.clone();
        if masked.api.token.is_some() {
            masked.api.token = Some("***".into());
        }
        toml::to_string(&masked).expect("settings serialize to TOML")
    }
}

/// Inputs common to both loaders.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Directory with `platctl.toml` and `platctl.<profile>.toml`.
    pub dir: PathBuf,
    pub profile: Option<String>,
    /// Highest layer: values from command-line flags (`None` = flag not given).
    pub overrides: Overrides,
}

#[derive(Debug, Clone, Default)]
pub struct Overrides {
    pub api_url: Option<String>,
    pub timeout_secs: Option<u64>,
    pub env: Option<String>,
}
