use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use std::{
    io::{self, Read, Write},
    os::{fd::AsFd, unix::net::UnixStream},
    time::{Duration, Instant},
};

const RECORD_BUDGET: Duration = Duration::from_secs(2);

pub(super) struct ClientRecordStream<'stream> {
    stream: &'stream UnixStream,
    read_deadline: Option<Instant>,
    write_deadline: Option<Instant>,
}
impl<'stream> ClientRecordStream<'stream> {
    pub(super) fn new(stream: &'stream UnixStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            read_deadline: None,
            write_deadline: None,
        })
    }
    fn wait(&self, interest: PollFlags, deadline: Option<Instant>) -> io::Result<()> {
        loop {
            let timeout = match deadline {
                None => PollTimeout::NONE,
                Some(deadline) => {
                    let remaining = remaining(deadline)?;
                    PollTimeout::try_from(remaining.as_millis().saturating_add(1))
                        .map_err(io::Error::other)?
                }
            };
            let mut fds = [PollFd::new(self.stream.as_fd(), interest)];
            match poll(&mut fds, timeout) {
                Ok(0) => return Err(io::ErrorKind::TimedOut.into()),
                Ok(_) => return Ok(()),
                Err(Errno::EINTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}
impl Read for ClientRecordStream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.write_deadline = None;
        loop {
            if let Some(deadline) = self.read_deadline {
                remaining(deadline)?;
            }
            match self.stream.read(buffer) {
                Ok(count) => {
                    if count > 0 && self.read_deadline.is_none() {
                        self.read_deadline = Some(Instant::now() + RECORD_BUDGET);
                    }
                    return Ok(count);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.wait(PollFlags::POLLIN, self.read_deadline)?
                }
                Err(error) => return Err(error),
            }
        }
    }
}
impl Write for ClientRecordStream<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.read_deadline = None;
        let deadline = *self
            .write_deadline
            .get_or_insert_with(|| Instant::now() + RECORD_BUDGET);
        loop {
            remaining(deadline)?;
            match self.stream.write(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.wait(PollFlags::POLLOUT, Some(deadline))?
                }
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| io::ErrorKind::TimedOut.into())
}
