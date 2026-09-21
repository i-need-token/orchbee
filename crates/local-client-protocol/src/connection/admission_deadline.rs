use std::{
    fmt,
    io::{self, Read, Write},
    num::NonZeroU64,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd},
        unix::net::UnixStream,
    },
    path::Path,
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::socket::{
        AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket, sockopt,
    },
};
use orchbee_local_peer_credentials::require_same_effective_user;

use super::{ClientSessionConnection, ConnectClientError};
use crate::{ClientHello, LocalClientMessage, read_message, write_message};

pub(super) fn connect_before_cancellable(
    endpoint: &Path,
    hello: ClientHello,
    expires_at: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<ClientSessionConnection, ConnectClientError> {
    admit(endpoint, hello, expires_at, wake).map_err(translate_wake)
}

fn admit(
    endpoint: &Path,
    hello: ClientHello,
    expires_at: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<ClientSessionConnection, ConnectClientError> {
    let deadline = Deadline {
        expires_at,
        now: &Instant::now,
        wake,
    };
    deadline.remaining()?;
    let address = UnixAddr::new(endpoint).map_err(io::Error::from)?;
    deadline.remaining()?;
    let descriptor = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
        None,
    )
    .map_err(io::Error::from)?;
    let mut stream = UnixStream::from(descriptor);
    deadline.remaining()?;
    // A backlog EAGAIN is not a pending Unix connect. Only EINPROGRESS permits
    // waiting; every other error drops this one socket without another attempt.
    let result = connect(stream.as_raw_fd(), &address);
    complete_connect(&stream, result, deadline).map_err(ConnectClientError::ConnectFailed)?;
    require_same_effective_user(&stream)?;
    deadline.remaining()?;
    let generation = exchange_hello(&mut stream, hello, deadline)?;
    finish_admission(stream, generation, deadline)
}

fn complete_connect(
    stream: &UnixStream,
    result: Result<(), Errno>,
    deadline: Deadline<'_>,
) -> io::Result<()> {
    match result {
        Ok(()) => {}
        Err(Errno::EINPROGRESS) => {
            deadline.wait(stream, PollFlags::POLLOUT)?;
            let error = getsockopt(stream, sockopt::SocketError)?;
            if error != 0 {
                return Err(io::Error::from_raw_os_error(error));
            }
        }
        Err(error) => return Err(error.into()),
    }
    deadline.remaining()?;
    Ok(())
}

fn exchange_hello(
    stream: &mut (impl Read + Write + AsFd),
    hello: ClientHello,
    deadline: Deadline<'_>,
) -> Result<NonZeroU64, ConnectClientError> {
    let mut transport = DeadlineIo { stream, deadline };
    write_message(&mut transport, &LocalClientMessage::ClientHello(hello))?;
    let response = read_message(&mut transport);
    deadline.remaining()?;
    match response? {
        LocalClientMessage::ClientAccepted { generation } => Ok(generation),
        LocalClientMessage::Rejected { reason } => Err(ConnectClientError::Rejected(reason)),
        LocalClientMessage::ClientHello(_) => Err(ConnectClientError::UnexpectedResponse),
    }
}

fn finish_admission(
    stream: UnixStream,
    generation: NonZeroU64,
    deadline: Deadline<'_>,
) -> Result<ClientSessionConnection, ConnectClientError> {
    deadline.remaining()?;
    stream.set_nonblocking(false)?;
    // Success linearizes after restoring the stream mode, not at frame arrival.
    deadline.remaining()?;
    Ok(ClientSessionConnection { stream, generation })
}

/// The borrowed wake descriptor reported `POLLIN` — a provisional wake
/// observation only. The protocol never reads the descriptor; the caller's
/// own signal owner performs the sole arbitration read and decides whether
/// any cancellation happened.
#[derive(Debug)]
struct WakeReady;

impl fmt::Display for WakeReady {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancellation wake descriptor reported readiness")
    }
}

impl std::error::Error for WakeReady {}

/// The borrowed wake descriptor reported `POLLERR`, `POLLHUP`, or
/// `POLLNVAL`: the wake channel itself is broken, distinct from a wake.
#[derive(Debug)]
struct WakeFault(PollFlags);

impl fmt::Display for WakeFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cancellation wake descriptor reported fault flags {:?}",
            self.0
        )
    }
}

impl std::error::Error for WakeFault {}

/// Surface private wake observations at the public error boundary. Only the
/// `Io` and `ConnectFailed` wrappers can carry one; every other variant
/// already holds its own typed provenance.
fn translate_wake(error: ConnectClientError) -> ConnectClientError {
    match error {
        ConnectClientError::ConnectFailed(inner) | ConnectClientError::Io(inner)
            if is_wake::<WakeReady>(&inner) =>
        {
            ConnectClientError::CancellationReady
        }
        ConnectClientError::ConnectFailed(inner) | ConnectClientError::Io(inner)
            if is_wake::<WakeFault>(&inner) =>
        {
            ConnectClientError::CancellationSourceFault(inner)
        }
        error => error,
    }
}

fn is_wake<M: std::error::Error + 'static>(error: &io::Error) -> bool {
    error.get_ref().is_some_and(|inner| inner.is::<M>())
}

#[derive(Clone, Copy)]
struct Deadline<'a> {
    expires_at: Instant,
    now: &'a dyn Fn() -> Instant,
    wake: Option<BorrowedFd<'a>>,
}

impl Deadline<'_> {
    fn remaining(self) -> io::Result<Duration> {
        match self.expires_at.checked_duration_since((self.now)()) {
            Some(remaining) if !remaining.is_zero() => Ok(remaining),
            Some(_) | None => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "local Client admission total deadline elapsed",
            )),
        }
    }

    fn wait(self, stream: &impl AsFd, interest: PollFlags) -> io::Result<()> {
        loop {
            let remaining = self.remaining()?;
            let milliseconds = remaining.as_millis().saturating_add(u128::from(
                !remaining.subsec_nanos().is_multiple_of(1_000_000),
            ));
            let timeout = PollTimeout::try_from(milliseconds).unwrap_or(PollTimeout::MAX);
            let Some(wake) = self.wake else {
                let mut descriptors = [PollFd::new(stream.as_fd(), interest)];
                match poll(&mut descriptors, timeout) {
                    Ok(0) | Err(Errno::EINTR) => {}
                    Ok(_) => {
                        self.remaining()?;
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                }
                continue;
            };
            // The borrowed descriptor only joins the poll set; it is never
            // read, drained, owned, or closed here.
            let mut descriptors = [
                PollFd::new(stream.as_fd(), interest),
                PollFd::new(wake, PollFlags::POLLIN),
            ];
            match poll(&mut descriptors, timeout) {
                Ok(0) | Err(Errno::EINTR) => {}
                Ok(_) => {
                    // A wake event is decided before any simultaneous socket
                    // readiness. POLLIN wins over fault flags because pending
                    // bytes still belong to the owner's arbitration read; a
                    // fault with nothing readable is a broken channel.
                    let observed = descriptors[1].revents().unwrap_or(PollFlags::empty());
                    if observed.contains(PollFlags::POLLIN) {
                        return Err(io::Error::other(WakeReady));
                    }
                    if observed
                        .intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL)
                    {
                        return Err(io::Error::other(WakeFault(observed)));
                    }
                    if descriptors[0]
                        .revents()
                        .unwrap_or(PollFlags::empty())
                        .is_empty()
                    {
                        continue;
                    }
                    self.remaining()?;
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

struct DeadlineIo<'a, Stream> {
    stream: &'a mut Stream,
    deadline: Deadline<'a>,
}

impl<Stream: Read + Write + AsFd> DeadlineIo<'_, Stream> {
    fn perform(
        &mut self,
        interest: PollFlags,
        mut operation: impl FnMut(&mut Stream) -> io::Result<usize>,
    ) -> io::Result<usize> {
        loop {
            self.deadline.wait(self.stream, interest)?;
            match operation(self.stream) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                    ) => {}
                result => return result,
            }
        }
    }
}

impl<Stream: Read + Write + AsFd> Read for DeadlineIo<'_, Stream> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.perform(PollFlags::POLLIN, |stream| stream.read(buffer))
    }
}

impl<Stream: Read + Write + AsFd> Write for DeadlineIo<'_, Stream> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.perform(PollFlags::POLLOUT, |stream| stream.write(buffer))
    }

    fn flush(&mut self) -> io::Result<()> {
        // UnixStream has no user-space write buffer.
        self.deadline.remaining().map(|_| ())
    }
}

#[cfg(test)]
mod tests;
