//! Byte-preserving Unix path representation; no filesystem observation is performed.
use std::{
    ffi::OsString,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

pub(crate) fn path_admitted(path: &Path) -> bool {
    let bytes = path.as_os_str().as_bytes();
    !bytes.is_empty() && bytes.len() <= super::MAX_PATH_BYTES
}

pub(crate) fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(OsString::from_vec(bytes.to_vec()))
}
