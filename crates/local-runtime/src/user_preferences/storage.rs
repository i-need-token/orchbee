//! Platform storage adapters implement the same source observation and guarded commit contract.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(super) use unix::{Writer, read};
#[cfg(windows)]
pub(super) use windows::{Writer, read};
