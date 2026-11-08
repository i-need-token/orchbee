//! Windows storage identity for the Client's optional position cache.

use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) fn storage_path(runtime: &Path) -> io::Result<PathBuf> {
    let root = orchbee_local_runtime::local_state_root(|name| std::env::var_os(name))
        .ok_or(io::ErrorKind::NotFound)?;
    Ok(root
        .join("tui-position")
        .join(orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::host_endpoint_id(runtime)))
}
