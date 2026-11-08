//! Windows storage identity for the Client's optional position cache: the record in the folder of
//! the Session whose runtime the Client admitted.

use orchbee_local_runtime::{DataRoot, SessionName, VersionedFile, VersionedRecord};
use std::{io, path::Path};

pub(super) fn storage(runtime: &Path) -> io::Result<VersionedFile> {
    let root = DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
    let name = runtime
        .file_name()
        .and_then(|leaf| leaf.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    Ok(VersionedFile::session(
        root,
        VersionedRecord::TuiPosition,
        &SessionName::parse(name)?,
    ))
}
