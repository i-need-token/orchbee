//! Terminal-reported directory observations, the Agent start variable and Windows shell bootstrap
//! text. Decoding is pure; only [`reported_directory`] on POSIX reads this machine's host name.
mod posix;
mod windows;
pub use posix::posix_reported_directory;
pub use windows::{
    windows_agent_start_command, windows_interactive_powershell_initialization,
    windows_powershell_application, windows_reported_directory,
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
