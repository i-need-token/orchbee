//! Unix Git program and paths: `git` as process creation finds it on `PATH`, and absolute UTF-8
//! paths compared byte for byte.
use std::{borrow::Cow, ffi::OsString, path::Path};

/// The name process creation resolves on `PATH`.
pub const GIT_PROGRAM: &str = "git";

/// The Git a read that starts no helper program runs: the name, which process creation resolves.
pub fn read_program() -> OsString {
    GIT_PROGRAM.into()
}

/// Whether `path` is one a Git owner may resolve: any absolute path.
pub fn is_admissible_path(path: &Path) -> bool {
    path.is_absolute()
}

/// A resolved path is already in the system's spelling.
pub(crate) fn native_spelling(resolved: String) -> Option<String> {
    Some(resolved)
}

/// Git prints and accepts a Unix path exactly as the system spells it.
pub(crate) fn git_spelling(native: &str) -> Cow<'_, str> {
    Cow::Borrowed(native)
}

/// The spelling under which a checkout path Git recorded names the same checkout.
pub fn checkout_spelling(value: &str) -> String {
    value.trim_end_matches('/').to_owned()
}
