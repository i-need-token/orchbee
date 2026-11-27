//! The `PATH` a new terminal's login shell leaves, for the programs this Worker starts without a
//! shell. macOS reads it, because a Session the window started from Finder or the Dock inherits
//! launchd's `PATH`, which has none of the folders a profile adds. Other platforms add none: a
//! Windows window passes the registry `Path` to its Session, and a Linux desktop session hands its
//! programs the profile's `PATH`.
use std::{ffi::OsString, time::Instant};

#[cfg(target_os = "macos")]
mod macos;

/// The folders to search first, as one `PATH` value, read by `ceiling`; `None` adds none.
#[cfg(target_os = "macos")]
pub(crate) fn first(ceiling: Instant, cancelled: impl Fn() -> bool) -> Option<OsString> {
    macos::read(ceiling, cancelled)
}

/// No folders are added here.
#[cfg(not(target_os = "macos"))]
pub(crate) fn first(_ceiling: Instant, _cancelled: impl Fn() -> bool) -> Option<OsString> {
    None
}

/// `first`'s folders, then each of `own`'s that `first` lacks, so a folder this Worker's own
/// `PATH` adds, such as its cohort's, stays found. `None` when either is not UTF-8 or `first` is
/// empty.
#[cfg(target_os = "macos")]
pub(crate) fn joined(first: &std::ffi::OsStr, own: Option<&std::ffi::OsStr>) -> Option<String> {
    let first = first.to_str().filter(|value| !value.is_empty())?;
    let own = own.map_or(Some(""), std::ffi::OsStr::to_str)?;
    let mut value = first.to_owned();
    for folder in own.split(':').filter(|folder| !folder.is_empty()) {
        if !first.split(':').any(|known| known == folder) {
            value.push(':');
            value.push_str(folder);
        }
    }
    Some(value)
}
