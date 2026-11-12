//! Windows repository paths: absolute drive paths only, compared without case or separator style.
use orchbee_capability_node_protocol::AbsoluteCwd;
use std::path::Path;

pub(crate) const GIT_PROGRAM: &str = "git.exe";

pub(crate) fn is_admissible_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    path.is_absolute()
        && matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
}

pub(crate) fn canonical_path(path: &Path) -> Result<AbsoluteCwd, ()> {
    // Filesystem access belongs to the Git job owner, never serialized ingress.
    if !is_admissible_path(path) {
        return Err(());
    }
    let canonical = std::fs::canonicalize(path).map_err(|_| ())?;
    let value = canonical.to_str().ok_or(())?;
    if value.starts_with("\\\\?\\UNC\\") {
        return Err(());
    }
    let normalized = value
        .strip_prefix("\\\\?\\")
        .unwrap_or(value)
        .replace('\\', "/");
    AbsoluteCwd::new(&normalized).map_err(|_| ())
}

/// The spelling under which a checkout path Git recorded names the same checkout.
pub(crate) fn checkout_spelling(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}
