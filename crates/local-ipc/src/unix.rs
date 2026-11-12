//! Unix child-output reader: a connected socket pair stands in for the Windows pipe pair, so one
//! nonblocking capture loop serves both platforms.
use std::{
    io::{self, Read},
    os::{
        fd::{AsFd, BorrowedFd, OwnedFd},
        unix::net::UnixStream,
    },
    process::Stdio,
};

/// A nonblocking reader for a child process's owned stdout or stderr.
/// It grants no duplex or peer-authenticated protocol capability.
#[derive(Debug)]
pub struct ChildOutputReader(UnixStream);

impl ChildOutputReader {
    /// Creates an owned channel for Command::stdout/stderr. The child writer is blocking; only
    /// the parent's reader is nonblocking, so output is not lost when the child fills its buffer.
    /// Both ends are close-on-exec; spawning duplicates the writer onto the child's descriptor.
    pub fn pipe() -> io::Result<(Self, Stdio)> {
        let (reader, writer) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        Ok((Self(reader), Stdio::from(OwnedFd::from(writer))))
    }
}

impl Read for ChildOutputReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer)
    }
}

impl AsFd for ChildOutputReader {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
