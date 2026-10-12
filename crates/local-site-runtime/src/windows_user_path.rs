//! Windows user path operands become absolute before runtime access, without probing filesystem paths.
pub const WINDOWS_USER_PATH_LIMIT: usize = 4096;
use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf, Prefix},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsUserPathError {
    Input,
    Context(&'static str),
}

/// Preserves literal absolute operands and resolves only explicitly declared caller-relative inputs.
pub fn normalize_windows_user_path(
    value: &str,
    mut environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<String, WindowsUserPathError> {
    if value.is_empty() || value.len() > WINDOWS_USER_PATH_LIMIT || value.contains('\0') {
        return Err(WindowsUserPathError::Input);
    }
    let path = Path::new(value);
    if drive_absolute(path) {
        return checked_value(value).map_err(|_| WindowsUserPathError::Input);
    }
    if path.has_root()
        || matches!(path.components().next(), Some(Component::Prefix(_)))
        || value.contains(':')
    {
        return Err(WindowsUserPathError::Input);
    }
    let rest = if value == "~" {
        Some("")
    } else {
        value
            .strip_prefix("~/")
            .or_else(|| value.strip_prefix(r"~\"))
    };
    let absolute = if let Some(rest) = rest {
        // Split separators so a second slash cannot replace the selected home root.
        let mut home = home(&mut environment)?;
        for component in rest.split(['/', '\\']).filter(|part| !part.is_empty()) {
            home.push(component);
        }
        home
    } else {
        let current = cwd().map_err(|_| WindowsUserPathError::Context("current-directory"))?;
        checked_context(&current, "current-directory")?;
        current.join(path)
    };
    if !drive_absolute(&absolute) {
        return Err(WindowsUserPathError::Input);
    }
    checked_value(absolute.to_str().ok_or(WindowsUserPathError::Input)?)
        .map_err(|_| WindowsUserPathError::Input)
}

fn drive_absolute(path: &Path) -> bool {
    path.is_absolute()
        && matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
}

fn checked_context(path: &Path, name: &'static str) -> Result<(), WindowsUserPathError> {
    let value = path.to_str().ok_or(WindowsUserPathError::Context(name))?;
    if !drive_absolute(path) || checked_value(value).is_err() {
        return Err(WindowsUserPathError::Context(name));
    }
    Ok(())
}

fn home(
    environment: &mut impl FnMut(&str) -> Option<OsString>,
) -> Result<PathBuf, WindowsUserPathError> {
    let nonempty = |value: Option<OsString>| value.filter(|value| !value.is_empty());
    if let Some(value) = nonempty(environment("USERPROFILE")) {
        let path = PathBuf::from(value);
        checked_context(&path, "USERPROFILE")?;
        return Ok(path);
    }
    let drive = nonempty(environment("HOMEDRIVE"));
    let path = nonempty(environment("HOMEPATH"));
    match (drive, path) {
        (Some(drive), Some(path)) => {
            let drive = drive
                .to_str()
                .ok_or(WindowsUserPathError::Context("HOMEDRIVE"))?;
            if !matches!(drive.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic()) {
                return Err(WindowsUserPathError::Context("HOMEDRIVE"));
            }
            let path = path
                .to_str()
                .ok_or(WindowsUserPathError::Context("HOMEPATH"))?;
            if !path.starts_with(['/', '\\']) || path.starts_with("//") || path.starts_with(r"\\") {
                return Err(WindowsUserPathError::Context("HOMEPATH"));
            }
            let home = PathBuf::from(format!("{drive}{path}"));
            checked_context(&home, "HOMEPATH")?;
            Ok(home)
        }
        (Some(_), None) => Err(WindowsUserPathError::Context("HOMEPATH")),
        (None, Some(_)) => Err(WindowsUserPathError::Context("HOMEDRIVE")),
        (None, None) => {
            let value = nonempty(environment("HOME")).ok_or(WindowsUserPathError::Context(
                "USERPROFILE/HOMEDRIVE/HOMEPATH/HOME",
            ))?;
            let home = PathBuf::from(value);
            checked_context(&home, "HOME")?;
            Ok(home)
        }
    }
}

fn checked_value(value: &str) -> Result<String, WindowsUserPathError> {
    if value.is_empty()
        || value.len() > WINDOWS_USER_PATH_LIMIT
        || value.contains('\0')
        || !drive_absolute(Path::new(value))
    {
        return Err(WindowsUserPathError::Input);
    }
    Ok(value.to_owned())
}
