//! Windows terminal directory observations and process-local PowerShell synchronization.

// Invoke the original prompt first: preceding statements would overwrite `$?`
// on Windows PowerShell 5.1. Single-quoted constant text also avoids that shell's
// command-line quoting differences. No launch argument is interpolated here.
pub(super) const WINDOWS_POWERSHELL_INITIALIZATION: &str =
    include_str!("windows_powershell_initialization.ps1");

/// Fixed initialization for runtime-owned interactive PowerShell processes.
/// Shared assets are also the Host's exact interactive-launch recognition input.
/// Encoded command helpers use only the common initialization, without reader hooks.
#[must_use]
pub fn windows_interactive_powershell_initialization() -> String {
    concat!(
        include_str!("windows_powershell_initialization.ps1"),
        include_str!("windows_powershell_reader.ps1")
    )
    .to_owned()
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
