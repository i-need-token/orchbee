//! Pinned private filesystem admission shared by runtime storage, Server frames and Client releases.
//! These capabilities validate handles and ownership and publish flushed files atomically; callers
//! own data meaning, publication order and cleanup.
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
use std::{
    fs::{self, File},
    io,
    path::Path,
    thread,
    time::{Duration, Instant},
};
#[cfg(unix)]
use unix::held;
#[cfg(unix)]
pub use unix::{
    file_identity, naming, open_directory, open_file, open_snapshot_file, open_tree, protect_new,
    require_private,
};
#[cfg(unix)]
pub(crate) use unix::{
    move_aside, open_ancestors, replace, require_data_root, require_exact_absolute,
};
#[cfg(windows)]
use windows::held;
#[cfg(windows)]
pub use windows::{
    file_identity, naming, open_directory, open_file, open_snapshot_file, open_tree, protect_new,
    require_private,
};
#[cfg(windows)]
pub(crate) use windows::{move_aside, open_ancestors, replace, require_exact_absolute};

/// How long an operation keeps trying while a cooperating process holds what it needs for an
/// instant. A reader of a record, a folder or a lock (a catalog listing, `session list`, `server
/// diagnose`, a stop waiting for a release, the machine hub) holds it for one read or probe, and a
/// writer holds a record it moves into place for that move: well under a millisecond, at most tens
/// of milliseconds on a loaded machine. Half a second outlasts that many times over and keeps a
/// refusal prompt when the holder is a real owner; it is spent only while held.
pub(crate) const MOMENTARY_HOLD: Duration = Duration::from_millis(500);

/// How often a held operation tries again.
const MOMENTARY_HOLD_POLL: Duration = Duration::from_millis(5);

/// Runs `attempt` again while `held` judges its outcome to be another handle's momentary hold, until
/// [`MOMENTARY_HOLD`] passes; then that outcome stands, as does any other at once.
pub(crate) fn outlast_momentary_hold<R>(
    held: impl Fn(&R) -> bool,
    mut attempt: impl FnMut() -> R,
) -> R {
    let until = Instant::now() + MOMENTARY_HOLD;
    loop {
        let outcome = attempt();
        if !held(&outcome) || Instant::now() >= until {
            return outcome;
        }
        thread::sleep(MOMENTARY_HOLD_POLL);
    }
}

/// Opens a published file to read. On Windows a writer moving a new version into place holds the
/// file with delete access for that move, which refuses a reader that shares no delete access, so
/// the read outlasts that hold.
pub(crate) fn open_published(path: &Path) -> io::Result<File> {
    outlast_momentary_hold(
        |opened: &io::Result<File>| opened.as_ref().is_err_and(held),
        || open_file(path, false, false),
    )
}

/// Removes a file, outlasting a reader's momentary hold of it, which refuses removal on Windows.
pub(crate) fn remove_file(path: &Path) -> io::Result<()> {
    outlast_momentary_hold(
        |removed: &io::Result<()>| removed.as_ref().is_err_and(held),
        || fs::remove_file(path),
    )
}

/// Removes an empty folder, outlasting a reader's momentary pin of it, which refuses removal on
/// Windows.
pub(crate) fn remove_dir(path: &Path) -> io::Result<()> {
    outlast_momentary_hold(
        |removed: &io::Result<()>| removed.as_ref().is_err_and(held),
        || fs::remove_dir(path),
    )
}
