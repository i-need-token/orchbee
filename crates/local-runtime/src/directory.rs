//! Terminal-reported directory observations, the Agent start variable and Windows shell bootstrap
//! text. Decoding is pure; only [`reported_directory`] on POSIX reads this machine's host name, and
//! [`same_existing_folder`] on Windows reads the long spelling of an existing folder.
mod posix;
mod windows;
pub use posix::posix_reported_directory;
pub use windows::{
    windows_agent_start_command, windows_cmd_agent_start_line,
    windows_interactive_powershell_initialization, windows_powershell_application,
    windows_reported_directory,
};

/// The environment variable through which a launch asks a managed shell, PowerShell on Windows
/// and zsh or bash on POSIX, to run an Agent as its first command.
pub const AGENT_START_VARIABLE: &str = "ORCHBEE_AGENT_START";

/// The longest observation either decoder admits, before callers apply narrower path bounds.
const OBSERVATION_LIMIT: usize = 16 * 1024;

/// Decodes a terminal-reported directory, an OSC 7 report or a process directory, by this
/// machine's path rules: drive and UNC paths on Windows, absolute paths and local `file:` URIs on
/// POSIX. A Server and its Terminal Host share a machine, so their reports follow its rules.
#[cfg(windows)]
#[must_use]
pub fn reported_directory(value: &str) -> Option<String> {
    windows_reported_directory(value)
}

/// Decodes a terminal-reported directory, an OSC 7 report or a process directory, by this
/// machine's path rules: drive and UNC paths on Windows, absolute paths and local `file:` URIs on
/// POSIX. A Server and its Terminal Host share a machine, so their reports follow its rules.
#[cfg(unix)]
#[must_use]
pub fn reported_directory(value: &str) -> Option<String> {
    posix_reported_directory(value, crate::local_host_name)
}

/// Whether two directory spellings name one existing Windows folder through its long spelling, so a
/// Workspace directory typed in 8.3 short form equals the long form its shell reports. Callers
/// compare spellings first and use this only for the equality it adds. Only an absolute plain drive
/// path on a local fixed disk or RAM disk is resolved, and only when one spelling has a `~`, the
/// mark of an 8.3 short name; a network, removable or optical drive is never resolved, and neither
/// are links or junctions. An unresolvable path is never equal. POSIX always answers `false`,
/// because its directory identity is already decided by the caller.
#[cfg(windows)]
#[must_use]
pub fn same_existing_folder(left: &str, right: &str) -> bool {
    windows::same_existing_folder(left, right)
}

/// See the Windows definition; POSIX has no 8.3 short spelling to resolve.
#[cfg(unix)]
#[must_use]
pub fn same_existing_folder(_left: &str, _right: &str) -> bool {
    false
}

/// The long spelling Windows gives an 8.3 short-name path, which names the folder's own final name.
/// Only a path carrying `~` is resolved, as an absolute plain drive path on a local fixed disk or
/// RAM disk; a network, removable or optical drive, any other path, a failed resolution, and every
/// POSIX path give `None`.
#[cfg(windows)]
#[must_use]
pub fn long_form_of_short_name(path: &str) -> Option<String> {
    windows::long_form_of_short_name(path)
}

/// See the Windows definition; POSIX names are never 8.3 aliases.
#[cfg(unix)]
#[must_use]
pub fn long_form_of_short_name(_path: &str) -> Option<String> {
    None
}

/// Percent-decodes a URI path once; a truncated or non-hexadecimal escape, or a result that is not
/// UTF-8, refuses it.
fn percent_decoded(path: &str) -> Option<String> {
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
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests;
