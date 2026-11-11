//! Process-local cwd synchronization for recognized interactive Windows shells.

use orchbee_capability_node_protocol::AgentStartCommand;
use orchbee_local_runtime::{
    windows_agent_start_command, windows_interactive_powershell_initialization,
};

/// The first-command line for a launch the Host runs as managed PowerShell, whose startup alone
/// reads it; any other program, cmd and custom shells included, starts without one. This mirrors
/// the Host's launch classification: PowerShell with only plain flags, which the Host completes
/// with the managed startup, or with the exact managed startup already prepared.
pub(super) fn agent_start(
    executable: &str,
    arguments: &[String],
    start: Option<&AgentStartCommand>,
) -> Option<String> {
    const PLAIN: [&str; 5] = ["-NoLogo", "-NoProfile", "-NoExit", "-Login", "-l"];
    let start = start?;
    let plain = |flags: &[String]| {
        flags
            .iter()
            .all(|flag| PLAIN.iter().any(|plain| flag.eq_ignore_ascii_case(plain)))
    };
    let prepared = match arguments {
        [flags @ .., switch, script] => {
            flags
                .iter()
                .any(|flag| flag.eq_ignore_ascii_case("-NoExit"))
                && plain(flags)
                && switch.eq_ignore_ascii_case("-Command")
                && *script == windows_interactive_powershell_initialization()
        }
        _ => false,
    };
    (is_powershell(executable) && (prepared || plain(arguments)))
        .then(|| windows_agent_start_command(start.program(), start.arguments()))
        .flatten()
}

fn is_powershell(executable: &str) -> bool {
    let name = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
    let name = name
        .get(name.len().saturating_sub(4)..)
        .filter(|suffix| suffix.eq_ignore_ascii_case(".exe"))
        .map_or(name, |_| &name[..name.len() - 4]);
    name.eq_ignore_ascii_case("pwsh") || name.eq_ignore_ascii_case("powershell")
}

pub(super) fn interactive_arguments(executable: &str, mut arguments: Vec<String>) -> Vec<String> {
    let name = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
    if !["powershell", "powershell.exe", "pwsh", "pwsh.exe"]
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
        || arguments.len() > 3
        || arguments.iter().any(|arg| {
            !["-NoLogo", "-NoProfile", "-NoExit"]
                .iter()
                .any(|flag| arg.eq_ignore_ascii_case(flag))
        })
    {
        return arguments;
    }
    if !arguments
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("-NoExit"))
    {
        arguments.push("-NoExit".to_owned());
    }
    arguments.extend([
        "-Command".to_owned(),
        windows_interactive_powershell_initialization(),
    ]);
    arguments
}

#[cfg(test)]
mod tests;
