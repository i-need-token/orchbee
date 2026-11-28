//! The path helpers VS Code's search code applies to paths relative to a searched folder, over
//! UTF-16 code units: `isEqualOrParent` (`base/common/extpath.ts`) with the separator a caller
//! names, and Node's `basename` and `dirname` on this platform, which on Windows split at either
//! slash.

use super::js::{self, BACKSLASH, SLASH};

/// `isEqualOrParent`: `base` is `parent` or lies below it, segments split at `separator`. The
/// paths it compares never hold `..` segments, which VS Code would normalize first.
pub(super) fn is_equal_or_parent(
    base: &[u16],
    parent: &[u16],
    ignore_case: bool,
    separator: u16,
) -> bool {
    if base == parent {
        return true;
    }
    if base.is_empty() || parent.is_empty() || parent.len() > base.len() {
        return false;
    }
    if ignore_case {
        if !js::starts_with_ignore_case(base, parent) {
            return false;
        }
        if parent.len() == base.len() {
            return true;
        }
        let mut offset = parent.len();
        if parent.last() == Some(&separator) {
            offset -= 1;
        }
        return base.get(offset) == Some(&separator);
    }
    if parent.last() == Some(&separator) {
        base.starts_with(parent)
    } else {
        base.starts_with(parent) && base.get(parent.len()) == Some(&separator)
    }
}

/// Whether `unit` separates segments for Node's path module on this platform.
fn is_separator(unit: u16) -> bool {
    unit == SLASH || (cfg!(windows) && unit == BACKSLASH)
}

/// Where `basename` of `path`, which has no trailing separator, starts.
pub(super) fn basename_start(path: &[u16]) -> usize {
    path.iter()
        .rposition(|unit| is_separator(*unit))
        .map_or(0, |separator| separator + 1)
}

/// The `.` `dirname` returns for a path without a separator.
const DOT: [u16; 1] = [0x2E];

/// `dirname` of `path`, which has no trailing separator: everything before its last separator,
/// or `.` at the top.
pub(super) fn dirname(path: &[u16]) -> &[u16] {
    match path.iter().rposition(|unit| is_separator(*unit)) {
        // A leading separator is the root, which `dirname` keeps.
        Some(0) => &path[..1],
        Some(separator) => &path[..separator],
        None => &DOT,
    }
}
