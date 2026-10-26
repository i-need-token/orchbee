//! Client-owned immutable release staging; update workflows own validation, activation and retention.
mod store;
#[cfg(not(windows))]
use unix as platform;
#[cfg(windows)]
use windows as platform;
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;
pub(crate) use store::{ReleaseStore, open_release_file, valid_release_path};

#[cfg(test)]
mod tests;
