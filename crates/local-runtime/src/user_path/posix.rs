//! POSIX path syntax is admitted and resolved from caller-supplied context alone: an absolute path
//! as written, `~` or `~/...` under `HOME`, or a path relative to the supplied current directory,
//! resolved lexically. It never reads the process environment, cwd or filesystem.
use super::windows::{USER_PATH_LIMIT, UserPathError};
use std::{ffi::OsString, path::PathBuf};

/// Resolves `value` in the POSIX grammar. A resolved home or relative operand is normalized
/// lexically, as a shell's logical `cd` does: empty and `.` components are dropped, `..` removes
/// one earlier component but never the root, and no trailing separator remains. An absolute
/// operand keeps its spelling, as the Windows grammar keeps an absolute literal. `~name` is an
/// ordinary relative name, and a Windows drive or `\` spelling is not part of the grammar.
///
/// # Errors
/// Returns `Input` for an empty, oversized, NUL-containing or Windows-form operand or an oversized
/// result, and `Context` when `HOME` or the current directory is missing, not absolute or not
/// UTF-8.
pub fn normalize_posix_user_path(
    value: &str,
    mut environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<String, UserPathError> {
    let windows_form = value.starts_with('\\')
        || matches!(value.as_bytes(), [letter, b':', ..] if letter.is_ascii_alphabetic());
    if value.is_empty() || value.len() > USER_PATH_LIMIT || value.contains('\0') || windows_form {
        return Err(UserPathError::Input);
    }
    if value.starts_with('/') {
        return Ok(value.to_owned());
    }
    let (base, relative) = match value.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => (
            context(environment("HOME").filter(|home| !home.is_empty()), "HOME")?,
            rest,
        ),
        _ => (
            context(cwd().ok().map(PathBuf::into_os_string), "current-directory")?,
            value,
        ),
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split('/').chain(relative.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    let resolved = format!("/{}", parts.join("/"));
    if resolved.len() > USER_PATH_LIMIT {
        return Err(UserPathError::Input);
    }
    Ok(resolved)
}

/// An absolute UTF-8 context value, or the named context error.
fn context(value: Option<OsString>, name: &'static str) -> Result<String, UserPathError> {
    value
        .and_then(|value| value.into_string().ok())
        .filter(|value| value.starts_with('/'))
        .ok_or(UserPathError::Context(name))
}

#[cfg(test)]
mod tests;
