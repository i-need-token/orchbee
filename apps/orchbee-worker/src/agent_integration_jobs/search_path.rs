//! Platform adapter for the folders the Agent integration writer searches before this Worker's
//! own `PATH`. macOS reads the `PATH` a new terminal's login shell leaves, because a Session the
//! window started from Finder or the Dock inherits launchd's `PATH`, which has none of the folders
//! a profile adds. Other platforms add none: a Windows window passes the registry `Path` to its
//! Session, and Linux is not a verified integration platform.
use std::{ffi::OsString, time::Instant};

#[cfg(target_os = "macos")]
mod macos;

/// The folders to search first, as one `PATH` value, read by `ceiling`; `None` adds none.
#[cfg(target_os = "macos")]
pub(super) fn first(ceiling: Instant, cancelled: impl Fn() -> bool) -> Option<OsString> {
    macos::read(ceiling, cancelled)
}

/// No folders are added here.
#[cfg(not(target_os = "macos"))]
pub(super) fn first(_ceiling: Instant, _cancelled: impl Fn() -> bool) -> Option<OsString> {
    None
}
