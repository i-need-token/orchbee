//! The binding record in the Session folder: one opaque document the Server publishes while it holds
//! the store, read back by the Server that holds it next and, without the store, by a process that
//! decides whether a running Host can be adopted. The Server owns its meaning.
use super::{SessionStore, publish_private, remove_if_present};
use crate::{DataRoot, ModeCheckedLocalSessionRendezvous, PrivateCache, private_filesystem};
use std::io::{self, Read};

/// The binding record's file in the Session folder, beside its staging file.
pub(crate) const FILE: &str = "bindings.json";
pub(crate) const PENDING: &str = "bindings.pending";
/// The most bytes one binding record holds.
pub const MAXIMUM_BINDING_RECORD_BYTES: usize = 1024 * 1024;

impl SessionStore {
    /// Publishes `bytes` as the binding record through a flushed staging file and an atomic
    /// replace; only the store's holder publishes it. `validate`, the Server's own reader, must
    /// admit the bytes first. A failure before the replace leaves the earlier record unchanged.
    pub fn publish_bindings(
        &mut self,
        bytes: &[u8],
        validate: impl FnOnce(&[u8]) -> bool,
    ) -> io::Result<()> {
        if bytes.len() > MAXIMUM_BINDING_RECORD_BYTES || !validate(bytes) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        publish_private(
            &self.directory.join(PENDING),
            &self.directory.join(FILE),
            bytes,
            "binding record",
        )
    }

    /// The binding record, or `None` when the Session folder holds none. A file larger than the
    /// bound is refused with `FileTooLarge`.
    pub fn read_bindings(&self) -> io::Result<Option<Vec<u8>>> {
        let mut file = match private_filesystem::open_snapshot_file(&self.directory.join(FILE)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAXIMUM_BINDING_RECORD_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAXIMUM_BINDING_RECORD_BYTES {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        Ok(Some(bytes))
    }

    /// Removes the binding record; an absent one counts as removed.
    pub fn remove_bindings(&mut self) -> io::Result<()> {
        remove_if_present(&self.directory.join(FILE))
    }
}

/// The binding record of the Session whose runtime `session` admitted, read without its store: it
/// creates nothing and takes no lock, and since the holder only ever replaces the file whole, it
/// reads one complete record, the earlier or the later one. A file larger than the bound is
/// refused with `FileTooLarge`.
pub fn read_session_bindings(
    root: &DataRoot,
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<Option<Vec<u8>>> {
    let name = session.session_name()?;
    PrivateCache::read_existing(
        &root.sessions().join(name.as_str()),
        FILE,
        MAXIMUM_BINDING_RECORD_BYTES,
    )
}
