//! Server-owned ephemeral frame roots; graphics leases determine retention and retry.
#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
pub(crate) use unsupported::PrivateFrameDirectory;
#[cfg(windows)]
pub(crate) use windows::PrivateFrameDirectory;
