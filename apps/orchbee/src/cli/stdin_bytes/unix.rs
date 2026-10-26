//! Optional byte-stream input is unsupported on Unix and starts no helper.
use super::NativeReader;
use std::{io, process::Stdio};
pub(super) fn with_input(_input: Stdio) -> io::Result<Box<dyn NativeReader>> {
    Err(io::ErrorKind::Unsupported.into())
}
