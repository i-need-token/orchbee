//! Process-local cwd synchronization for recognized interactive Windows shells.

use orchbee_capability_node_protocol::AgentStartCommand;
use orchbee_local_runtime::{
    windows_agent_start_command, windows_interactive_powershell_initialization,
};

/// The first-command line for a shell whose prepared arguments end in the managed startup, which
/// alone reads it; any other program, cmd and custom shells included, starts without one.
pub(super) fn agent_start(
    arguments: &[String],
    start: Option<&AgentStartCommand>,
) -> Option<String> {
    let start = start?;
    let [.., switch, script] = arguments else {
        return None;
    };
    (switch.eq_ignore_ascii_case("-Command")
        && *script == windows_interactive_powershell_initialization())
    .then(|| windows_agent_start_command(start.program(), start.arguments()))
    .flatten()
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
