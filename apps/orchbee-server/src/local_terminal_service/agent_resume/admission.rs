//! Whether one fresh read of a restored terminal admits typing its resume line now, later or
//! never. Only a shell at its prompt, with nothing else in its console, in the retained directory
//! admits it; every doubt waits until the attempt's deadline, and every refutation skips. It reads
//! evidence only and never decides timing, ordering or consumption.
use super::{Reason, plan::ResumeShell};
use crate::{
    local_terminal_service::tab_close::evidence::ShellReport, session_coordinator::decoded_argv,
};
use orchbee_agent_observation_protocol::{
    ShellRoot, classify_shell_root, idle_shell_only, is_console_host, native_agent_basename,
};
use orchbee_terminal_protocol::{MAXIMUM_PROCESS_SAMPLE_ROWS, TerminalProcessInfo};
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
pub(super) enum Readiness {
    /// Type the line for this shell now.
    Deliver(ResumeShell),
    /// A silent, shell-only unmarked shell; the next read delivers if it says the same.
    Quiet(ResumeShell),
    /// The read cannot decide yet.
    Wait,
    /// The terminal must never receive this resume.
    Skip(Reason),
}

/// Decides one read. A running command or any other program skips as not idle, a root that is not
/// an interactive `cmd`, `powershell` or `pwsh` skips as unsupported, and a directory other than
/// `retained` skips as a mismatch. A truncated sample, an unreadable row or root, a missing
/// directory or frame waits. A shell at its prompt delivers. Managed PowerShell marks its prompt,
/// so its silence waits; an unmarked shell delivers on the second consecutive silent read.
pub(super) fn readiness(
    facts: &ResumeFacts,
    previous: Option<Readiness>,
    retained: &WorkbenchDirectoryPath,
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
    let root = sample.processes.iter().find(|p| p.pid == sample.root_pid);
    let argv = root.and_then(|root| decoded_argv(root.command_line.as_deref()));
    let initialization = orchbee_local_runtime::windows_interactive_powershell_initialization();
    let shell_root = root.map_or(ShellRoot::Unreadable, |root| {
        classify_shell_root(root.image.as_deref(), argv.as_deref(), &initialization)
    });
    if shell_root == ShellRoot::Other {
        return Readiness::Skip(Reason::UnsupportedShell);
    }
    let complete = sample.processes.len() < MAXIMUM_PROCESS_SAMPLE_ROWS;
    if !idle_shell_only(shell_root, complete, others().map(|p| p.image.as_deref())) {
        return Readiness::Wait;
    }
    let Some(image) = root.and_then(|root| root.image.as_deref()) else {
        return Readiness::Wait;
    };
    let shell = if native_agent_basename(image) == "cmd" {
        ResumeShell::Cmd
    } else if argv
        .as_deref()
        .and_then(<[String]>::last)
        .is_some_and(|script| *script == initialization)
    {
        ResumeShell::ManagedPowerShell
    } else {
        ResumeShell::PowerShell
    };
    match &facts.directory {
        None => return Readiness::Wait,
        Some(directory) if !same_directory(directory.as_str(), retained.as_str()) => {
            return Readiness::Skip(Reason::DirectoryMismatch);
        }
        Some(_) => {}
    }
    match facts.report {
        Some(ShellReport::AtPrompt) => Readiness::Deliver(shell),
        Some(ShellReport::Silent) if shell == ResumeShell::ManagedPowerShell => Readiness::Wait,
        Some(ShellReport::Silent) if previous == Some(Readiness::Quiet(shell)) => {
            Readiness::Deliver(shell)
        }
        Some(ShellReport::Silent) => Readiness::Quiet(shell),
        None | Some(ShellReport::Running) => Readiness::Wait,
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
