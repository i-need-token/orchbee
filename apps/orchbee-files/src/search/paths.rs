//! The path helper VS Code's search code applies to paths relative to a searched folder, over
//! UTF-16 code units: `isEqualOrParent` (`base/common/extpath.ts`) with the separator a caller
//! names.

use super::js;

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
