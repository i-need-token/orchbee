use std::fmt;

mod native;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DefaultLaunchEnvironmentEntry {
    pub(crate) name: String,
    pub(crate) value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DefaultInteractiveLaunch {
    pub(crate) executable: String,
    pub(crate) arguments: Vec<String>,
    pub(crate) environment: [DefaultLaunchEnvironmentEntry; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DefaultInteractiveLaunchError {
    NoUsableShell,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct UsableShellCandidates {
    inherited: Option<String>,
    account: Option<String>,
    fallback: Option<String>,
}

impl fmt::Display for DefaultInteractiveLaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUsableShell => formatter.write_str(
                "Worker could not resolve a usable default interactive shell executable",
            ),
        }
    }
}

impl std::error::Error for DefaultInteractiveLaunchError {}

pub(crate) fn resolve_default_interactive_launch()
-> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
    select_default_interactive_launch(native::usable_shell_candidates())
}

fn select_default_interactive_launch(
    candidates: UsableShellCandidates,
) -> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
    candidates
        .inherited
        .or(candidates.account)
        .or(candidates.fallback)
        .map(|executable| default_launch(&executable))
        .ok_or(DefaultInteractiveLaunchError::NoUsableShell)
}

fn default_launch(executable: &str) -> DefaultInteractiveLaunch {
    DefaultInteractiveLaunch {
        executable: executable.to_owned(),
        arguments: Vec::new(),
        environment: default_launch_environment(),
    }
}

pub(crate) fn default_launch_environment() -> [DefaultLaunchEnvironmentEntry; 3] {
    [
        environment_entry("TERM", "xterm-256color"),
        environment_entry("COLORTERM", "truecolor"),
        environment_entry("TERM_PROGRAM", "ghostty"),
    ]
}

fn environment_entry(name: &str, value: &str) -> DefaultLaunchEnvironmentEntry {
    DefaultLaunchEnvironmentEntry {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

#[cfg(test)]
mod tests;
