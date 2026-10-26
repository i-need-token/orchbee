//! The only writer of the private saved-machine catalog.
use super::{Catalog, Edit, Error};
use orchbee_local_runtime::{PrivateCache, user_settings_path};
use std::{io, path::PathBuf};

const FILE: &str = "catalog.json";
const MAX_BYTES: usize = 128 * 1024;

pub(crate) struct Store {
    root: PathBuf,
}
impl Store {
    pub(crate) fn configured() -> Result<Self, Error> {
        let path = user_settings_path(|name| std::env::var_os(name))
            .map_err(|_| Error::Configuration)?
            .ok_or(Error::Configuration)?;
        let parent = path.parent().ok_or(Error::Configuration)?;
        Ok(Self {
            root: parent.join("machines"),
        })
    }
    pub(crate) fn read(&self) -> Result<Catalog, Error> {
        decode(PrivateCache::read_existing(&self.root, FILE, MAX_BYTES).map_err(storage)?)
    }
    /// Re-read while holding the writer lock, after remote preparation has ended.
    pub(crate) fn commit(&self, edit: &Edit) -> Result<(Catalog, bool), Error> {
        let mut cache = PrivateCache::acquire(&self.root).map_err(storage)?;
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
