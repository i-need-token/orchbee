//! Windows path spelling normalizes drive and UNC separators before shared control escaping;
//! non-Unicode machine paths keep their UTF-16 code units.
use serde_json::{Value, json};

pub(super) fn display(value: &str) -> String {
    let drive =
        matches!(value.as_bytes(), [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic());
    if drive || value.starts_with("//") {
        crate::terminal_text::unquoted(&value.replace('/', "\\"))
    } else {
        crate::terminal_text::unquoted(value)
    }
}

pub(super) fn detail(path: &std::path::Path) -> (String, Value) {
    use std::os::windows::ffi::OsStrExt;
    match path.to_str() {
        Some(text) => ("path".to_owned(), json!(text)),
        None => (
            "pathUtf16".to_owned(),
            json!(path.as_os_str().encode_wide().collect::<Vec<_>>()),
        ),
    }
}
