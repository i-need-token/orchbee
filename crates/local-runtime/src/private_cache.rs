//! Private cache capability; applications own record semantics and publication policy.
#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
pub use unsupported::PrivateCache;
#[cfg(windows)]
pub use windows::PrivateCache;
