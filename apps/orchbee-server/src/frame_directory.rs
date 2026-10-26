//! Server-owned ephemeral frame roots; graphics leases determine retention and retry.
mod windows;
pub(crate) use windows::PrivateFrameDirectory;
