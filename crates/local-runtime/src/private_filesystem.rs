//! Pinned private filesystem admission shared by runtime storage, Server frames and Client releases.
//! These capabilities validate handles and ownership; callers own data meaning, publication and cleanup.
#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
pub use unsupported::{
    file_identity, open_directory, open_file, open_snapshot_file, open_tree, protect_new,
    require_private,
};
#[cfg(windows)]
pub use windows::{
    file_identity, open_directory, open_file, open_snapshot_file, open_tree, protect_new,
    require_private,
};
#[cfg(windows)]
pub(crate) use windows::{open_ancestors, require_exact_absolute};
#[cfg(target_os = "macos")]
pub(crate) mod macos;
