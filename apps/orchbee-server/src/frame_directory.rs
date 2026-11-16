//! Server-owned ephemeral frame roots; graphics leases determine retention and retry.
#[cfg(unix)]
mod posix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(crate) use posix::PrivateFrameDirectory;
#[cfg(windows)]
pub(crate) use windows::PrivateFrameDirectory;
