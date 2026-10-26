//! Client-owned immutable release staging; update workflows own validation, activation and retention.
mod windows;
pub(crate) use windows::{ReleaseStore, open_release_file, valid_release_path};
