//! The inherited stdin read in place, each read gated on a zero-timeout poll, so the shared
//! polling loop keeps its deadline and cancellation without a helper process. The caller's file
//! description is never switched to nonblocking mode, because other processes share it. A read
//! stalls only when another reader of the same pipe drains it between the poll and the read, the
//! documented non-preemptible residual of reading a shared pipe in place.
use super::NativeReader;
use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use std::{
    io,
    os::{fd::AsFd, unix::process::ExitStatusExt},
    process::ExitStatus,
};

struct Inherited;

pub(super) fn start() -> io::Result<Box<dyn NativeReader>> {
    Ok(Box::new(Inherited))
}

impl NativeReader for Inherited {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let stdin = io::stdin();
        let fd = stdin.as_fd();
        let mut descriptors = [PollFd::new(
            fd,
            PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR,
        )];
        let ready = match poll(&mut descriptors, PollTimeout::ZERO) {
            Ok(_) => descriptors[0].revents().unwrap_or_else(PollFlags::empty),
            Err(Errno::EINTR) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(error) => return Err(error.into()),
        };
        if ready.contains(PollFlags::POLLNVAL) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if ready.is_empty() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        match nix::unistd::read(fd, bytes) {
            Ok(count) => Ok(count),
            Err(Errno::EINTR | Errno::EAGAIN) => Err(io::ErrorKind::WouldBlock.into()),
            Err(error) => Err(error.into()),
        }
    }

    /// No helper owns the input, so end of input is the descriptor's own EOF.
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(Some(ExitStatus::from_raw(0)))
    }
}
