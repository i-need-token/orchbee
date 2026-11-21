//! Client-owned release store: immutable version directories, the active-version pointer and
//! their writer lock; install and update workflows own validation, activation and retention.

// module-layer: workflow

mod store;
#[cfg(not(windows))]
use unix as platform;
#[cfg(windows)]
use windows as platform;
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use store::numbered_name;
pub(crate) use store::{
    PathGrammar, ReleaseStore, open_listed_file, open_release_file, read_pointer, transient_name,
    valid_release_path, version_directory_parts,
};

#[cfg(test)]
mod tests;
