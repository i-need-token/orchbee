//! This build's Terminal Host conventions: how its process samples write a command line, and which
//! shells it starts as a terminal's interactive root. A Windows Host reports each process's own
//! command line, which only the Windows parser splits; a POSIX Host quotes every argument for a
//! POSIX shell. Neither split guesses: a line its Host could not have written is unreadable.
use orchbee_agent_observation_protocol::{InteractiveShells, ShellRoot, classify_shell_root};
#[cfg(not(windows))]
use std::borrow::Cow;

/// Argument vector for one Host-reported command line, or `None` when the Windows parser rejects
/// the line.
#[cfg(windows)]
pub(super) fn argv(line: &str) -> Option<Vec<String>> {
    winsafe::CommandLineToArgv(line).ok()
}

/// Argument vector for one Host-reported command line, or `None` when it is not the POSIX Host's
/// quoted form.
#[cfg(not(windows))]
pub(super) fn argv(line: &str) -> Option<Vec<String>> {
    orchbee_terminal_protocol::split_posix_command_line(line)
}

/// The command line a POSIX Host writes for `argv`: each argument quoted as Python's `shlex.quote`
/// quotes it, joined by one space, so [`argv`] splits it back. An empty vector has none.
#[cfg(not(windows))]
pub(super) fn line(argv: &[String]) -> Option<String> {
    (!argv.is_empty()).then(|| argv.iter().map(|a| quoted(a)).collect::<Vec<_>>().join(" "))
}

#[cfg(not(windows))]
fn quoted(argument: &str) -> Cow<'_, str> {
    let bare = |c: char| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c);
    if !argument.is_empty() && argument.chars().all(bare) {
        Cow::Borrowed(argument)
    } else {
        Cow::Owned(format!("'{}'", argument.replace('\'', "'\"'\"'")))
    }
}

/// How a terminal's root reads among the shells a Windows Host starts, including the Worker's
/// fixed PowerShell initialization.
#[cfg(windows)]
pub(super) fn shell_root(image: Option<&str>, argv: Option<&[String]>) -> ShellRoot {
    let initialization = orchbee_local_runtime::windows_interactive_powershell_initialization();
    classify_shell_root(
        image,
        argv,
        InteractiveShells::Windows {
            initialization: &initialization,
        },
    )
}

/// How a terminal's root reads among the POSIX shells a POSIX Host starts.
#[cfg(not(windows))]
pub(super) fn shell_root(image: Option<&str>, argv: Option<&[String]>) -> ShellRoot {
    classify_shell_root(image, argv, InteractiveShells::Posix)
}
