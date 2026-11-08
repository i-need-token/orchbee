//! Read-only operating-system facts for a diagnostic bundle: the product name and version the
//! host reports about itself, never anything about the person or their files.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::read_windows_version;

/// The Windows release a host reports, as `CurrentVersion` in the registry spells it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WindowsVersion {
    /// `ProductName`, such as `Windows 10 Pro`.
    pub product: Option<String>,
    /// `DisplayVersion`, such as `24H2`.
    pub display: Option<String>,
    /// `major.minor.build.revision`.
    pub version: Option<String>,
}
