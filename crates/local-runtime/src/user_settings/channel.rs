//! Save-channel capability; downloads and installation remain application-owned.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::set_update_channel;
#[cfg(not(windows))]
pub fn set_update_channel(
    _channel: super::UpdateChannel,
    _env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<std::path::PathBuf, super::SettingsError> {
    Err(super::SettingsError::new(
        "platform",
        "unsupported-platform",
    ))
}
