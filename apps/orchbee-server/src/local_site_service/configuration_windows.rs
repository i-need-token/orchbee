//! Resolve the managed startup snapshot before any process or durable effect.
use orchbee_local_site_runtime::{
    WindowsSettingsError, WindowsUserSettings, normalize_windows_user_path,
};
use std::{ffi::OsString, io, path::PathBuf};
pub(super) fn resolve(
    mut arguments: impl Iterator<Item = OsString>,
    load: impl FnOnce() -> Result<WindowsUserSettings, WindowsSettingsError>,
) -> io::Result<WindowsUserSettings> {
    let Some(flag) = arguments.next() else {
        return load().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e));
    };
    if flag != "--resume-agents-on-restore" {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let resume_agents_on_restore = boolean(arguments.next())?;
    flag_value(&mut arguments, "--manifest-check")?;
    let manifest_check = boolean(arguments.next())?;
    flag_value(&mut arguments, "--manifest-check-source")?;
    let manifest_check_source = match arguments
        .next()
        .as_deref()
        .and_then(std::ffi::OsStr::to_str)
    {
        Some("default") => "default",
        Some("file") => "file",
        _ => return Err(io::ErrorKind::InvalidInput.into()),
    };
    if manifest_check_source == "default" && !manifest_check {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    flag_value(&mut arguments, "--configuration-file")?;
    let path = arguments.next().ok_or(io::ErrorKind::InvalidInput)?;
    let path = if path.is_empty() {
        None
    } else {
        let text = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
        Some(PathBuf::from(
            normalize_windows_user_path(text, |_| None, || Err(io::ErrorKind::InvalidInput.into()))
                .map_err(|_| io::ErrorKind::InvalidInput)?,
        ))
    };
    flag_value(&mut arguments, "--configuration-file-explicit")?;
    let explicit = boolean(arguments.next())?;
    let shell =
        orchbee_local_site_runtime::WindowsShellSettings::from_private_arguments(&mut arguments)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if arguments.next().is_some()
        || (path.is_none() && (explicit || manifest_check_source == "file"))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(WindowsUserSettings {
        shell,
        path,
        explicit,
        manifest_check,
        manifest_check_source,
        resume_agents_on_restore,
        ..WindowsUserSettings::default()
    })
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
