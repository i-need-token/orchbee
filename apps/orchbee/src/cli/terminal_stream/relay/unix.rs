//! The inherited stdout written in place: each length-prefixed record's body goes to stdout and
//! is acknowledged once complete, so no helper process relays it. Every write waits for a
//! zero-timeout `POLLOUT` and writes at most `PIPE_BUF` bytes, so a blocked reader leaves the
//! shared loop its deadline and cancellation; the caller's file description is never switched to
//! nonblocking mode. A write stalls only when another writer fills the same pipe between the poll
//! and the write, the documented non-preemptible residual of writing a shared pipe in place.
use super::NativeRelay;
use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use std::{
    io,
    os::fd::{AsFd, BorrowedFd},
};

/// A record body the helper protocol admits: nonempty and at most 8 MiB.
const MAXIMUM_RECORD: usize = 8 * 1024 * 1024;

#[derive(Default)]
struct Inherited {
    header: [u8; 4],
    header_len: usize,
    body_remaining: usize,
    acks: usize,
}

pub(super) fn start() -> io::Result<Box<dyn NativeRelay>> {
    Ok(Box::<Inherited>::default())
}

impl NativeRelay for Inherited {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.body_remaining == 0 {
            let taken = bytes.len().min(self.header.len() - self.header_len);
            self.header[self.header_len..self.header_len + taken].copy_from_slice(&bytes[..taken]);
            self.header_len += taken;
            if self.header_len == self.header.len() {
                let size = usize::try_from(u32::from_le_bytes(self.header))
                    .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
                if size == 0 || size > MAXIMUM_RECORD {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                self.header_len = 0;
                self.body_remaining = size;
            }
            return Ok(taken);
        }
        let stdout = io::stdout();
        let fd = stdout.as_fd();
        if !writable(fd)? {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let length = bytes
            .len()
            .min(self.body_remaining)
            .min(nix::libc::PIPE_BUF);
        let written = match nix::unistd::write(fd, &bytes[..length]) {
            Ok(written) => written,
            Err(Errno::EINTR | Errno::EAGAIN) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(error) => return Err(error.into()),
        };
        self.body_remaining -= written;
        if written > 0 && self.body_remaining == 0 {
            self.acks += 1;
        }
        Ok(written)
    }

    fn read_ack(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let Some(first) = bytes.first_mut() else {
            return Err(io::ErrorKind::InvalidInput.into());
        };
        if self.acks == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.acks -= 1;
        *first = 1;
        Ok(1)
    }

    fn stop(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn writable(fd: BorrowedFd<'_>) -> io::Result<bool> {
    let mut descriptors = [PollFd::new(fd, PollFlags::POLLOUT)];
    match poll(&mut descriptors, PollTimeout::ZERO) {
        Ok(_) => {
            let ready = descriptors[0].revents().unwrap_or_else(PollFlags::empty);
            if ready.intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL) {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            Ok(ready.contains(PollFlags::POLLOUT))
        }
        Err(Errno::EINTR) => Ok(false),
        Err(error) => Err(error.into()),
    }
}
