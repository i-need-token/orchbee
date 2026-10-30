//! Private unit harnesses for the CLI-owned Agent integration adapters.

#[cfg(target_os = "linux")]
mod process_teardown;
#[cfg(windows)]
pub(super) mod windows_adapters;
