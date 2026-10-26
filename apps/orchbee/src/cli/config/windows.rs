//! Windows path diagnostics preserve UTF-16 and its existing complete check inventory.
use serde_json::{Value, json};

pub(super) const APPEND_UNSUPPORTED_CHECK: bool = false;

pub(super) fn path_detail(path: &std::path::Path) -> (String, Value) {
    use std::os::windows::ffi::OsStrExt;
    match path.to_str() {
        Some(text) => ("path".to_owned(), json!(text)),
        None => (
            "pathUtf16".to_owned(),
            json!(path.as_os_str().encode_wide().collect::<Vec<_>>()),
        ),
    }
}
