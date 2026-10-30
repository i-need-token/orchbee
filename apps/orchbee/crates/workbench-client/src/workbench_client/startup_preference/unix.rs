//! Unix has no admitted Client position-cache storage adapter.

use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) fn storage_path(_runtime: &Path) -> io::Result<PathBuf> {
    Err(crate::host_capabilities::Unsupported(
        crate::host_capabilities::Capability::PositionStorage,
    )
    .into())
}
