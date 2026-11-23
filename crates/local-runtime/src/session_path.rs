//! The `PATH` a macOS or Linux Session Server starts with. The Server's Worker, Terminal Host and
//! terminals inherit it, so a terminal of a Session started from a release or test cohort finds that
//! cohort's own Client by its name, `orchbee` in release, even when the person's shell profile never
//! put `~/.local/bin` on `PATH`. The cohort's folder goes last, so an `orchbee` earlier on `PATH`
//! still wins, and no shell profile is ever edited.
use crate::{EnvironmentKind, RetainedChild, environment_kind};
use std::{
    ffi::{OsStr, OsString},
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::Path,
};

/// As [`crate::spawn_retained`], for a Session Server: a release or test cohort's folder, the folder
/// `executable` lies in, ends the Server's `PATH` when no entry names it. A development cohort's
/// folder is a build output folder and is left off.
pub fn spawn_session_server(executable: &Path, arguments: &[&OsStr]) -> io::Result<RetainedChild> {
    RetainedChild::spawn_with_path(executable, arguments, server_path(executable).as_deref())
}

/// The `PATH` a Session Server started from `executable` gets, or `None` when it keeps this
/// process's own, as [`spawn_session_server`] gives it.
pub(crate) fn server_path(executable: &Path) -> Option<OsString> {
    match (environment_kind(), executable.parent()) {
        (EnvironmentKind::Development, _) | (_, None) => None,
        (_, Some(folder)) => with_folder(std::env::var_os("PATH").as_deref(), folder),
    }
}

/// `inherited` with `folder` appended, or `None` when it stays as it is: an unset or empty `PATH`,
/// which the system's default search then covers, a `PATH` that already names `folder`, with or
/// without a trailing `/`, and a `folder` that is relative or holds `:`, which no `PATH` entry can
/// name.
fn with_folder(inherited: Option<&OsStr>, folder: &Path) -> Option<OsString> {
    let folder = folder.as_os_str().as_bytes();
    let inherited = inherited.map(OsStrExt::as_bytes).unwrap_or_default();
    if inherited.is_empty() || !folder.starts_with(b"/") || folder.contains(&b':') {
        return None;
    }
    let trimmed = |entry: &[u8]| -> Vec<u8> {
        let mut entry = entry.to_vec();
        while entry.len() > 1 && entry.ends_with(b"/") {
            entry.pop();
        }
        entry
    };
    let wanted = trimmed(folder);
    if inherited
        .split(|byte| *byte == b':')
        .any(|entry| trimmed(entry) == wanted)
    {
        return None;
    }
    let mut value = inherited.to_vec();
    value.push(b':');
    value.extend_from_slice(folder);
    Some(OsString::from_vec(value))
}

#[cfg(test)]
mod tests;
