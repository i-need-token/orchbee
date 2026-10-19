//! Windows path syntax is admitted without process context; the execution owner resolves relative operands without filesystem probes.
use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf, Prefix},
};

/// Maximum UTF-8 bytes in an operand or its resolved absolute path.
pub const WINDOWS_USER_PATH_LIMIT: usize = 4096;

/// Separates invalid operand syntax from the named process context that could not resolve it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsUserPathError {
    Input,
    Context(&'static str),
}

/// A syntactically valid operand that retains its exact spelling until its execution owner resolves it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsUserPathOperand {
    value: String,
    kind: OperandKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperandKind {
    Absolute,
    Home { suffix_start: usize },
    CurrentDirectory,
}

impl WindowsUserPathOperand {
    /// Admits bounded Windows path syntax without reading the environment, cwd or filesystem.
    ///
    /// # Errors
    /// Returns `Input` for empty, oversized, NUL-containing or ambiguous-root operands.
    pub fn parse(value: &str) -> Result<Self, WindowsUserPathError> {
        if value.is_empty() || value.len() > WINDOWS_USER_PATH_LIMIT || value.contains('\0') {
            return Err(WindowsUserPathError::Input);
        }
        let path = Path::new(value);
        let kind = if drive_absolute(path) {
            OperandKind::Absolute
        } else {
            if path.has_root()
                || matches!(path.components().next(), Some(Component::Prefix(_)))
                || value.contains(':')
            {
                return Err(WindowsUserPathError::Input);
            }
            if value == "~" {
                OperandKind::Home { suffix_start: 1 }
            } else if value.starts_with("~/") || value.starts_with(r"~\") {
                OperandKind::Home { suffix_start: 2 }
            } else {
                OperandKind::CurrentDirectory
            }
        };
        Ok(Self {
            value: value.to_owned(),
            kind,
        })
    }

    /// Returns the original operand, without expansion or canonicalization.
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Preserves absolute literals and resolves home or cwd only through the supplied execution context.
    /// A resolved home or cwd operand is normalized lexically, as Windows itself does: `.` is
    /// dropped, `..` removes one earlier component but never the root, and no trailing separator
    /// remains.
    ///
    /// # Errors
    /// Returns `Context` for a missing or malformed required process value, or `Input` when
    /// expansion exceeds the path bound or cannot produce a Unicode drive-absolute path.
    pub fn resolve(
        &self,
        mut environment: impl FnMut(&str) -> Option<OsString>,
        cwd: impl FnOnce() -> std::io::Result<PathBuf>,
    ) -> Result<String, WindowsUserPathError> {
        let absolute = match self.kind {
            OperandKind::Absolute => return Ok(self.value.clone()),
            OperandKind::Home { suffix_start } => {
                // Split separators so a second slash cannot replace the selected home root.
                let mut home = home(&mut environment)?;
                for component in self.value[suffix_start..]
                    .split(['/', '\\'])
                    .filter(|part| !part.is_empty())
                {
                    home.push(component);
                }
                home
            }
            OperandKind::CurrentDirectory => {
                let current =
                    cwd().map_err(|_| WindowsUserPathError::Context("current-directory"))?;
                checked_context(&current, "current-directory")?;
                current.join(&self.value)
            }
        };
        checked_value(
            lexical(&absolute)
                .to_str()
                .ok_or(WindowsUserPathError::Input)?,
        )
    }
}

/// Parses and resolves an operand when the caller already owns its execution context.
///
/// # Errors
/// Returns the operand's `Input` error or the required execution `Context` error.
pub fn normalize_windows_user_path(
    value: &str,
    environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<String, WindowsUserPathError> {
    WindowsUserPathOperand::parse(value)?.resolve(environment, cwd)
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

/// Resolve `.` and `..` without touching the filesystem; the prefix and root
/// are kept, so `..` can never climb above the drive root.
fn lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut depth = 0_usize;
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth > 0 {
                    normalized.pop();
                    depth -= 1;
                }
            }
            Component::Normal(part) => {
                normalized.push(part);
                depth += 1;
            }
        }
    }
    normalized
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

#[cfg(test)]
mod tests;
