//! Whether one fresh read of a terminal makes a gated line worth trying now, later or never. Only a
//! shell the Host launched with prompt marks, with nothing else in its console, and in the expected
//! directory when one is given, is worth trying: at its prompt, or silent where the caller counts
//! silence; the Host's prompt gate alone decides whether anything is typed. Every doubt waits, and
//! every refutation refuses. It reads evidence only and never decides timing, ordering or
//! consumption. Agent session resume reads a restored shell through it with the retained directory
//! and also tries a silent one, and Agent start delivery reads a new shell through it without one.
use crate::local_terminal_service::tab_close::evidence::ShellReport;
use orchbee_agent_observation_protocol::{ShellRoot, idle_shell_only, is_console_host};
use orchbee_terminal_protocol::{
    MAXIMUM_PROCESS_SAMPLE_ROWS, TerminalLaunchKind, TerminalProcessInfo,
};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

mod directory;
#[cfg(test)]
mod tests;

use directory::same_directory;

/// One read of a terminal: the shell's report and reported directory from one fresh frame, then
/// one process sample, whose root directory stands in when the shell reports none.
#[derive(Clone, Debug, Default)]
pub(in crate::local_terminal_service) struct ResumeFacts {
    pub(in crate::local_terminal_service) report: Option<ShellReport>,
    pub(in crate::local_terminal_service) directory: Option<WorkbenchDirectoryPath>,
    pub(in crate::local_terminal_service) sample: Option<TerminalProcessInfo>,
}

/// A shell the Host launched with prompt marks, as its own launch record names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_terminal_service) enum MarkedShell {
    /// `powershell` or `pwsh` running the Worker's fixed initialization, which strips the run
    /// prefix and echoes its token in the command's start and finish markers.
    ManagedPowerShell,
    /// An interactive `cmd` whose prompt the Host marks.
    Cmd,
    /// `zsh` running the Host's managed startup, which marks its prompt and every command.
    ManagedZsh,
    /// `bash` running the Host's managed startup, which marks its prompt and every command.
    ManagedBash,
}

/// Why a read refutes the line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_terminal_service) enum Refusal {
    /// A command or another program runs.
    NotIdle,
    /// The Host launched a program without prompt marks.
    UnsupportedShell,
    /// The terminal's Host records no launch kind, so it predates the prompt gate.
    TerminalTooOld,
    /// The shell is in another directory than the expected one.
    DirectoryMismatch,
}

/// What one read admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_terminal_service) enum Readiness {
    /// Worth one gated send of the line for this shell now: its frame shows it at its prompt.
    Try(MarkedShell),
    /// Everything but the frame admits the line: the shell is silent, with no prompt mark at or
    /// above the cursor. A console host repaints its screen after a resize, which erases those
    /// marks from every later frame while the Host's prompt gate still holds the prompt, so only
    /// that gate can tell whether the shell waits at it.
    Silent(MarkedShell),
    /// The read cannot decide yet.
    Wait,
    /// The terminal must never receive the line.
    Refuse(Refusal),
}

/// Decides one read. A running command or any other program refuses as not idle. The Host's own
/// launch record names the shell: managed PowerShell, marked cmd and managed zsh or bash are tried,
/// another program refuses as unsupported, and a Host that records no launch kind predates the
/// prompt gate and refuses as too old. When `expected` is given, a directory other than it refuses
/// as a mismatch and a missing directory waits. A truncated sample, an unreadable row and a missing
/// frame wait. A shell at its prompt is tried, and a silent one is named for the caller to decide.
pub(in crate::local_terminal_service) fn readiness(
    facts: &ResumeFacts,
    expected: Option<&WorkbenchDirectoryPath>,
) -> Readiness {
    if facts.report == Some(ShellReport::Running) {
        return Readiness::Refuse(Refusal::NotIdle);
    }
    let Some(sample) = &facts.sample else {
        return Readiness::Wait;
    };
    let others = || sample.processes.iter().filter(|p| p.pid != sample.root_pid);
    if others().any(|p| {
        p.image
            .as_deref()
            .is_some_and(|image| !is_console_host(image))
    }) {
        return Readiness::Refuse(Refusal::NotIdle);
    }
    let shell = match sample.launch_kind {
        Some(TerminalLaunchKind::ManagedPowershell) => MarkedShell::ManagedPowerShell,
        Some(TerminalLaunchKind::Cmd) => MarkedShell::Cmd,
        Some(TerminalLaunchKind::ManagedZsh) => MarkedShell::ManagedZsh,
        Some(TerminalLaunchKind::ManagedBash) => MarkedShell::ManagedBash,
        Some(TerminalLaunchKind::Other) => return Readiness::Refuse(Refusal::UnsupportedShell),
        None => return Readiness::Refuse(Refusal::TerminalTooOld),
    };
    let complete = sample.processes.len() < MAXIMUM_PROCESS_SAMPLE_ROWS;
    if !idle_shell_only(
        ShellRoot::Interactive,
        complete,
        others().map(|p| p.image.as_deref()),
    ) {
        return Readiness::Wait;
    }
    if let Some(expected) = expected {
        match &facts.directory {
            None => return Readiness::Wait,
            Some(directory) if !same_directory(directory.as_str(), expected.as_str()) => {
                return Readiness::Refuse(Refusal::DirectoryMismatch);
            }
            Some(_) => {}
        }
    }
    match facts.report {
        Some(ShellReport::AtPrompt) => Readiness::Try(shell),
        Some(ShellReport::Silent) => Readiness::Silent(shell),
        None | Some(ShellReport::Running) => Readiness::Wait,
    }
}
