//! The local endpoints of saved machines: the machine hub's, its askpass endpoint, each askpass
//! helper's answer endpoint and each Session carrier's. Windows serves them as private named pipes,
//! macOS and Linux as Unix sockets in this environment's socket folder; either way an accepted or
//! connected stream has proved its peer is this user before a byte is read. Every one binds
//! through [`bind`] or [`bind_locked`], so on Windows this user's processes at every integrity
//! level the endpoint's label lets in connect. It names, binds and connects endpoints only; their
//! protocols belong to their owners.
//! `apps/orchbee/docs/machines.md#names-that-freeze` owns the names.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

pub use platform::{
    FolderHold, LocalListener, LocalStream, Readiness, bind, bind_locked, endpoint_exists,
    hold_folder, refused_from_above,
};
use std::path::PathBuf;

/// What every endpoint of `role` this user's environment names starts with: on Windows
/// `\\.\pipe\orchbee-<role>-`, elsewhere `<socket folder>/<role>-`.
pub fn endpoint_prefix(role: &str) -> String {
    platform::prefix(role)
}

/// The private endpoint of `role` told apart by `unique`, which holds no path separator.
pub fn private_endpoint(role: &str, unique: &str) -> PathBuf {
    PathBuf::from(format!(
        "{}{unique}{}",
        endpoint_prefix(role),
        platform::SUFFIX
    ))
}

/// Whether `endpoint` is a private endpoint of `role` this environment names, with nothing after
/// its prefix that leaves the folder.
pub fn names_private_endpoint(endpoint: &str, role: &str) -> bool {
    endpoint
        .strip_prefix(endpoint_prefix(role).as_str())
        .is_some_and(|rest| !rest.is_empty() && !rest.contains(['/', '\\']))
}
