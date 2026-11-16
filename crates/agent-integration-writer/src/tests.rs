//! Private unit harnesses for the CLI-owned Agent integration adapters.

#[cfg(target_os = "linux")]
mod process_teardown;
#[cfg(windows)]
pub(super) mod windows_adapters;

/// An absolute path a test spells for Windows, in the host's spelling: `C:\a\b` stays as it is on
/// Windows and reads `/a/b` elsewhere, so path-independent rules run on every host.
pub(crate) fn host_path(windows: &str) -> &'static std::path::Path {
    let spelled = if cfg!(windows) {
        windows.to_owned()
    } else {
        windows
            .strip_prefix("C:")
            .unwrap_or(windows)
            .replace('\\', "/")
    };
    Box::leak(std::path::PathBuf::from(spelled).into_boxed_path())
}
