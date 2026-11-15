//! The window launcher's decisions: which arguments are its own, which window version it starts,
//! with which arguments and in which folder, whether it watches that start, and what it does after
//! it. They read no clock, start nothing, and see the process tree and the file system only
//! through the facts their callers pass in.
use orchbee_release_records::Pointer;
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

#[cfg(test)]
mod tests;

/// The launcher protocol variable shared with the command launcher: the launcher's own process id
/// in decimal. A window treats its parent as announcing the protocol only when the value names it.
pub(crate) const PROTOCOL_VARIABLE: &str = "ORCHBEE_RELAUNCH_V1";

/// As the first argument, followed by a process id: wait for that process before starting.
const RELAUNCH_AFTER: &str = "--relaunch-after-v1";
/// Prefixed to a version: the window was started because that version failed.
const FALLBACK_FROM: &str = "--fallback-from-v1=";
/// The window pointer, in the launcher's own folder.
const POINTER: &str = "active.json";
/// The folder of window versions, in the launcher's own folder.
const VERSIONS: &str = "versions";

/// Why the launcher starts no window.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Failure {
    /// The pointer at this path is missing, unreadable, or names no active version.
    NoActiveVersion(PathBuf),
    /// The active version's window at this path is missing or could not start.
    ClientUnavailable(PathBuf),
}

/// One window start.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Start {
    /// The window program, `versions\<version>\<the launcher's own file name>`.
    pub(crate) window: PathBuf,
    /// Its working folder, the version folder, so no window holds another version's folder.
    pub(crate) folder: PathBuf,
    /// Its arguments, one by one.
    pub(crate) arguments: Vec<OsString>,
    /// Whether it receives the protocol variable; every other start runs without it.
    pub(crate) announced: bool,
}

/// What the launcher does, in order: wait, start `first`, and start `fallback` when `after` says so.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Plan {
    /// The process to wait for before the first start: the one `--relaunch-after-v1` named, only
    /// when it is the launcher's parent.
    pub(crate) wait_for: Option<u32>,
    /// The active window version, with the arguments that remain after the launcher's own.
    pub(crate) first: Start,
    /// The previous window version, present only when the pointer names one whose program exists;
    /// the launcher then watches `first`.
    pub(crate) fallback: Option<Start>,
}

/// The decision of the window launcher at `launcher` over the pointer `read` returns for the
/// `active.json` beside it, its arguments after the program name, `parent`, the process id of its
/// parent confirmed by start times when one is known, and `exists`, whether a program file
/// exists.
///
/// # Errors
///
/// `NoActiveVersion` when the pointer names no active version, and `ClientUnavailable` when
/// neither the active window version nor a previous one can start.
pub(crate) fn plan(
    launcher: &Path,
    read: impl FnOnce(&Path) -> Option<Vec<u8>>,
    arguments: Vec<OsString>,
    parent: Option<u32>,
    exists: impl Fn(&Path) -> bool,
) -> Result<Plan, Failure> {
    let (Some(root), Some(name)) = (launcher.parent(), launcher.file_name()) else {
        return Err(Failure::NoActiveVersion(launcher.to_path_buf()));
    };
    let pointer_path = root.join(POINTER);
    let Some(pointer) = read(&pointer_path).as_deref().and_then(Pointer::parse) else {
        return Err(Failure::NoActiveVersion(pointer_path));
    };
    let (relaunch_after, arguments) = own_arguments(arguments);
    let wait_for = relaunch_after.filter(|&process| parent == Some(process));
    let active = pointer.active.product;
    let version_start = |version: &str, arguments: Vec<OsString>, announced: bool| {
        let folder = root.join(VERSIONS).join(version);
        Start {
            window: folder.join(name),
            folder,
            arguments,
            announced,
        }
    };
    // The pointer grammar admits only version names, so each is one folder name. A previous entry
    // naming the active version again would only repeat the failed start.
    let fallback = pointer
        .previous
        .map(|previous| previous.product)
        .filter(|previous| *previous != active)
        .map(|previous| {
            let mut fallback_arguments = Vec::with_capacity(arguments.len() + 1);
            // First, so a reader that takes the first such argument reads this one.
            fallback_arguments.push(OsString::from(format!("{FALLBACK_FROM}{active}")));
            fallback_arguments.extend(arguments.iter().cloned());
            version_start(&previous, fallback_arguments, false)
        })
        .filter(|fallback| exists(&fallback.window));
    let first = version_start(&active, arguments, fallback.is_some());
    if fallback.is_none() && !exists(&first.window) {
        return Err(Failure::ClientUnavailable(first.window));
    }
    Ok(Plan {
        wait_for,
        first,
        fallback,
    })
}

/// The process `--relaunch-after-v1` names when it leads `arguments`, and the arguments the window
/// receives. Only the exact switch in first place is the launcher's; it is always dropped with the
/// argument after it, which names a process only as a decimal number from 1 without a sign or a
/// leading zero. Any other argument, the switch elsewhere included, passes on unchanged.
fn own_arguments(mut arguments: Vec<OsString>) -> (Option<u32>, Vec<OsString>) {
    if arguments.first().map(OsString::as_os_str) != Some(OsStr::new(RELAUNCH_AFTER)) {
        return (None, arguments);
    }
    let taken = arguments.len().min(2);
    let process = arguments
        .drain(..taken)
        .nth(1)
        .and_then(|value| value.to_str().and_then(process_id));
    (process, arguments)
}

fn process_id(text: &str) -> Option<u32> {
    let canonical = !text.starts_with('0')
        && !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit());
    canonical.then(|| text.parse().ok()).flatten()
}

/// How a watched start came out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Watched {
    /// No window process started.
    NotStarted,
    /// The window ended inside the watch with this exit code; an `NTSTATUS` reads as negative.
    Ended(i32),
    /// The window still ran when the watch ended, or the launcher did not watch it.
    Running,
}

/// What the launcher does after a start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Next {
    /// A window stands, or ended by its own choice; the launcher exits.
    Done,
    /// Starts the previous window version once.
    Fallback,
    /// No window starts; the launcher says so and exits with a failure.
    Unavailable,
}

/// The step after a start came to `watched`, where `fallback` says whether a fallback start remains.
/// A window that cannot start, or ends with a non-zero status inside the watch, gives way to the
/// previous version once; a window that ends with status 0, as a second instance does after handing
/// its command line to a running window, is no failure.
pub(crate) fn after(watched: Watched, fallback: bool) -> Next {
    match watched {
        Watched::NotStarted if fallback => Next::Fallback,
        Watched::NotStarted => Next::Unavailable,
        Watched::Ended(code) if code != 0 && fallback => Next::Fallback,
        Watched::Ended(_) | Watched::Running => Next::Done,
    }
}
