//! Unix path diagnostics preserve native bytes and the existing unsupported-check row.
use serde_json::{Value, json};
use std::path::Path;

pub(super) const APPEND_UNSUPPORTED_CHECK: bool = true;

pub(super) fn path_detail(path: &Path) -> (String, Value) {
    match path.to_str() {
        Some(text) => ("path".to_owned(), json!(text)),
        None => {
            use std::os::unix::ffi::OsStrExt;
            let hex: String =
                path.as_os_str()
                    .as_bytes()
                    .iter()
                    .fold(String::new(), |mut output, byte| {
                        use std::fmt::Write;
                        let _ = write!(output, "{byte:02x}");
                        output
                    });
            ("pathBytesHex".to_owned(), json!(hex))
        }
    }
}
