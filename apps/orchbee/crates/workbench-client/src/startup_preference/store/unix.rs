//! Unix has no admitted Client position-cache storage adapter.

use orchbee_local_runtime::VersionedFile;
use std::{io, path::Path};

pub(super) fn storage(_runtime: &Path) -> io::Result<VersionedFile> {
    Err(crate::host_capabilities::Unsupported(
        crate::host_capabilities::Capability::PositionStorage,
    )
    .into())
}
