//! Unix child-output reader over an anonymous pipe, as a shell pipeline gives a child: a child may
//! open `/dev/stderr` or probe its output's type, which a socket would break. One nonblocking
//! capture loop serves both platforms.
use std::{
    io::{self, PipeReader, Read},
    os::fd::{AsFd, BorrowedFd},
    process::Stdio,
};

/// A nonblocking reader for a child process's owned stdout or stderr.
/// It grants no duplex or peer-authenticated protocol capability.
#[derive(Debug)]
pub struct ChildOutputReader(PipeReader);

impl ChildOutputReader {
    /// Creates an owned channel for Command::stdout/stderr. The child writer is blocking; only
    /// the parent's reader is nonblocking, so output is not lost when the child fills its buffer.
    /// The two ends are separate open files, so the reader's flag never reaches the writer. Both
    /// ends are close-on-exec; spawning duplicates the writer onto the child's descriptor.
    pub fn pipe() -> io::Result<(Self, Stdio)> {
        let (reader, writer) = io::pipe()?;
        rustix::io::ioctl_fionbio(&reader, true)?;
        Ok((Self(reader), Stdio::from(writer)))
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
