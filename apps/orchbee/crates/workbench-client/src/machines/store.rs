//! The only writer of the private saved-machine catalog.
use super::{Catalog, Edit, Error};
use orchbee_local_runtime::{DataRoot, OpenedRecord, VersionedFile, VersionedRecord};
use std::{io, sync::Mutex};

/// The catalog this process keeps while the record's version is one it cannot use: such a record
/// is never rewritten, so the store starts empty and keeps changes here until the process ends.
static MEMORY: Mutex<Option<Catalog>> = Mutex::new(None);

const MAX_BYTES: usize = 128 * 1024;

pub struct Store {
    record: VersionedFile,
}
impl Store {
    pub fn configured() -> Result<Self, Error> {
        let root =
            DataRoot::resolve(|name| std::env::var_os(name)).map_err(|_| Error::Configuration)?;
        Ok(Self::in_data_root(root))
    }
    fn in_data_root(root: DataRoot) -> Self {
        Self {
            record: VersionedFile::device(root, VersionedRecord::Machines),
        }
    }
    /// A read keeps a migrated earlier record in memory; only a commit persists it.
    pub fn read(&self) -> Result<Catalog, Error> {
        match decode(self.record.read(MAX_BYTES, admits).map_err(storage)?)? {
            Stored::Current(catalog) => Ok(catalog),
            Stored::Unusable => Ok(remembered()),
        }
    }
    /// Re-read while holding the writer lock, after remote preparation has ended. Opening under
    /// the lock backs up and persists an earlier record before this edit replaces it.
    pub fn commit(&self, edit: &Edit) -> Result<(Catalog, bool), Error> {
        let (mut writer, opened) = self.record.acquire(MAX_BYTES, admits).map_err(storage)?;
        let current = match decode(opened.map_err(storage)?)? {
            Stored::Current(catalog) => catalog,
            Stored::Unusable => {
                let (next, changed) = remembered().edit(edit)?;
                if let Ok(mut memory) = MEMORY.lock() {
                    *memory = Some(next.clone());
                }
                return Ok((next, changed));
            }
        };
        let (next, changed) = current.edit(edit)?;
        if changed {
            let bytes = serde_json::to_vec_pretty(&next.json()).map_err(|_| Error::Storage)?;
            if bytes.len() > MAX_BYTES {
                return Err(Error::Capacity);
            }
            writer.replace(&bytes).map_err(storage)?;
        }
        Ok((next, changed))
    }
}
enum Stored {
    Current(Catalog),
    /// The record's version is one this build cannot use.
    Unusable,
}
fn admits(bytes: &[u8]) -> bool {
    Catalog::parse(bytes).is_some()
}
fn decode(opened: OpenedRecord) -> Result<Stored, Error> {
    match opened {
        OpenedRecord::Absent => Ok(Stored::Current(Catalog::default())),
        OpenedRecord::Unusable(_) => Ok(Stored::Unusable),
        OpenedRecord::Readable(bytes) => Catalog::parse(&bytes)
            .map(Stored::Current)
            .ok_or(Error::InvalidCatalog),
    }
}
fn remembered() -> Catalog {
    MEMORY
        .lock()
        .ok()
        .and_then(|memory| memory.clone())
        .unwrap_or_default()
}
fn storage(error: io::Error) -> Error {
    if error.kind() == io::ErrorKind::WouldBlock {
        Error::Busy
    } else {
        Error::Storage
    }
}
#[cfg(all(test, windows))]
mod tests;
