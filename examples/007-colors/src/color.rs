//! Deciding WHETHER to use colors – the important part; painting is easy.
//!
//! Rules (in order), as implemented by well-behaved tools (git, cargo, ripgrep):
//! 1. explicit `--color always|never` wins,
//! 2. `NO_COLOR` set to a non-empty value disables colors (https://no-color.org),
//! 3. `CLICOLOR_FORCE` set (not "0") forces colors even into pipes (https://bixense.com/clicolors),
//! 4. `TERM=dumb` – terminal without ANSI support,
//! 5. otherwise: colors only if the stream is a terminal.
//!
//! The decision is made PER STREAM: stdout may be piped (`| less`) while stderr is a terminal.

/// `--color` values; `Auto` applies the rules above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

/// Inputs of the decision – passed in explicitly so the logic is a pure, testable function.
#[derive(Debug, Default)]
pub struct Environment {
    pub no_color: Option<String>,
    pub clicolor_force: Option<String>,
    pub term: Option<String>,
    pub is_terminal: bool,
}

impl Environment {
    /// Reads the real process environment for a given stream.
    pub fn detect(is_terminal: bool) -> Self {
        Self {
            no_color: std::env::var("NO_COLOR").ok(),
            clicolor_force: std::env::var("CLICOLOR_FORCE").ok(),
            term: std::env::var("TERM").ok(),
            is_terminal,
        }
    }
}

pub fn use_color(choice: ColorChoice, env: &Environment) -> bool {
    let set = |value: &Option<String>| value.as_deref().is_some_and(|v| !v.is_empty());
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            if set(&env.no_color) {
                false
            } else if set(&env.clicolor_force) && env.clicolor_force.as_deref() != Some("0") {
                true
            } else if env.term.as_deref() == Some("dumb") {
                false
            } else {
                env.is_terminal
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(
        no_color: Option<&str>,
        force: Option<&str>,
        term: Option<&str>,
        tty: bool,
    ) -> Environment {
        Environment {
            no_color: no_color.map(Into::into),
            clicolor_force: force.map(Into::into),
            term: term.map(Into::into),
            is_terminal: tty,
        }
    }

    #[test]
    fn decision_table() {
        use ColorChoice::*;
        let cases = [
            // choice, NO_COLOR, CLICOLOR_FORCE, TERM, tty -> colors
            (Auto, None, None, Some("xterm-256color"), true, true),
            (Auto, None, None, Some("xterm-256color"), false, false),
            (Auto, Some("1"), None, None, true, false),
            (Auto, Some(""), None, None, true, true), // empty NO_COLOR is ignored
            (Auto, None, Some("1"), None, false, true),
            (Auto, None, Some("0"), None, false, false),
            (Auto, Some("1"), Some("1"), None, true, false), // NO_COLOR beats FORCE
            (Auto, None, None, Some("dumb"), true, false),
            (Always, Some("1"), None, Some("dumb"), false, true),
            (Never, None, Some("1"), None, true, false),
        ];
        for (choice, no_color, force, term, tty, expected) in cases {
            let environment = env(no_color, force, term, tty);
            assert_eq!(
                use_color(choice, &environment),
                expected,
                "{choice:?} {environment:?}"
            );
        }
    }
}
