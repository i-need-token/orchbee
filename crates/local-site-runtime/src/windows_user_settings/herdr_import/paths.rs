//! Ordinary Windows file operands exclude stream/device aliases before any file operation.
use super::{OsString, WindowsSettingsError};
use std::path::{Component, Path, PathBuf, Prefix};

/// Pure lexical admission for a literal file operand; never admits stdin or Windows aliases.
pub fn valid_windows_herdr_file_operand(value: &str) -> bool {
    if value.is_empty()
        || value == "-"
        || value.starts_with(' ')
        || value.len() > 4096
        || value.chars().any(char::is_control)
        || value.ends_with(['/', '\\'])
    {
        return false;
    }
    let path = Path::new(value);
    if let Some(Component::Prefix(prefix)) = path.components().next() {
        if !path.is_absolute() || !matches!(prefix.kind(), Prefix::Disk(_)) {
            return false;
        }
    } else if path.has_root() {
        return false;
    }
    let rest = if value.as_bytes().get(1) == Some(&b':') {
        &value[2..]
    } else {
        value
    };
    rest.split(['/', '\\'])
        .filter(|p| !p.is_empty())
        .all(|part| {
            if matches!(part, "." | "..")
                || part.ends_with(['.', ' '])
                || part.contains([':', '<', '>', '"', '|', '?', '*'])
            {
                return false;
            }
            let base = part
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            !matches!(
                base.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
            ) && !["COM", "LPT"].iter().any(|prefix| {
                base.strip_prefix(prefix).is_some_and(|suffix| {
                    matches!(
                        suffix,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            })
        })
        && path.file_name().is_some()
}

/// Resolve one explicit operand through the existing home/caller adapter, then revalidate its leaf.
///
/// # Errors
/// Refuses malformed operands, malformed required context and nonordinary resolved file paths.
pub fn normalize_windows_herdr_file_operand(
    value: &str,
    env: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<String, WindowsSettingsError> {
    if !valid_windows_herdr_file_operand(value) {
        return Err(WindowsSettingsError::new("operand", "invalid-path"));
    }
    let resolved = crate::normalize_windows_user_path(value, env, cwd)
        .map_err(|_| WindowsSettingsError::new("operand", "invalid-path"))?;
    if !valid_windows_herdr_file_operand(&resolved) {
        return Err(WindowsSettingsError::new("operand", "invalid-path"));
    }
    Ok(resolved)
}
