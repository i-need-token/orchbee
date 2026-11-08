//! The only writer of the private saved-machine catalog.
use super::{Catalog, Edit, Error};
use orchbee_local_runtime::{DataRoot, PrivateCache, StateRecord};
use std::{io, path::PathBuf};

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
        decode(PrivateCache::read_existing(&self.root, FILE, MAX_BYTES).map_err(storage)?)
    }
    /// Re-read while holding the writer lock, after remote preparation has ended.
    pub fn commit(&self, edit: &Edit) -> Result<(Catalog, bool), Error> {
        let mut cache = PrivateCache::acquire(&self.root, RECORD.name()).map_err(storage)?;
        let current = decode(cache.read(FILE, MAX_BYTES).map_err(storage)?)?;
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
fn decode(bytes: Option<Vec<u8>>) -> Result<Catalog, Error> {
    match bytes {
        None => Ok(Catalog::default()),
        Some(bytes) => Catalog::parse(&bytes).ok_or(Error::InvalidCatalog),
    }
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
