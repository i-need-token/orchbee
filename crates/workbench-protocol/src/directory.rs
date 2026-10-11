use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

pub const MAXIMUM_DIRECTORY_PATH_BYTES: usize = 4096;

/// Compares admitted Windows path spellings using the existing Worktree association rule.
/// Only separators, trailing separators and ASCII case are equivalent. This performs no
/// filesystem identity check, canonicalization, verbatim-prefix or dot-component resolution.
#[must_use]
pub fn same_windows_path_spelling(left: &str, right: &str) -> bool {
    left.replace('\\', "/")
        .trim_end_matches('/')
        .eq_ignore_ascii_case(right.replace('\\', "/").trim_end_matches('/'))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchDirectoryPathError;

impl fmt::Display for WorkbenchDirectoryPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "directory path must be an absolute local-platform UTF-8 path of 1 through 4096 bytes without NUL",
        )
    }
}

impl std::error::Error for WorkbenchDirectoryPathError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkbenchDirectoryPath(String);

impl WorkbenchDirectoryPath {
    pub fn parse(value: &str) -> Result<Self, WorkbenchDirectoryPathError> {
        if !is_absolute_path(value.as_bytes())
            || value.len() > MAXIMUM_DIRECTORY_PATH_BYTES
            || value.contains('\0')
        {
            return Err(WorkbenchDirectoryPathError);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for WorkbenchDirectoryPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for WorkbenchDirectoryPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

/// Platform-independent wire syntax; the execution owner validates local meaning.
/// Windows admits drive-absolute paths, including verbatim drive paths. No UNC,
/// device namespace, drive-relative, or current-drive-rooted backslash spelling.
pub(crate) fn is_absolute_path(value: &[u8]) -> bool {
    if value.first() == Some(&b'/') {
        return true;
    }
    let drive = value.strip_prefix(br"\\?\").unwrap_or(value);
    matches!(drive, [letter, b':', b'\\' | b'/', ..] if letter.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests;
