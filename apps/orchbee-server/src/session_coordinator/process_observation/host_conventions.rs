//! This build's Terminal Host conventions: how its process samples write a command line, and which
//! shells it starts as a terminal's interactive root. A Windows Host reports each process's own
//! command line, which only the Windows parser splits; a POSIX Host quotes every argument for a
//! POSIX shell. Neither split guesses: a line its Host could not have written is unreadable.
use orchbee_agent_observation_protocol::{InteractiveShells, ShellRoot, classify_shell_root};

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
