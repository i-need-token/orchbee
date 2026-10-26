//! Windows path syntax is admitted without process context; the execution owner resolves relative operands without filesystem probes.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
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
        let kind = if drive_root(value).is_some() {
            OperandKind::Absolute
        } else {
            if value.starts_with(['/', '\\']) || value.contains(':') {
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
                let home = home(&mut environment)?;
                let mut absolute = home.to_str().ok_or(WindowsUserPathError::Input)?.to_owned();
                // Each suffix component is appended separately, matching the Windows execution
                // owner's home expansion even when the selected home has a verbatim prefix.
                for component in self.value[suffix_start..]
                    .split(['/', '\\'])
                    .filter(|part| !part.is_empty())
                {
                    absolute = join_context(&absolute, component)?;
                }
                absolute
            }
            OperandKind::CurrentDirectory => {
                let current =
                    cwd().map_err(|_| WindowsUserPathError::Context("current-directory"))?;
                checked_context(&current, "current-directory")?;
                join_context(
                    current.to_str().ok_or(WindowsUserPathError::Input)?,
                    &self.value,
                )?
            }
        };
        checked_value(&lexical(&absolute)?)
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

// Parse the wire grammar, never the host's `std::path` grammar. Unix clients must
// retain exactly the same Windows operand admission and normalization semantics.
fn drive_root(value: &str) -> Option<usize> {
    let verbatim = value.starts_with(r"\\?\");
    let offset = if verbatim { 4 } else { 0 };
    let bytes = value.as_bytes().get(offset..)?;
    match bytes {
        [letter, b':', b'/' | b'\\', ..] if letter.is_ascii_alphabetic() => Some(offset + 3),
        // Windows gives a verbatim disk prefix an implicit root, including the bare prefix.
        [letter, b':'] if verbatim && letter.is_ascii_alphabetic() => Some(offset + 2),
        _ => None,
    }
}

// A verbatim context treats '/' inside an existing component literally. Windows joins the
// ordinary relative operand before lexical normalization, so '..' removes that whole component.
fn join_context(context: &str, relative: &str) -> Result<String, WindowsUserPathError> {
    drive_root(context).ok_or(WindowsUserPathError::Input)?;
    Ok(push_path(context, relative))
}

fn disk_prefix(value: &str) -> bool {
    matches!(value.as_bytes(), [letter, b':', ..] if letter.is_ascii_alphabetic())
}

fn network_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !matches!(bytes, [b'/' | b'\\', b'/' | b'\\', ..]) {
        return false;
    }
    let mut parts = value[2..].split(['/', '\\']);
    parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_some_and(|part| !part.is_empty())
}

// The admitted grammar needs the Windows lexical push operation, not host PathBuf::push.
// Keep its two stages distinct: a verbatim base supplies literal components, while an
// appended operand supplies ordinary separators and parent traversal.
fn push_path(context: &str, relative: &str) -> String {
    if disk_prefix(relative) || network_prefix(relative) {
        return relative.to_owned();
    }
    let rooted = relative.starts_with(['/', '\\']);
    if context.starts_with(r"\\?\") && !relative.is_empty() {
        let root = drive_root(context).expect("admitted verbatim disk context");
        let mut parts: Vec<_> = if rooted {
            Vec::new()
        } else {
            context[root..]
                .split('\\')
                .filter(|part| !part.is_empty())
                .collect()
        };
        for part in relative.split(['/', '\\']) {
            match part {
                "" | "." => {}
                ".." => {
                    if parts
                        .last()
                        .is_some_and(|part| !matches!(*part, "." | ".."))
                    {
                        parts.pop();
                    }
                }
                part => parts.push(part),
            }
        }
        let mut result = context[..6].to_owned();
        if rooted || root > 6 || !parts.is_empty() {
            result.push('\\');
        }
        result.push_str(&parts.join("\\"));
        return result;
    }
    if rooted {
        let prefix = if disk_prefix(context) {
            &context[..2]
        } else {
            ""
        };
        return format!("{prefix}{relative}");
    }
    let mut result = context.to_owned();
    if !context.is_empty()
        && !context.ends_with(['/', '\\'])
        && !(context.len() == 2 && disk_prefix(context))
    {
        result.push('\\');
    }
    result.push_str(relative);
    result
}

fn pop_path(value: &mut String) {
    let verbatim = value.starts_with(r"\\?\");
    let prefix = if verbatim {
        6
    } else if disk_prefix(value) {
        2
    } else {
        0
    };
    let separator = |ch| ch == '\\' || (!verbatim && ch == '/');
    let trimmed = value.trim_end_matches(separator);
    if trimmed.len() <= prefix {
        return;
    }
    let end = trimmed.rfind(separator).filter(|end| *end >= prefix);
    value.truncate(match end {
        Some(end) if end == prefix => end + 1,
        Some(end) => end,
        None => prefix,
    });
}

fn checked_context(path: &Path, name: &'static str) -> Result<(), WindowsUserPathError> {
    let value = path.to_str().ok_or(WindowsUserPathError::Context(name))?;
    if drive_root(value).is_none() || checked_value(value).is_err() {
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
fn lexical(value: &str) -> Result<String, WindowsUserPathError> {
    let root = drive_root(value).ok_or(WindowsUserPathError::Input)?;
    let verbatim = value.starts_with(r"\\?\");
    let prefix = if verbatim { 6 } else { 2 };
    let mut normalized = value[..prefix].to_owned();
    if root > prefix {
        normalized.push('\\');
    }
    let mut depth = 0_usize;
    for component in value[root..].split(|ch| ch == '\\' || (!verbatim && ch == '/')) {
        match component {
            "" | "." => {}
            ".." => {
                if depth > 0 {
                    pop_path(&mut normalized);
                    depth -= 1;
                }
            }
            part => {
                // Reconstruct one component at a time, preserving Windows' separate join
                // and normalization stages even for verbatim contexts containing '/'.
                normalized = push_path(&normalized, part);
                depth += 1;
            }
        }
    }
    Ok(normalized)
}

fn checked_value(value: &str) -> Result<String, WindowsUserPathError> {
    if value.is_empty()
        || value.len() > WINDOWS_USER_PATH_LIMIT
        || value.contains('\0')
        || drive_root(value).is_none()
    {
        return Err(WindowsUserPathError::Input);
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests;
