//! Native path spelling for people and machines. Text keeps the platform separators and escapes
//! controls; machine fields keep the exact native representation instead of a lossy string.

use std::path::Path;

use serde_json::Value;

#[cfg(windows)]
#[path = "native_path/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "native_path/unix.rs"]
mod platform;

/// A path as the platform spells it: on Windows, a drive-absolute or UNC path
/// reported with forward slashes uses backslashes. Controls stay escaped and
/// the result remains copyable.
pub(crate) fn display(value: &str) -> String {
    // `\\?\C:\…` is an API spelling; people read and paste `C:\…`.
    let value = match value.strip_prefix("\\\\?\\") {
        Some(rest) if matches!(rest.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic()) => {
            rest
        }
        _ => value,
    };
    platform::display(value)
}

/// A filesystem path as a machine field: UTF-8 paths are `path`; native
/// non-Unicode representations retain bytes or UTF-16 without lossy display.
pub(crate) fn detail(path: &Path) -> (String, Value) {
    platform::detail(path)
}

#[cfg(test)]
mod tests;
