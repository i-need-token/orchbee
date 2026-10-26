//! Streaming stdout is unavailable on Unix; no writer or subprocess is acquired.
use super::NativeRelay;
use std::io;
pub(super) fn start() -> io::Result<Box<dyn NativeRelay>> {
    Err(io::ErrorKind::Unsupported.into())
}
