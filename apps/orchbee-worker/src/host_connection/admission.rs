#[cfg(unix)]
use nix::{
    errno::Errno,
    sys::socket::{
        AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket, sockopt,
    },
};
use orchbee_local_ipc::{
    AsLocalIo, LocalStream as UnixStream, PollFd, PollFlags, PollTimeout, poll,
};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::{
    io::{self, Read, Write},
    path::Path,
    time::Instant,
};

/// One Host dial and handshake share the caller's startup deadline.
pub(super) struct Admission {
    pub(super) stream: UnixStream,
    deadline: Instant,
}
impl Admission {
    #[cfg(unix)]
    pub(super) fn connect(endpoint: &Path, deadline: Instant) -> io::Result<Self> {
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let address = UnixAddr::new(endpoint)?;
        let stream = UnixStream::from(socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            None,
        )?);
        let admission = Self { stream, deadline };
        match connect(admission.stream.as_raw_fd(), &address) {
            Ok(()) => {}
            Err(Errno::EINPROGRESS) => {
                admission.wait(PollFlags::POLLOUT)?;
                let error = getsockopt(&admission.stream, sockopt::SocketError)?;
                if error != 0 {
                    return Err(io::Error::from_raw_os_error(error));
                }
            }
            Err(error) => return Err(error.into()),
        }
        orchbee_local_peer_credentials::require_same_effective_user(&admission.stream)?;
        admission.remaining()?;
        Ok(admission)
    }
    #[cfg(windows)]
    pub(super) fn connect(endpoint: &Path, deadline: Instant) -> io::Result<Self> {
        let stream = UnixStream::connect_before(endpoint, deadline)?;
        stream.set_nonblocking(true)?;
        Ok(Self { stream, deadline })
    }
    pub(super) fn remaining(&self) -> io::Result<std::time::Duration> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(remaining)
    }
    fn wait(&self, interest: PollFlags) -> io::Result<()> {
        loop {
            let timeout = PollTimeout::try_from(self.remaining()?.as_millis().saturating_add(1))
                .unwrap_or(PollTimeout::MAX);
            let mut descriptors = [PollFd::new(self.stream.as_local_io(), interest)];
            match poll(&mut descriptors, timeout) {
                Ok(0) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Ok(_) => {
                    self.remaining()?;
                    return Ok(());
                }
                Err(error) => return Err(error),
            }
        }
    }
}
impl Read for Admission {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            self.wait(PollFlags::POLLIN)?;
            match self.stream.read(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
}
impl Write for Admission {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            self.wait(PollFlags::POLLOUT)?;
            match self.stream.write(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.remaining().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn byte_progress_cannot_extend_the_original_host_admission_deadline() -> io::Result<()> {
        let (stream, mut peer) = UnixStream::pair()?;
        stream.set_nonblocking(true)?;
        let peer = thread::spawn(move || {
            for _ in 0..5 {
                thread::sleep(Duration::from_millis(40));
                if peer.write_all(b"x").is_err() {
                    break;
                }
            }
        });
        let started = Instant::now();
        let mut admission = Admission {
            stream,
            deadline: started + Duration::from_millis(100),
        };
        let mut bytes = [0; 5];
        assert_eq!(
            admission.read_exact(&mut bytes).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(started.elapsed() < Duration::from_millis(190));
        assert_eq!(
            admission.write_all(b"after expiry").unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        drop(admission);
        peer.join().unwrap();
        Ok(())
    }
}
