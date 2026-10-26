//! Windows path spelling normalizes drive and UNC separators before shared control escaping.
pub(super) fn native_path(value: &str) -> String {
    let drive =
        matches!(value.as_bytes(), [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic());
    if drive || value.starts_with("//") {
        super::unquoted(&value.replace('/', "\\"))
    } else {
        super::unquoted(value)
    }
}
