//! Prompts with `dialoguer` (from the console-rs family, like indicatif).

use dialoguer::{Confirm, Input, MultiSelect, Password, Select, theme::ColorfulTheme};

use crate::setup::Prompter;

pub struct DialoguerPrompter {
    theme: ColorfulTheme,
}

impl DialoguerPrompter {
    pub fn new() -> Self {
        Self {
            theme: ColorfulTheme::default(),
        }
    }
}

impl Prompter for DialoguerPrompter {
    fn name(&self, default: &str) -> anyhow::Result<String> {
        // Validation runs on Enter; invalid input shows the message and asks again.
        Ok(Input::<String>::with_theme(&self.theme)
            .with_prompt("Service name")
            .default(default.into())
            .validate_with(|input: &String| -> Result<(), &str> {
                if input
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                {
                    Ok(())
                } else {
                    Err("use lowercase letters, digits and '-'")
                }
            })
            .interact_text()?)
    }

    fn template(&self, templates: &[&str]) -> anyhow::Result<String> {
        let index = Select::with_theme(&self.theme)
            .with_prompt("Template")
            .items(templates)
            .default(0)
            .interact()?;
        Ok(templates[index].to_string())
    }

    fn features(&self, features: &[&str]) -> anyhow::Result<Vec<String>> {
        // Space toggles, Enter accepts.
        let chosen = MultiSelect::with_theme(&self.theme)
            .with_prompt("Features (space to toggle)")
            .items(features)
            .interact()?;
        Ok(chosen
            .into_iter()
            .map(|i| features[i].to_string())
            .collect())
    }

    fn confirm(&self, summary: &str) -> anyhow::Result<bool> {
        Ok(Confirm::with_theme(&self.theme)
            .with_prompt(summary)
            .default(false) // safe default for irreversible actions
            .interact()?)
    }

    fn api_token(&self) -> anyhow::Result<String> {
        // No echo; `allow_empty_password(true)` – empty = skip.
        Ok(Password::with_theme(&self.theme)
            .with_prompt("Platform API token (empty to skip)")
            .allow_empty_password(true)
            .interact()?)
    }
}
