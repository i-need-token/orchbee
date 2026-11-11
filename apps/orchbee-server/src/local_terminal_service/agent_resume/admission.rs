//! Whether one fresh read of a terminal makes a gated line worth trying now, later or never. Only a
//! shell the Host launched with prompt marks, with nothing else in its console, at its prompt, and
//! for a resume in the retained directory, is worth trying; the Host's prompt gate alone decides
//! whether anything is typed. Every doubt waits until the attempt's deadline, and every refutation
//! gives up. It reads evidence only and never decides timing, ordering or consumption; Agent start
//! delivery reads its new shells through the same rule, without a directory.
use super::{Reason, plan::ResumeShell};
use crate::local_terminal_service::tab_close::evidence::ShellReport;
use orchbee_agent_observation_protocol::{ShellRoot, idle_shell_only, is_console_host};
use orchbee_terminal_protocol::{
    MAXIMUM_PROCESS_SAMPLE_ROWS, TerminalLaunchKind, TerminalProcessInfo,
};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

/// One read of a restored terminal: the shell's report and reported directory from one fresh
/// frame, then one process sample, whose root directory stands in when the shell reports none.
#[derive(Clone, Debug, Default)]
pub(in crate::local_terminal_service) struct ResumeFacts {
    pub(in crate::local_terminal_service) report: Option<ShellReport>,
    pub(in crate::local_terminal_service) directory: Option<WorkbenchDirectoryPath>,
    pub(in crate::local_terminal_service) sample: Option<TerminalProcessInfo>,
}

/// What one read admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_terminal_service) enum Readiness {
    /// Worth one gated send of the line for this shell now.
    Try(ResumeShell),
    /// The read cannot decide yet.
    Wait,
    /// The terminal must never receive this resume.
    Skip(Reason),
}

/// Decides one read. A running command or any other program skips as not idle. The Host's own
/// launch record names the shell: managed PowerShell and marked cmd are tried, another program
/// skips as unsupported, and a Host that records no launch kind predates the prompt gate and skips
/// as too old. When `retained` is given, a directory other than it skips as a mismatch and a
/// missing directory waits. A truncated sample, an unreadable row, a missing frame and a silent
/// shell wait: only a shell at its prompt is tried.
pub(in crate::local_terminal_service) fn readiness(
    facts: &ResumeFacts,
    retained: Option<&WorkbenchDirectoryPath>,
) -> Readiness {
    if facts.report == Some(ShellReport::Running) {
        return Readiness::Skip(Reason::NotIdle);
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
        return Readiness::Skip(Reason::NotIdle);
    }
    let shell = match sample.launch_kind {
        Some(TerminalLaunchKind::ManagedPowershell) => ResumeShell::ManagedPowerShell,
        Some(TerminalLaunchKind::Cmd) => ResumeShell::Cmd,
        Some(TerminalLaunchKind::Other) => return Readiness::Skip(Reason::UnsupportedShell),
        None => return Readiness::Skip(Reason::TerminalTooOld),
    };
    let complete = sample.processes.len() < MAXIMUM_PROCESS_SAMPLE_ROWS;
    if !idle_shell_only(
        ShellRoot::Interactive,
        complete,
        others().map(|p| p.image.as_deref()),
    ) {
        return Readiness::Wait;
    }
    if let Some(retained) = retained {
        match &facts.directory {
            None => return Readiness::Wait,
            Some(directory) if !same_directory(directory.as_str(), retained.as_str()) => {
                return Readiness::Skip(Reason::DirectoryMismatch);
            }
            Some(_) => {}
        }
    }
    match facts.report {
        Some(ShellReport::AtPrompt) => Readiness::Try(shell),
        None | Some(ShellReport::Silent | ShellReport::Running) => Readiness::Wait,
    }
}

/// Windows paths compare ASCII case-insensitively. One trailing separator is trimmed, except the
/// one that makes a drive root, since `C:` alone names the drive's current directory.
pub(super) fn same_directory(left: &str, right: &str) -> bool {
    fn trimmed(path: &str) -> &str {
        match path.strip_suffix(['\\', '/']) {
            Some(rest) if !(rest.len() == 2 && rest.as_bytes()[1] == b':') => rest,
            _ => path,
        }
    }
    trimmed(left).eq_ignore_ascii_case(trimmed(right))
}
