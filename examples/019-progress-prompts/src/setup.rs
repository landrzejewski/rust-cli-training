//! Resolving the inputs of `init`: flags first, prompts only for what's missing, and only when
//! a human is there to answer.

/// Questions a front-end must be able to ask. Two implementations (dialoguer, inquire) plus
/// fakes in tests – the decision logic below doesn't know which one it talks to.
pub trait Prompter {
    fn name(&self, default: &str) -> anyhow::Result<String>;
    fn template(&self, templates: &[&str]) -> anyhow::Result<String>;
    fn features(&self, features: &[&str]) -> anyhow::Result<Vec<String>>;
    fn confirm(&self, summary: &str) -> anyhow::Result<bool>;
    fn api_token(&self) -> anyhow::Result<String>;
}

pub const TEMPLATES: [&str; 3] = ["http-service", "worker", "cli-tool"];
pub const FEATURES: [&str; 4] = ["postgres", "redis", "metrics", "tracing"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Plan {
    pub name: String,
    pub template: String,
    pub features: Vec<String>,
}

/// Values given as flags.
#[derive(Debug, Default)]
pub struct Given {
    pub name: Option<String>,
    pub template: Option<String>,
    pub features: Option<Vec<String>>,
    pub yes: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Create(Plan),
    Cancelled,
}

/// `interactive` = stdin and stderr are terminals. Without a terminal NEVER prompt: a CI job
/// would hang forever waiting for input. Report all missing flags at once instead.
pub fn resolve(
    given: Given,
    interactive: bool,
    prompter: &dyn Prompter,
) -> anyhow::Result<Outcome> {
    if !interactive {
        let mut missing = Vec::new();
        if given.name.is_none() {
            missing.push("--name");
        }
        if given.template.is_none() {
            missing.push("--template");
        }
        if !given.yes {
            missing.push("--yes");
        }
        anyhow::ensure!(
            missing.is_empty(),
            "not running in a terminal, cannot prompt; provide {}",
            missing.join(", ")
        );
    }

    let name = match given.name {
        Some(name) => name,
        None => prompter.name("my-service")?,
    };
    let template = match given.template {
        Some(template) => template,
        None => prompter.template(&TEMPLATES)?,
    };
    let features = match given.features {
        Some(features) => features,
        None if interactive => prompter.features(&FEATURES)?,
        None => Vec::new(),
    };
    let plan = Plan {
        name,
        template,
        features,
    };

    // Destructive/irreversible actions: confirm unless `--yes`.
    if !given.yes {
        let summary = format!(
            "create `{}` from template `{}` with [{}]?",
            plan.name,
            plan.template,
            plan.features.join(", ")
        );
        if !prompter.confirm(&summary)? {
            return Ok(Outcome::Cancelled);
        }
    }
    Ok(Outcome::Create(plan))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Scripted answers + a record of which questions were asked.
    #[derive(Default)]
    struct Fake {
        asked: RefCell<Vec<&'static str>>,
        confirm: bool,
    }

    impl Prompter for Fake {
        fn name(&self, _: &str) -> anyhow::Result<String> {
            self.asked.borrow_mut().push("name");
            Ok("billing".into())
        }
        fn template(&self, _: &[&str]) -> anyhow::Result<String> {
            self.asked.borrow_mut().push("template");
            Ok("worker".into())
        }
        fn features(&self, _: &[&str]) -> anyhow::Result<Vec<String>> {
            self.asked.borrow_mut().push("features");
            Ok(vec!["metrics".into()])
        }
        fn confirm(&self, _: &str) -> anyhow::Result<bool> {
            self.asked.borrow_mut().push("confirm");
            Ok(self.confirm)
        }
        fn api_token(&self) -> anyhow::Result<String> {
            Ok(String::new())
        }
    }

    #[test]
    fn prompts_only_for_missing_values() {
        let fake = Fake {
            confirm: true,
            ..Fake::default()
        };
        let given = Given {
            name: Some("search".into()),
            ..Given::default()
        };

        let outcome = resolve(given, true, &fake).unwrap();

        assert_eq!(*fake.asked.borrow(), ["template", "features", "confirm"]);
        assert!(matches!(outcome, Outcome::Create(plan) if plan.name == "search"));
    }

    #[test]
    fn declined_confirmation_cancels() {
        let fake = Fake::default();

        assert_eq!(
            resolve(Given::default(), true, &fake).unwrap(),
            Outcome::Cancelled
        );
    }

    #[test]
    fn non_interactive_lists_all_missing_flags_and_never_prompts() {
        let fake = Fake::default();

        let error = resolve(Given::default(), false, &fake).unwrap_err();

        assert!(
            error.to_string().contains("--name, --template, --yes"),
            "{error}"
        );
        assert!(fake.asked.borrow().is_empty());
    }

    #[test]
    fn non_interactive_with_all_flags_works() {
        let given = Given {
            name: Some("a".into()),
            template: Some("worker".into()),
            features: None,
            yes: true,
        };

        let outcome = resolve(given, false, &Fake::default()).unwrap();

        assert!(matches!(outcome, Outcome::Create(plan) if plan.features.is_empty()));
    }
}
