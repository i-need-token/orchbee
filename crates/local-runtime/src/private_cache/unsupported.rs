//! Unsupported private Windows-cache effects on Unix; callers retain their state and content.
use std::{io, path::Path};

/// The private cache capability has no Unix implementation.
pub struct PrivateCache;

impl PrivateCache {
    /// Refuses acquisition before creating a directory, lock, or file.
    pub fn acquire(_path: &Path) -> io::Result<Self> {
        Err(io::ErrorKind::Unsupported.into())
    }

    /// Refuses a read without opening the supplied path.
    pub fn read(&self, _name: &str, _maximum: usize) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::Unsupported.into())
    }

    /// Refuses a read without opening the supplied path.
    pub fn read_existing(
        _path: &Path,
        _name: &str,
        _maximum: usize,
    ) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::Unsupported.into())
    }

    /// Refuses publication without writing or replacing any file.
    pub fn replace(&mut self, _name: &str, _bytes: &[u8]) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    /// Refuses removal without deleting any file.
    pub fn remove(&mut self, _name: &str) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

/// No value can be produced without an admitted platform read.
pub(crate) struct PrivateRead {
    bytes: Option<Vec<u8>>,
}
impl PrivateRead {
    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}
impl PrivateCache {
    pub(crate) fn read_existing_pinned(
        _path: &Path,
        _name: &str,
        _maximum: usize,
    ) -> io::Result<PrivateRead> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
