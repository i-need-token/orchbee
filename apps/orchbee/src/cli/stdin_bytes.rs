//! Bounded polling and EOF interpretation for an invocation-owned native stdin reader.
use std::{io, process::ExitStatus};
#[cfg(windows)]
#[path = "stdin_bytes/windows.rs"]
mod platform;
#[cfg(unix)]
#[path = "stdin_bytes/unix.rs"]
mod platform;

pub(in crate::cli) enum Poll {
    Pending,
    Bytes(usize),
    Eof,
}
trait NativeReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize>;
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
}
pub(in crate::cli) struct Reader {
    native: Box<dyn NativeReader>,
    eof: bool,
}
impl Reader {
    /// Reads the invocation's inherited stdin; the caller's descriptor flags never change.
    pub(in crate::cli) fn start() -> io::Result<Self> {
        platform::start().map(|native| Self { native, eof: false })
    }
    #[cfg(all(test, windows))]
    pub(in crate::cli) fn with_input(input: std::process::Stdio) -> io::Result<Self> {
        platform::with_input(input).map(|native| Self { native, eof: false })
    }
    pub(in crate::cli) fn poll(&mut self, buffer: &mut [u8]) -> io::Result<Poll> {
        if buffer.is_empty() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if !self.eof {
            match self.native.read(buffer) {
                Ok(0) => self.eof = true,
                Ok(count) => return Ok(Poll::Bytes(count)),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(Poll::Pending);
                }
                Err(error) => return Err(error),
            }
        }
        match self.native.try_wait()? {
            None => Ok(Poll::Pending),
            Some(status) if status.success() => Ok(Poll::Eof),
            Some(_) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
}
