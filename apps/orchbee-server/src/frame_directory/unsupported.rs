//! Builds without pinned private frame reads hold no frame root: reservation refuses with
//! `Unsupported`, so the remaining operations are unreachable.
use std::{io, path::Path};

/// No value exists; [`PrivateFrameDirectory::reserve`] is the only way to obtain one.
pub(crate) enum PrivateFrameDirectory {}

impl PrivateFrameDirectory {
    /// Refuses before touching the runtime directory.
    pub(crate) fn reserve(_runtime: &Path, _nonce: &str) -> io::Result<Self> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub(crate) fn create(&mut self, _owner_record: &[u8]) -> io::Result<()> {
        match *self {}
    }
    pub(crate) fn path(&self) -> &Path {
        match *self {}
    }
    pub(crate) fn read_frame(&self, _path: &Path, _expected_bytes: usize) -> io::Result<Vec<u8>> {
        match *self {}
    }
    pub(crate) fn reap(&mut self) -> io::Result<()> {
        match *self {}
    }
}
