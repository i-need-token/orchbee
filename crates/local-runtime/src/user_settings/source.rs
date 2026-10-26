//! Configuration-file capability admission; parsing and normalized profiles remain shared.
use super::SettingsError;
use std::{fs::File, io, path::Path};

#[cfg(windows)]
mod windows;

pub(super) fn require() -> Result<(), SettingsError> {
    if cfg!(windows) {
        Ok(())
    } else {
        Err(SettingsError::new("platform", "unsupported-platform"))
    }
}

pub(super) fn open(path: &Path) -> io::Result<File> {
    require().map_err(|error| io::Error::new(io::ErrorKind::Unsupported, error))?;
    #[cfg(windows)]
    return windows::open(path);
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(io::ErrorKind::Unsupported.into())
    }
}
