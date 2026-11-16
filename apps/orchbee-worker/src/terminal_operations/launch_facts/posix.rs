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
    managed.then(|| {
        std::iter::once(start.program())
            .chain(start.arguments().iter().map(String::as_str))
            .map(word)
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// One token as the shell reads it back literally. A start's tokens hold only
/// `[A-Za-z0-9._:=-]`, which zsh and bash take as they are, except a leading `=`, which zsh expands
/// to a command's path; such a token is single-quoted. The line runs as typed, so the person's
/// own alias or function of the program's name runs, as it would for a line they typed.
fn word(token: &str) -> String {
    if token.starts_with('=') {
        format!("'{token}'")
    } else {
        token.to_owned()
    }
}
