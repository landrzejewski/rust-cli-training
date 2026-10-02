//! Command implementations – know nothing about clap; take plain values, return `Result`.

mod restart;
mod scale;

use crate::Action;

/// Single dispatch point used by every front-end.
pub fn execute(action: Action) -> anyhow::Result<String> {
    match action {
        Action::Scale {
            service,
            replicas,
            env,
        } => scale::run(&service, replicas, &env),
        Action::Restart {
            service,
            force,
            env,
        } => restart::run(&service, force, &env),
        Action::Alias { name, expands_to } => Ok(format!(
            "`{name}` would run `platctl {expands_to}` (alias expansion is left as an exercise)"
        )),
    }
}
