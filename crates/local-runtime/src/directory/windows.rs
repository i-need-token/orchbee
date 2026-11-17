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
        let path = super::percent_decoded(path)?.replace('/', "\\");
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
    if path.len() > super::OBSERVATION_LIMIT
        || path.chars().any(char::is_control)
        || !parsed.is_absolute()
    {
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

/// The long-spelling equality behind [`super::same_existing_folder`]. An 8.3 short name always
/// carries `~`, so a pair without one needs no file-system lookup.
#[cfg(windows)]
pub(super) fn same_existing_folder(left: &str, right: &str) -> bool {
    if !left.contains('~') && !right.contains('~') {
        return false;
    }
    match (long_drive_folder(left), long_drive_folder(right)) {
        (Some(left), Some(right)) => {
            trimmed_separator(&left).eq_ignore_ascii_case(trimmed_separator(&right))
        }
        _ => false,
    }
}

/// The long spelling of an 8.3 short-name path, from [`long_drive_folder`]. A path without `~`
/// is never resolved, so ordinary spellings never reach the file system.
#[cfg(windows)]
pub(super) fn long_form_of_short_name(path: &str) -> Option<String> {
    if path.contains('~') {
        long_drive_folder(path)
    } else {
        None
    }
}

/// The long spelling Windows gives an absolute plain drive path on a local fixed disk or RAM disk,
/// read with `\` separators. Any other prefix, a relative path, a network, removable, optical,
/// unknown or missing drive, or a path Windows cannot resolve gives `None`.
#[cfg(windows)]
fn long_drive_folder(path: &str) -> Option<String> {
    let native = path.replace('/', "\\");
    let parsed = std::path::Path::new(&native);
    let Some(std::path::Component::Prefix(prefix)) = parsed.components().next() else {
        return None;
    };
    let std::path::Prefix::Disk(letter) = prefix.kind() else {
        return None;
    };
    if !parsed.is_absolute() || !is_local_fixed_or_ram_drive(letter) {
        return None;
    }
    winsafe::GetLongPathName(&native).ok()
}

/// Whether a drive's root is a fixed disk or a RAM disk. `GetDriveType` answers from the local
/// mount manager without contacting a server, so a remote drive is refused before
/// `GetLongPathName` could wait on its share.
#[cfg(windows)]
fn is_local_fixed_or_ram_drive(letter: u8) -> bool {
    let root = format!("{}:\\", char::from(letter));
    let drive = winsafe::GetDriveType(Some(root.as_str()));
    drive == winsafe::co::DRIVE::FIXED || drive == winsafe::co::DRIVE::RAMDISK
}

/// Drops trailing separators, but keeps the one that makes a drive root.
#[cfg(windows)]
fn trimmed_separator(path: &str) -> &str {
    let rest = path.trim_end_matches('\\');
    if rest.is_empty() || rest.ends_with(':') {
        path
    } else {
        rest
    }
}
