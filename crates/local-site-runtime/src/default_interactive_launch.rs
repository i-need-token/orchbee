use std::fmt;

#[cfg_attr(windows, path = "default_interactive_launch/windows.rs")]
mod native;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultLaunchEnvironmentEntry {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultInteractiveLaunch {
    pub executable: String,
    pub arguments: Vec<String>,
    pub environment: [DefaultLaunchEnvironmentEntry; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultInteractiveLaunchError {
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
                "Orchbee could not resolve a usable default interactive shell executable",
            ),
        }
    }
}

impl std::error::Error for DefaultInteractiveLaunchError {}

pub fn resolve_default_interactive_launch(
    #[cfg(windows)] shell: &crate::WindowsShellSettings,
) -> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
    #[cfg(windows)]
    return native::resolve(shell, |name| std::env::var_os(name));
    #[cfg(not(windows))]
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

pub fn default_launch_environment() -> [DefaultLaunchEnvironmentEntry; 3] {
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
