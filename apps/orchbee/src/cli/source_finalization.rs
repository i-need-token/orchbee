//! The emit finalizer consumes the native cancellation token after its last output attempt.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::SourceFinalization;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::SourceFinalization;
#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) use unsupported::SourceFinalization;
