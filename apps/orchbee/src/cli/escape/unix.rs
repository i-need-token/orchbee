//! Unix preserves reported path separators and delegates only control escaping.
pub(super) fn native_path(value: &str) -> String {
    super::unquoted(value)
}
