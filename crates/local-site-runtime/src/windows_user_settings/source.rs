//! Configuration-file capability admission; parsing and normalized profiles remain shared.
use super::WindowsSettingsError;
use std::{fs::File, io, path::Path};

pub(super) fn require() -> Result<(), WindowsSettingsError> {
    if cfg!(windows) {
        Ok(())
    } else {
        Err(WindowsSettingsError::new(
            "platform",
            "unsupported-platform",
        ))
    }
}

pub(super) fn open(path: &Path) -> io::Result<File> {
    require().map_err(|error| io::Error::new(io::ErrorKind::Unsupported, error))?;
    File::open(path)
}
