//! Pinned private filesystem admission shared by runtime storage, Server frames and Client releases.
//! These capabilities validate handles and ownership and publish flushed files atomically; callers
//! own data meaning, publication order and cleanup.
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub use unix::{
    file_identity, open_directory, open_file, open_snapshot_file, open_tree, protect_new,
    require_private,
};
#[cfg(unix)]
pub(crate) use unix::{
    move_aside, open_ancestors, replace, require_data_root, require_exact_absolute,
};
#[cfg(windows)]
pub use windows::{
    file_identity, open_directory, open_file, open_snapshot_file, open_tree, protect_new,
    require_private,
};
#[cfg(windows)]
pub(crate) use windows::{move_aside, open_ancestors, replace, require_exact_absolute};
