//! Hosts without an Agent read signal producer reject before acquiring native resources.
use super::NativeSignals;
use std::io;

pub(super) fn arm() -> io::Result<Box<dyn NativeSignals>> {
    Err(io::ErrorKind::Unsupported.into())
}
