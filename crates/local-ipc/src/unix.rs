//! Unix child pipes over anonymous pipes, as a shell pipeline gives a child: a child may open
//! `/dev/stderr` or probe its streams' type, which a socket would break. One nonblocking capture
//! loop serves both platforms.
use std::{
    io::{self, PipeReader, PipeWriter, Read, Write},
    os::fd::{AsFd, BorrowedFd, OwnedFd},
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
        Self::pipe_descriptor().map(|(reader, writer)| (reader, Stdio::from(writer)))
    }

    /// [`Self::pipe`] with the writer as a descriptor, for a launch that duplicates it onto the
    /// child's standard output or error itself.
    pub fn pipe_descriptor() -> io::Result<(Self, OwnedFd)> {
        let (reader, writer) = io::pipe()?;
        rustix::io::ioctl_fionbio(&reader, true)?;
        Ok((Self(reader), OwnedFd::from(writer)))
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

/// A nonblocking writer for a child process's owned stdin. The child's reader stays blocking, and
/// dropping the writer ends the child's input.
#[derive(Debug)]
pub struct ChildInputWriter(PipeWriter);

impl ChildInputWriter {
    /// Creates an owned channel for Command::stdin. The two ends are separate open files, so the
    /// writer's flag never reaches the reader. Both ends are close-on-exec; spawning duplicates the
    /// reader onto the child's descriptor.
    pub fn pipe() -> io::Result<(Self, Stdio)> {
        let (reader, writer) = io::pipe()?;
        rustix::io::ioctl_fionbio(&writer, true)?;
        Ok((Self(writer), Stdio::from(OwnedFd::from(reader))))
    }
}

impl Write for ChildInputWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
