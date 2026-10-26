//! Unix has no admitted Client position-cache storage adapter.

use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) fn storage_path(_runtime: &Path) -> io::Result<PathBuf> {
    Err(
        crate::tui_capabilities::Unsupported(crate::tui_capabilities::Capability::PositionStorage)
            .into(),
    )
}
