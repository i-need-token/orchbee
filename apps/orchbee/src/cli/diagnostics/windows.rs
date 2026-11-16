//! Windows bundle facts: the Windows product and build, and a process's image.
use serde_json::{Value, json};
use std::path::Path;
use winsafe::{self as w, co};

/// `windows` and its product, display version and build.
pub(super) fn release() -> Option<(&'static str, Value)> {
    let windows = orchbee_local_runtime::read_windows_version();
    Some((
        "windows",
        json!({
            "product": windows.product,
            "display": windows.display,
            "version": windows.version,
        }),
    ))
}

/// Whether the process `pid` runs the image `expected`, compared without case; `None` when the
/// process cannot be inspected.
pub(super) fn runs(pid: u32, expected: &Path) -> Option<bool> {
    let image = w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, pid)
        .and_then(|process| process.QueryFullProcessImageName(co::PROCESS_NAME::WIN32))
        .ok()?;
    Some(
        expected
            .to_str()
            .is_some_and(|expected| expected.eq_ignore_ascii_case(&image)),
    )
}
