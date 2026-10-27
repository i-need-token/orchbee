//! Read the managed startup configuration once, before any process or durable effect.
use orchbee_local_runtime::{SettingsError, UserSettingsLoad, normalize_user_path};
use std::{ffi::OsString, io, path::Path, path::PathBuf};
/// The Client freezes only the location; this read is the Server's authoritative baseline.
pub(super) fn resolve(
    mut arguments: impl Iterator<Item = OsString>,
    load: impl FnOnce() -> Result<UserSettingsLoad, SettingsError>,
    load_at: impl FnOnce(Option<&Path>, bool) -> Result<UserSettingsLoad, SettingsError>,
) -> io::Result<UserSettingsLoad> {
    let invalid = |error: SettingsError| io::Error::new(io::ErrorKind::InvalidInput, error);
    let Some(flag) = arguments.next() else {
        return load().map_err(invalid);
    };
    if flag != "--configuration-file" {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let path = arguments.next().ok_or(io::ErrorKind::InvalidInput)?;
    let path = if path.is_empty() {
        None
    } else {
        let text = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
        Some(PathBuf::from(
            normalize_user_path(text, |_| None, || Err(io::ErrorKind::InvalidInput.into()))
                .map_err(|_| io::ErrorKind::InvalidInput)?,
        ))
    };
    flag_value(&mut arguments, "--configuration-file-explicit")?;
    let explicit = boolean(arguments.next())?;
    if arguments.next().is_some() || (path.is_none() && explicit) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    load_at(path.as_deref(), explicit).map_err(invalid)
}
fn flag_value(arguments: &mut impl Iterator<Item = OsString>, expected: &str) -> io::Result<()> {
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new(expected)) {
        Ok(())
    } else {
        Err(io::ErrorKind::InvalidInput.into())
    }
}
fn boolean(value: Option<OsString>) -> io::Result<bool> {
    match value.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err(io::ErrorKind::InvalidInput.into()),
    }
}
#[cfg(test)]
mod tests;
