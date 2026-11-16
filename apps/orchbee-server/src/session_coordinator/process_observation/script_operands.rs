//! Real paths for the runtime script operands a POSIX sample leaves unverified. A package manager
//! installs a JavaScript Agent on POSIX as a bin link, and the shebang hands the runtime that link,
//! where npm's Windows shim hands it the package entry. The runtime reads the file the link
//! resolves to, so that path, read against the process's own working folder, is the evidence a
//! Windows command line carries. A row adopts it only when it proves a provider; every other row,
//! and every Windows row, stays as the Host reported it.
#[cfg(not(windows))]
use orchbee_agent_observation_protocol::{
    AgentProcessRecognition as Recognition, AgentUnresolvedProcess, classify_agent_process,
    unverified_script_operand,
};
#[cfg(not(windows))]
use orchbee_terminal_protocol::TerminalProcess;
use orchbee_terminal_protocol::TerminalProcessInfo;

/// The sample with each such operand replaced by its real path in its row's command line, or
/// `None` when no row changes.
#[cfg(not(windows))]
pub(crate) fn resolve_script_operands(sample: &TerminalProcessInfo) -> Option<TerminalProcessInfo> {
    let mut resolved: Option<TerminalProcessInfo> = None;
    for (index, row) in sample.processes.iter().enumerate() {
        if let Some(line) = resolved_line(row) {
            resolved.get_or_insert_with(|| sample.clone()).processes[index].command_line =
                Some(line);
        }
    }
    resolved
}

/// A Windows Host's command line already names the file a runtime runs, so no row changes.
#[cfg(windows)]
pub(crate) fn resolve_script_operands(
    _sample: &TerminalProcessInfo,
) -> Option<TerminalProcessInfo> {
    None
}

/// The row's command line with its unverified script operand replaced by the file it resolves to,
/// when that file proves a provider and the line still fits the Host's form and bound.
#[cfg(not(windows))]
fn resolved_line(row: &TerminalProcess) -> Option<String> {
    use std::path::Path;
    let image = row.image.as_deref()?;
    let mut argv = super::decoded_argv(row.command_line.as_deref())?;
    let index = unverified_script_operand(Some(image), Some(&argv))?;
    let operand = Path::new(&argv[index]);
    let path = if operand.is_absolute() {
        operand.to_owned()
    } else {
        let directory = Path::new(row.current_directory.as_deref()?);
        if !directory.is_absolute() {
            return None;
        }
        directory.join(operand)
    };
    let real = std::fs::canonicalize(path)
        .ok()
        .filter(|real| real.is_file())?
        .into_os_string()
        .into_string()
        .ok()?;
    argv[index] = real;
    let proven = match classify_agent_process(Some(image), Some(&argv)) {
        Recognition::Interactive { .. } | Recognition::Noninteractive { .. } => true,
        Recognition::Unresolved { provider, reason } => {
            provider.is_some() && reason == AgentUnresolvedProcess::UnclassifiedArguments
        }
        Recognition::NotAgent | Recognition::RemoteClient => false,
    };
    if !proven {
        return None;
    }
    let line = super::host_conventions::line(&argv)?;
    (super::decoded_argv(Some(&line)).as_ref() == Some(&argv)).then_some(line)
}
