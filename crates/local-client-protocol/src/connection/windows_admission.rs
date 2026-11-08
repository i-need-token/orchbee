use super::{
    AsLocalIo, CANDIDATE_DEADLINE, ClientConnection, ConnectClientError, LocalIo, Offer, Path,
    Read, UnixStream, Write, handshake, io,
};
use crate::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use std::time::{Duration, Instant};

/// Asks for the Server's processes; the connection closes with the reply.
pub(super) fn query(
    endpoint: &Path,
    environment: orchbee_connection_handshake::EnvironmentName,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<orchbee_connection_handshake::QueryResult, ConnectClientError> {
    let mut transport =
        Admission::candidate(connect_stream(endpoint, deadline, wake)?, deadline, wake);
    let exchange = (|| -> Result<_, ConnectClientError> {
        let result = handshake::query(&mut transport, environment)?;
        settle(wake, deadline)?;
        Ok(result)
    })();
    if let Some(cancelled) = transport.cancelled {
        return Err(cancelled);
    }
    exchange
}

pub(super) fn connect(
    endpoint: &Path,
    offer: &Offer,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<ClientConnection, ConnectClientError> {
    let mut transport =
        Admission::candidate(connect_stream(endpoint, deadline, wake)?, deadline, wake);
    let exchange = (|| -> Result<_, ConnectClientError> {
        handshake::offer(&mut transport, offer)?;
        transport.deadline = deadline;
        let generation = handshake::enter_client_phase(&mut transport)?;
        settle(wake, deadline)?;
        Ok(generation)
    })();
    if let Some(cancelled) = transport.cancelled {
        return Err(cancelled);
    }
    Ok(ClientConnection {
        generation: exchange?,
        stream: transport.stream,
    })
}

/// A reply that arrived after a wake or past the deadline does not count.
fn settle(wake: Option<LocalIo<'_>>, deadline: Instant) -> Result<(), ConnectClientError> {
    check_wake(wake)?;
    if Instant::now() >= deadline {
        return Err(ConnectClientError::Io(io::ErrorKind::TimedOut.into()));
    }
    Ok(())
}

fn connect_stream(
    endpoint: &Path,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<UnixStream, ConnectClientError> {
    let stream = connect_stream_with(
        deadline,
        Instant::now,
        || check_wake(wake),
        |deadline, wait_limit| {
            UnixStream::connect_before_with_wait_limit(endpoint, deadline, wait_limit).map_err(
                |error| match error {
                    orchbee_local_ipc::ConnectAttemptError::Dial(error) => {
                        ConnectClientError::ConnectFailed(error)
                    }
                    orchbee_local_ipc::ConnectAttemptError::Admission(error) => {
                        ConnectClientError::Io(error)
                    }
                },
            )
        },
    )?;
    stream.set_nonblocking(true)?;
    Ok(stream)
}

/// Owns cancellation cadence and the one operation deadline; connected candidates never retry.
fn connect_stream_with<T>(
    deadline: Instant,
    mut now: impl FnMut() -> Instant,
    mut wake: impl FnMut() -> Result<(), ConnectClientError>,
    mut connect: impl FnMut(Instant, Duration) -> Result<Option<T>, ConnectClientError>,
) -> Result<T, ConnectClientError> {
    loop {
        wake()?;
        let current = now();
        if current >= deadline {
            return Err(ConnectClientError::Io(io::ErrorKind::TimedOut.into()));
        }
        let wait_limit = Duration::from_millis(10);
        if let Some(stream) = connect(deadline, wait_limit)? {
            wake()?;
            if now() >= deadline {
                return Err(ConnectClientError::Io(io::ErrorKind::TimedOut.into()));
            }
            return Ok(stream);
        }
    }
}

#[cfg(test)]
mod tests;

pub(super) fn session_stop(
    endpoint: &Path,
    offer: &Offer,
    request: crate::SessionStopRequest,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<crate::SessionStopResult, super::SessionStopExchangeError> {
    let mut submitted = false;
    let result = (|| {
        let mut transport =
            Admission::candidate(connect_stream(endpoint, deadline, wake)?, deadline, wake);
        let exchange = (|| {
            handshake::offer(&mut transport, offer)?;
            transport.deadline = deadline;
            let id = request.id;
            write_message(
                &mut transport,
                &LocalClientMessage::SessionStopRequest(request),
            )?;
            submitted = true;
            let result = match read_message(&mut transport)? {
                LocalClientMessage::SessionStopResult(result) if result.id == id => result,
                LocalClientMessage::Rejected { reason } => {
                    return Err(ConnectClientError::Rejected(reason));
                }
                LocalClientMessage::ClientHello
                | LocalClientMessage::ClientAccepted { .. }
                | LocalClientMessage::SessionStopRequest(..)
                | LocalClientMessage::SessionStopResult(..) => {
                    return Err(ConnectClientError::UnexpectedResponse);
                }
            };
            Ok(result)
        })();
        if exchange.is_err()
            && let Some(cancelled) = transport.cancelled
        {
            return Err(cancelled);
        }
        exchange
    })();
    result.map_err(|failure| super::SessionStopExchangeError { failure, submitted })
}

pub(super) fn exit(
    endpoint: &Path,
    environment: orchbee_connection_handshake::EnvironmentName,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<super::ExitReply, super::SessionStopExchangeError> {
    let mut submitted = false;
    let result = (|| {
        let mut transport =
            Admission::candidate(connect_stream(endpoint, deadline, wake)?, deadline, wake);
        let exchange = (|| {
            handshake::request_exit(&mut transport, environment)?;
            submitted = true;
            // The Server replies once it has settled, so the reply is due by the caller's deadline.
            transport.deadline = deadline;
            handshake::exit_reply(&mut transport)
        })();
        if exchange.is_err()
            && let Some(cancelled) = transport.cancelled
        {
            return Err(cancelled);
        }
        exchange
    })();
    result.map_err(|failure| super::SessionStopExchangeError { failure, submitted })
}

fn check_wake(wake: Option<LocalIo<'_>>) -> Result<(), ConnectClientError> {
    let Some(wake) = wake else {
        return Ok(());
    };
    let mut sources = [PollFd::new(wake, PollFlags::POLLIN)];
    poll(&mut sources, PollTimeout::ZERO).map_err(ConnectClientError::CancellationSourceFault)?;
    let events = sources[0].revents().unwrap_or(PollFlags::POLLNVAL);
    if events.intersects(PollFlags::POLLHUP | PollFlags::POLLERR | PollFlags::POLLNVAL) {
        return Err(ConnectClientError::CancellationSourceFault(
            io::ErrorKind::BrokenPipe.into(),
        ));
    }
    if events.contains(PollFlags::POLLIN) {
        return Err(ConnectClientError::CancellationReady);
    }
    Ok(())
}

struct Admission<'a> {
    stream: UnixStream,
    deadline: Instant,
    wake: Option<LocalIo<'a>>,
    cancelled: Option<ConnectClientError>,
}
impl<'a> Admission<'a> {
    /// The handshake reply is due within the candidate deadline, inside the caller's.
    fn candidate(stream: UnixStream, deadline: Instant, wake: Option<LocalIo<'a>>) -> Self {
        Self {
            stream,
            deadline: deadline.min(Instant::now() + CANDIDATE_DEADLINE),
            wake,
            cancelled: None,
        }
    }

    fn wait(&mut self, interest: PollFlags) -> io::Result<()> {
        loop {
            if let Err(error) = check_wake(self.wake) {
                self.cancelled = Some(error);
                return Err(io::Error::other("local admission cancelled"));
            }
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let mut sources = [PollFd::new(self.stream.as_local_io(), interest)];
            poll(
                &mut sources,
                PollTimeout::try_from(remaining.min(Duration::from_millis(10)))?,
            )?;
            if sources[0]
                .revents()
                .is_some_and(|events| !events.is_empty())
            {
                return Ok(());
            }
        }
    }
}
impl Read for Admission<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            self.wait(PollFlags::POLLIN)?;
            match self.stream.read(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                result => return result,
            }
        }
    }
}
impl Write for Admission<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            self.wait(PollFlags::POLLOUT)?;
            match self.stream.write(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
