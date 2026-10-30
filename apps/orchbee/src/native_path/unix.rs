//! Unix preserves reported path separators and delegates only control escaping; non-UTF-8
//! machine paths keep their native bytes.
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn display(value: &str) -> String {
    crate::terminal_text::unquoted(value)
}

pub(super) fn detail(path: &Path) -> (String, Value) {
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
