//! Read the managed startup configuration once, before any process or durable effect. The Client
//! freezes only the location; the read is the Server's authoritative baseline.
use orchbee_local_runtime::{SettingsError, UserSettingsLoad};
use std::{ffi::OsString, io, path::Path, path::PathBuf};

/// Where the managed start reads its configuration.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Location {
    /// A direct start: the location the environment names.
    Environment,
    /// The location the Client froze; `None` when the environment names no file.
    Frozen {
        path: Option<PathBuf>,
        explicit: bool,
    },
}

impl Location {
    /// Reads the configuration at this location, through the shared settings owner.
    pub(super) fn read(
        self,
        load: impl FnOnce() -> Result<UserSettingsLoad, SettingsError>,
        load_at: impl FnOnce(Option<&Path>, bool) -> Result<UserSettingsLoad, SettingsError>,
    ) -> Result<UserSettingsLoad, SettingsError> {
        match self {
            Self::Environment => load(),
            Self::Frozen { path, explicit } => load_at(path.as_deref(), explicit),
        }
    }
}

/// Parses the closed private arguments after the runtime directory: none, or the frozen location
/// as `--configuration-file <path or empty> --configuration-file-explicit <true|false>`.
pub(super) fn parse(mut arguments: impl Iterator<Item = OsString>) -> io::Result<Location> {
    let Some(flag) = arguments.next() else {
        return Ok(Location::Environment);
    };
    if flag != "--configuration-file" {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let path = arguments.next().ok_or(io::ErrorKind::InvalidInput)?;
    let path = if path.is_empty() {
        None
    } else {
        let text = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
        Some(admitted(text).ok_or(io::ErrorKind::InvalidInput)?)
    };
    flag_value(&mut arguments, "--configuration-file-explicit")?;
    let explicit = boolean(arguments.next())?;
    if arguments.next().is_some() || (path.is_none() && explicit) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(Location::Frozen { path, explicit })
}

/// A frozen Windows location follows the user-path grammar the settings file always used.
#[cfg(windows)]
fn admitted(text: &str) -> Option<PathBuf> {
    orchbee_local_runtime::normalize_user_path(
        text,
        |_| None,
        || Err(io::ErrorKind::InvalidInput.into()),
    )
    .ok()
    .map(PathBuf::from)
}

/// A frozen POSIX location is a plain absolute path without `.` or `..` parts, the rule
/// `ORCHBEE_CONFIG_FILE` follows.
#[cfg(not(windows))]
fn admitted(text: &str) -> Option<PathBuf> {
    let path = PathBuf::from(text);
    let plain = path.is_absolute()
        && path.components().all(|part| {
            matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        });
    plain.then_some(path)
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
