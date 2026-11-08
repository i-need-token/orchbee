//! Windows storage identity for the Client's optional position cache: the folder of the Session
//! whose runtime the Client admitted.

use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) fn storage_path(runtime: &Path) -> io::Result<PathBuf> {
    let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
    let name = runtime
        .file_name()
        .and_then(|leaf| leaf.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    Ok(root
        .sessions()
        .join(orchbee_local_runtime::SessionName::parse(name)?.as_str()))
}
