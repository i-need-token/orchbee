//! The emit finalizer consumes the native cancellation token after its last output attempt.
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use unix::SourceFinalization;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::SourceFinalization;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) use unsupported::SourceFinalization;
