//! Pre-admission Server metadata exchange for already-admitted CLI capabilities.
//! Native ports preserve the endpoint, deadline and borrowed cancellation source.

#[cfg(windows)]
#[path = "server_metadata/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "server_metadata/unix.rs"]
mod platform;

pub(super) use platform::read;
