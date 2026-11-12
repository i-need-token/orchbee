//! Client-owned release store: immutable version directories, the active-version pointer and
//! their writer lock; install and update workflows own validation, activation and retention.
mod store;
#[cfg(not(windows))]
use unix as platform;
#[cfg(windows)]
use windows as platform;
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;
pub(crate) use store::{
    ReleaseStore, open_release_file, read_pointer, valid_release_path, version_directory_parts,
};

#[cfg(test)]
mod tests;
