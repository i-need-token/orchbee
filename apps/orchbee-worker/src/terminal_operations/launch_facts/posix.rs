//! The first-command line for a POSIX launch the Host gives the managed zsh or bash startup.

use orchbee_capability_node_protocol::AgentStartCommand;

/// The first-command line for a launch the Host runs as managed zsh or bash, whose startup alone
/// reads it; any other program starts without one. This mirrors the Host's launch rule
/// (`posix_shell_integration.zig`): `zsh` or `bash` with only `-i`, `-l`, `-il`, `-li` or
/// `--login`. A Host that finds no managed startup to give removes the variable itself.
pub(super) fn agent_start(
    executable: &str,
    arguments: &[String],
    start: Option<&AgentStartCommand>,
) -> Option<String> {
    let start = start?;
    let name = executable.rsplit('/').next().unwrap_or(executable);
    let managed = matches!(name, "zsh" | "bash")
        && arguments
            .iter()
            .all(|argument| matches!(argument.as_str(), "-i" | "-l" | "-il" | "-li" | "--login"));
    // The line runs as typed, so the person's own alias or function of the program's name runs,
    // as it would for a line they typed; a typed start in an existing shell reads the same way.
    managed
        .then(|| orchbee_local_runtime::posix_command_line(start.program(), start.arguments()))
        .flatten()
}
