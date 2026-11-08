//! Reads `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`; a missing or unexpected value stays
//! unknown.
use super::WindowsVersion;
use winsafe::{self as w, co};

const CURRENT_VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

pub fn read_windows_version() -> WindowsVersion {
    let text = |name: &str| match w::HKEY::LOCAL_MACHINE
        .RegGetValue(Some(CURRENT_VERSION_KEY), Some(name), co::RRF::RT_REG_SZ)
        .ok()?
    {
        w::RegistryValue::Sz(value) => Some(value),
        _ => None,
    };
    let number = |name: &str| match w::HKEY::LOCAL_MACHINE
        .RegGetValue(Some(CURRENT_VERSION_KEY), Some(name), co::RRF::RT_REG_DWORD)
        .ok()?
    {
        w::RegistryValue::Dword(value) => Some(value),
        _ => None,
    };
    let version = match (
        number("CurrentMajorVersionNumber"),
        number("CurrentMinorVersionNumber"),
        text("CurrentBuildNumber"),
    ) {
        (Some(major), Some(minor), Some(build)) => Some(format!(
            "{major}.{minor}.{build}.{}",
            number("UBR").unwrap_or(0)
        )),
        _ => None,
    };
    WindowsVersion {
        product: text("ProductName"),
        display: text("DisplayVersion"),
        version,
    }
}
