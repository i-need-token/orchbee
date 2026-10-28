//! Private unit harnesses for the CLI-owned Agent integration adapters.

#[cfg(unix)]
pub(super) mod fault_injection;
#[cfg(target_os = "linux")]
mod process_teardown;
#[cfg(unix)]
mod provider_conformance;
#[cfg(unix)]
mod provider_effects;
#[cfg(windows)]
pub(super) mod windows_adapters;
