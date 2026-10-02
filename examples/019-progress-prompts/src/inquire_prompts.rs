//! The same prompts with `inquire` – richer widgets: autocompletion, filtering, help messages.

use inquire::{
    Confirm, MultiSelect, Password, PasswordDisplayMode, Select, Text, validator::Validation,
};

use crate::setup::Prompter;

pub struct InquirePrompter;

/// Suggestions while typing – e.g. existing service names fetched from the platform.
#[derive(Clone)]
struct ExistingNames;

impl inquire::Autocomplete for ExistingNames {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        Ok([
            "billing-api",
            "billing-worker",
            "search-api",
            "search-indexer",
        ]
        .iter()
        .filter(|name| name.starts_with(input))
        .map(|name| (*name).to_string())
        .collect())
    }

    fn get_completion(
        &mut self,
        _input: &str,
        highlighted: Option<String>,
    ) -> Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {
        Ok(highlighted)
    }
}

impl Prompter for InquirePrompter {
    fn name(&self, default: &str) -> anyhow::Result<String> {
        Ok(Text::new("Service name")
            .with_default(default)
            .with_autocomplete(ExistingNames)
            .with_help_message("↑↓ to pick a suggestion, Tab to complete")
            .with_validator(|input: &str| {
                Ok(if input.is_empty() {
                    Validation::Invalid("name must not be empty".into())
                } else {
                    Validation::Valid
                })
            })
            .prompt()?)
    }

    fn template(&self, templates: &[&str]) -> anyhow::Result<String> {
        // Typing filters the options.
        Ok(Select::new("Template", templates.to_vec())
            .prompt()?
            .to_string())
    }

    fn features(&self, features: &[&str]) -> anyhow::Result<Vec<String>> {
        Ok(MultiSelect::new("Features", features.to_vec())
            .prompt()?
            .into_iter()
            .map(String::from)
            .collect())
    }

    fn confirm(&self, summary: &str) -> anyhow::Result<bool> {
        Ok(Confirm::new(summary).with_default(false).prompt()?)
    }

    fn api_token(&self) -> anyhow::Result<String> {
        Ok(Password::new("Platform API token")
            .with_display_mode(PasswordDisplayMode::Masked)
            .without_confirmation()
            .prompt()?)
    }
}
