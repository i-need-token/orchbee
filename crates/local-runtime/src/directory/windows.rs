//! Windows terminal directory observations and process-local PowerShell synchronization.

// Invoke the original prompt first: preceding statements would overwrite `$?`
// on Windows PowerShell 5.1, so the wrapper captures it as a parameter default during
// binding. Single-quoted constant text also avoids that shell's command-line quoting
// differences. No launch argument is interpolated here. Typed commands get a shell-minted
// run boundary and leave OSC 133 completion to the user's shell integration. A line
// submitted with the run prefix stays out of PSReadLine history while the reader holds
// it, and the next prompt removes its session-history entry.
//
// The start asset runs an Agent as the shell's first command when the launch sets
// `ORCHBEE_AGENT_START` to the line [`windows_agent_start_command`] composes. It clears the
// variable before anything runs, marks the run like a typed line, and runs the line as if it
// were typed. Without the variable it does nothing. The command travels in the environment,
// so the startup argument stays the same fixed bytes.
//
// The assets carry no comments and indent with tabs: the interactive startup is one
// `-Command` argument, and the Host samples a command line longer than 4096 UTF-16 units
// as unreadable, which hides the idle managed shell from Agent start.
/// Fixed initialization for runtime-owned interactive PowerShell processes.
/// Shared assets are also the Host's exact interactive-launch recognition input.
#[must_use]
pub fn windows_interactive_powershell_initialization() -> String {
    concat!(
        include_str!("windows/initialization.ps1"),
        include_str!("windows/reader.ps1"),
        include_str!("windows/start.ps1")
    )
    .to_owned()
}

/// The environment variable through which a launch asks managed PowerShell to run an Agent as
/// its first command.
pub const WINDOWS_AGENT_START_VARIABLE: &str = "ORCHBEE_AGENT_START";

/// How PowerShell resolves an Agent's command name: an application or script on the search path,
/// never an alias, function or cmdlet of the same name, and the first match only.
#[must_use]
pub fn windows_powershell_application(program: &str) -> String {
    format!(
        "& (Get-Command '{program}' -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1)"
    )
}

/// The line managed PowerShell runs as its first command: the resolved program and each argument
/// single-quoted, so none is read as a parameter or a number. `None` unless every token is
/// visible ASCII without a single quote, since PowerShell also closes a quote at typographic
/// quote marks.
#[must_use]
pub fn windows_agent_start_command(program: &str, arguments: &[String]) -> Option<String> {
    let plain = |token: &str| {
        !token.is_empty()
            && token
                .chars()
                .all(|character| character.is_ascii_graphic() && character != '\'')
    };
    if !plain(program) || !arguments.iter().all(|argument| plain(argument)) {
        return None;
    }
    Some(
        std::iter::once(windows_powershell_application(program))
            .chain(arguments.iter().map(|argument| format!("'{argument}'")))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Decodes reported paths as observations; callers separately admit launchable path syntax.
#[must_use]
pub fn windows_reported_directory(value: &str) -> Option<String> {
    let path = if let Some(uri) = value.strip_prefix("file://") {
        if uri.contains(['?', '#', '\\']) {
            return None;
        }
        let (host, path) = uri.split_once('/')?;
        if matches!(host, "." | "..")
            || host.contains(['@', ':', '%'])
            || host.chars().any(char::is_whitespace)
        {
            return None;
        }
        let mut bytes = Vec::with_capacity(path.len());
        let mut input = path.as_bytes().iter().copied();
        while let Some(byte) = input.next() {
            bytes.push(if byte == b'%' {
                let high = char::from(input.next()?).to_digit(16)?;
                let low = char::from(input.next()?).to_digit(16)?;
                (high * 16 + low) as u8
            } else {
                byte
            });
        }
        let path = String::from_utf8(bytes).ok()?.replace('/', "\\");
        if path.as_bytes().get(1) == Some(&b':') {
            path
        } else if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
            format!("\\\\{host}\\{path}")
        } else {
            return None;
        }
    } else {
        value.to_owned()
    };
    let parsed = std::path::Path::new(&path);
    if path.len() > 16 * 1024 || path.chars().any(char::is_control) || !parsed.is_absolute() {
        return None;
    }
    // Observed UNC directories are data, not permission to launch a Workspace there.
    // Keep their validation independent of the narrower Workbench launch-path contract.
    let Some(std::path::Component::Prefix(prefix)) = parsed.components().next() else {
        return None;
    };
    if !matches!(
        prefix.kind(),
        std::path::Prefix::Disk(_)
            | std::path::Prefix::VerbatimDisk(_)
            | std::path::Prefix::UNC(_, _)
            | std::path::Prefix::VerbatimUNC(_, _)
    ) {
        return None;
    }
    Some(path)
}
