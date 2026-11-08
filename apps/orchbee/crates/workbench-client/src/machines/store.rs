//! The only writer of the private saved-machine catalog.
use super::{Catalog, Edit, Error};
use orchbee_local_runtime::{DataRoot, PrivateCache, RecordNotice, StateRecord, VersionedRecord};
use std::{io, path::PathBuf, sync::Mutex};

/// The catalog this process keeps while the record's version is one it cannot use: such a record
/// is never rewritten, so the store starts empty and keeps changes here until the process ends.
static MEMORY: Mutex<Option<Catalog>> = Mutex::new(None);

const RECORD: StateRecord = StateRecord::Machines;
const FILE: &str = RECORD.file_name();
const MAX_BYTES: usize = 128 * 1024;

pub struct Store {
    root: PathBuf,
}
impl Store {
    pub fn configured() -> Result<Self, Error> {
        let root =
            DataRoot::resolve(|name| std::env::var_os(name)).map_err(|_| Error::Configuration)?;
        Ok(Self { root: root.state() })
    }
    pub fn read(&self) -> Result<Catalog, Error> {
        match self
            .decode(PrivateCache::read_existing(&self.root, FILE, MAX_BYTES).map_err(storage)?)?
        {
            Stored::Current(catalog) => Ok(catalog),
            Stored::Unusable => Ok(remembered()),
        }
    }
    /// Re-read while holding the writer lock, after remote preparation has ended.
    pub fn commit(&self, edit: &Edit) -> Result<(Catalog, bool), Error> {
        let mut cache = PrivateCache::acquire(&self.root, RECORD.name()).map_err(storage)?;
        let current = match self.decode(cache.read(FILE, MAX_BYTES).map_err(storage)?)? {
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
            cache.replace(FILE, &bytes).map_err(storage)?;
        }
        Ok((next, changed))
    }
}
enum Stored {
    Current(Catalog),
    /// The record's version is one this build cannot use.
    Unusable,
}
impl Store {
    fn decode(&self, bytes: Option<Vec<u8>>) -> Result<Stored, Error> {
        let Some(bytes) = bytes else {
            return Ok(Stored::Current(Catalog::default()));
        };
        let opening =
            orchbee_local_runtime::open_record(VersionedRecord::Machines, &bytes, |bytes| {
                Catalog::parse(bytes).is_some()
            });
        if let Some((problem, version)) = opening.problem() {
            orchbee_local_runtime::report_record_notice(&RecordNotice {
                record: VersionedRecord::Machines,
                problem,
                version,
                path: self.root.join(FILE),
            });
            return Ok(Stored::Unusable);
        }
        let bytes = match opening {
            orchbee_local_runtime::RecordOpening::Migrated { bytes, .. } => bytes,
            _ => bytes,
        };
        Catalog::parse(&bytes)
            .map(Stored::Current)
            .ok_or(Error::InvalidCatalog)
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
