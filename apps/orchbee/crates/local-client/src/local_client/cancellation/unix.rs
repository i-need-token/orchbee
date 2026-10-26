//! A real nonblocking pipe keeps Unix cancellation inside the existing FD poll contract.
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use orchbee_local_ipc::LocalIo;
use std::{
    io::{self, PipeReader, PipeWriter, Write},
    os::fd::AsFd,
    sync::{Mutex, atomic::AtomicU8},
};

pub(super) struct Signal {
    read: PipeReader,
    // Serializes notification with terminal failure/closure. No reader consumes the pipe.
    write: Mutex<Option<PipeWriter>>,
}

impl Signal {
    pub(super) fn new() -> io::Result<Self> {
        let (read, write) = io::pipe()?;
        for fd in [read.as_fd(), write.as_fd()] {
            let flags = OFlag::from_bits_retain(fcntl(fd, FcntlArg::F_GETFL)?);
            fcntl(fd, FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK))?;
        }
        Ok(Self {
            read,
            write: Mutex::new(Some(write)),
        })
    }

    pub(super) fn wake<'a>(&'a self, _state: &'a AtomicU8) -> LocalIo<'a> {
        self.read.as_fd()
    }

    pub(super) fn notify(&self) {
        let mut write = match self.write.lock() {
            Ok(write) => write,
            Err(poisoned) => {
                // State is not trusted after poison. Closing the sole writer wakes FD waiters
                // with HUP, which the transport classifies as a wake fault, not cancellation.
                poisoned.into_inner().take();
                return;
            }
        };
        if let Some(writer) = write.as_mut() {
            // Do not let a signal storm turn cancellation/Drop into an unbounded retry loop.
            for _ in 0..8 {
                match writer.write(&[1]) {
                    Ok(1) => return,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Ok(_) | Err(_) => break,
                }
            }
        }
        // A failed notification must still end an in-flight wait. Retain the read end and
        // close the sole writer: native poll observes HUP and the worker settles before join.
        write.take();
    }
}

#[cfg(test)]
mod tests;
