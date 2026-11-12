//! Unix repository paths: absolute UTF-8 paths, compared byte for byte.
use orchbee_capability_node_protocol::AbsoluteCwd;
use std::path::Path;

pub(crate) const GIT_PROGRAM: &str = "git";

pub(crate) fn is_admissible_path(path: &Path) -> bool {
    path.is_absolute()
}

pub(crate) fn canonical_path(path: &Path) -> Result<AbsoluteCwd, ()> {
    // Filesystem access belongs to the Git job owner, never serialized ingress.
    if !is_admissible_path(path) {
        return Err(());
    }
    let canonical = std::fs::canonicalize(path).map_err(|_| ())?;
    AbsoluteCwd::new(canonical.to_str().ok_or(())?).map_err(|_| ())
}

/// The spelling under which a checkout path Git recorded names the same checkout.
pub(crate) fn checkout_spelling(value: &str) -> String {
    value.trim_end_matches('/').to_owned()
}
