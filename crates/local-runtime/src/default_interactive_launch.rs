//! Selects a usable default interactive shell from platform observations and supplies literal
//! argv and terminal environment facts. It does not start the shell, create a terminal, or
//! grant launch authority.

use std::fmt;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

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

pub fn resolve_default_interactive_launch()
-> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
    #[cfg(windows)]
    return windows::resolve(|name| std::env::var_os(name));
    #[cfg(not(windows))]
    select_default_interactive_launch(unix::usable_shell_candidates())
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

/// The arguments that run `command` in the login `shell`, as VS Code runs a shell task on macOS:
/// `-l -c <command>`. csh and tcsh accept `-l` only as their sole flag and read their `.cshrc` or
/// `.tcshrc` on every start, so they take `-c <command>` alone.
#[cfg(unix)]
pub fn login_shell_command(shell: &str, command: &str) -> Vec<String> {
    let name = shell.rsplit('/').next().unwrap_or(shell);
    let login = (!matches!(name, "csh" | "tcsh")).then(|| "-l".to_owned());
    login
        .into_iter()
        .chain(["-c".to_owned(), command.to_owned()])
        .collect()
}

pub fn default_launch_environment() -> [DefaultLaunchEnvironmentEntry; 3] {
    [
        environment_entry("TERM", "xterm-256color"),
        environment_entry("COLORTERM", "truecolor"),
        environment_entry("TERM_PROGRAM", "ghostty"),
    ]
}

/// The variables that choose a process's character locale, in the C library's precedence.
#[cfg(unix)]
const CHARACTER_LOCALE_VARIABLES: [&str; 3] = ["LC_ALL", "LC_CTYPE", "LANG"];

/// The UTF-8 locale a terminal launch receives when its environment names no character locale, and
/// the files whose presence shows that the C library can load it: macOS keeps each locale under
/// `/usr/share/locale`, and glibc installs its built-in `C.UTF-8` as `C.utf8` (Debian and Ubuntu
/// releases before glibc 2.35 as `C.UTF-8`) under `/usr/lib/locale`.
#[cfg(target_os = "macos")]
const UTF8_LOCALE: (&str, &[&str]) = ("en_US.UTF-8", &["/usr/share/locale/en_US.UTF-8/LC_CTYPE"]);
#[cfg(all(unix, not(target_os = "macos")))]
const UTF8_LOCALE: (&str, &[&str]) = (
    "C.UTF-8",
    &[
        "/usr/lib/locale/C.utf8/LC_CTYPE",
        "/usr/lib/locale/C.UTF-8/LC_CTYPE",
    ],
);

/// The `LANG` entry a terminal launch adds when `effective`, the environment the terminal would
/// otherwise start with, sets none of `LC_ALL`, `LC_CTYPE` and `LANG`. A process started from Finder,
/// the Dock or a login item inherits launchd's environment, which has none, and its shell then runs
/// in the C locale: `ls` prints `?` for each byte of a non-ASCII name and the line editor cannot
/// take non-ASCII input. Terminal.app, iTerm2, VS Code and Ghostty set `LANG` in that case too. A
/// locale already named, UTF-8 or not, is never replaced; nothing is added when this host lacks the
/// UTF-8 locale.
#[cfg(unix)]
pub fn character_locale_entry(
    effective: impl Fn(&str) -> Option<String>,
) -> Option<DefaultLaunchEnvironmentEntry> {
    add_character_locale(effective, |path| std::path::Path::new(path).is_file())
}

#[cfg(unix)]
fn add_character_locale(
    effective: impl Fn(&str) -> Option<String>,
    installed: impl Fn(&str) -> bool,
) -> Option<DefaultLaunchEnvironmentEntry> {
    let named = CHARACTER_LOCALE_VARIABLES
        .into_iter()
        .any(|name| effective(name).is_some_and(|value| !value.is_empty()));
    let (locale, files) = UTF8_LOCALE;
    (!named && files.iter().copied().any(installed)).then(|| environment_entry("LANG", locale))
}

fn environment_entry(name: &str, value: &str) -> DefaultLaunchEnvironmentEntry {
    DefaultLaunchEnvironmentEntry {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

#[cfg(test)]
mod tests;
