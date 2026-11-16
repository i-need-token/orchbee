//! Whether a shell's directory is the one a line expects, by this platform's own rule for naming a
//! directory. Only Agent session resume gives an expected directory.

/// Windows paths compare ASCII case-insensitively. One trailing separator is trimmed, except the
/// one that makes a drive root, since `C:` alone names the drive's current directory.
#[cfg(windows)]
pub(super) fn same_directory(left: &str, right: &str) -> bool {
    fn trimmed(path: &str) -> &str {
        match path.strip_suffix(['\\', '/']) {
            Some(rest) if !(rest.len() == 2 && rest.as_bytes()[1] == b':') => rest,
            _ => path,
        }
    }
    trimmed(left).eq_ignore_ascii_case(trimmed(right))
}

/// POSIX paths are equal byte for byte after one trailing `/` is trimmed, except the root's, or
/// name the same existing directory: the same device and inode, as zsh and bash decide whether
/// `$PWD` still names the current directory. A link, the `/private` spelling macOS gives `/tmp`
/// and `/var`, and another case on a case-insensitive volume therefore match, while a sibling that
/// differs only by case on a case-sensitive volume does not. A relative path never matches another
/// spelling.
#[cfg(unix)]
pub(super) fn same_directory(left: &str, right: &str) -> bool {
    use std::os::unix::fs::MetadataExt;
    fn trimmed(path: &str) -> &str {
        match path.strip_suffix('/') {
            Some(rest) if !rest.is_empty() => rest,
            _ => path,
        }
    }
    fn identity(path: &str) -> Option<(u64, u64)> {
        if !path.starts_with('/') {
            return None;
        }
        let metadata = std::fs::metadata(path).ok()?;
        metadata
            .is_dir()
            .then_some((metadata.dev(), metadata.ino()))
    }
    trimmed(left) == trimmed(right)
        || identity(left).is_some_and(|left| identity(right) == Some(left))
}
