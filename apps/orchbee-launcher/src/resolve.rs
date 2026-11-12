//! From the launcher's own path and the store pointer beside it to the Client it runs, and from its
//! own command line to the arguments that Client receives.
use orchbee_release_records::Pointer;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// Why the launcher starts no Client.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Failure {
    /// The pointer at this path is missing, unreadable, or names no active version.
    NoActiveVersion(PathBuf),
    /// The active version's Client at this path is missing or could not start.
    ClientUnavailable(PathBuf),
}

/// The Client a launcher at `<root>\bin\<name>.exe` runs: `<root>\versions\<product>\<name>.exe`,
/// where `<product>` is the active entry of the pointer `read` returns for `<root>\active.json`.
/// The pointer grammar admits only version names, so `<product>` is one directory name.
pub(crate) fn client(
    launcher: &Path,
    read: impl FnOnce(&Path) -> Option<Vec<u8>>,
) -> Result<PathBuf, Failure> {
    let (Some(bin), Some(name)) = (launcher.parent(), launcher.file_name()) else {
        return Err(Failure::NoActiveVersion(launcher.to_path_buf()));
    };
    let root = bin.parent().unwrap_or(bin);
    let pointer = root.join("active.json");
    let Some(active) = read(&pointer)
        .as_deref()
        .and_then(Pointer::parse)
        .map(|pointer| pointer.active)
    else {
        return Err(Failure::NoActiveVersion(pointer));
    };
    Ok(root.join("versions").join(active.product).join(name))
}

/// The arguments after the program name in a Windows command line, exactly as written. The program
/// name ends at the first space or tab outside double quotes, where a quote only toggles, as the
/// Microsoft C runtime and Rust read it; the whitespace that follows it is not part of the tail.
pub(crate) fn command_tail(command_line: &str) -> &str {
    let mut quoted = false;
    for (index, character) in command_line.char_indices() {
        match character {
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                return command_line[index..].trim_start_matches([' ', '\t']);
            }
            _ => {}
        }
    }
    ""
}
