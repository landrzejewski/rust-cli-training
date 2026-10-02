//! Argument parsing – the only place where clap types appear.

pub mod builder;
pub mod derive;

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{builder, derive::Cli};
    use crate::Action;

    const ALIASES: [(&str, &str); 1] = [("bounce", "restart --force")];

    fn parse_both(args: &[&str]) -> (Action, Action) {
        let derived = Cli::try_parse_from(args).unwrap().into_action();
        let matches = builder::command(&ALIASES)
            .try_get_matches_from(args)
            .unwrap();
        (derived, builder::action(&matches, &ALIASES))
    }

    #[test]
    fn both_definitions_are_valid() {
        Cli::command().debug_assert();
        builder::command(&ALIASES).debug_assert();
    }

    #[test]
    fn both_apis_produce_the_same_action() {
        for args in [
            &["platctl", "scale", "billing", "-r", "3", "--env", "prod"][..],
            &["platctl", "-e", "staging", "restart", "search", "--force"],
            &["platctl", "restart", "search"],
        ] {
            let (derived, built) = parse_both(args);
            assert_eq!(derived, built, "{args:?}");
        }
    }

    #[test]
    fn builder_supports_runtime_subcommands() {
        let matches = builder::command(&ALIASES)
            .try_get_matches_from(["platctl", "bounce"])
            .unwrap();

        assert_eq!(
            builder::action(&matches, &ALIASES),
            Action::Alias {
                name: "bounce".into(),
                expands_to: "restart --force".into()
            }
        );
    }

    #[test]
    fn both_validate_types() {
        let args = ["platctl", "scale", "billing", "-r", "many"];

        assert!(Cli::try_parse_from(args).is_err());
        assert!(builder::command(&[]).try_get_matches_from(args).is_err());
    }
}
