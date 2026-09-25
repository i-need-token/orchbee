use super::*;
use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use std::time::Instant;

pub(super) fn server_metadata(
    endpoint: &Path,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<crate::ServerMetadata, ConnectClientError> {
    let stream = connect_stream(endpoint, deadline, wake)?;
    let mut transport = Admission {
        stream,
        deadline,
        wake,
        cancelled: None,
    };
    let exchange = (|| {
        write_message(&mut transport, &LocalClientMessage::ServerMetadataRequest)?;
        let metadata = match read_message(&mut transport)? {
            LocalClientMessage::ServerMetadata(metadata) => metadata,
            LocalClientMessage::Rejected { reason } => {
                return Err(ConnectClientError::Rejected(reason));
            }
            _ => return Err(ConnectClientError::UnexpectedResponse),
        };
        check_wake(wake)?;
        if Instant::now() >= deadline {
            return Err(ConnectClientError::Io(io::ErrorKind::TimedOut.into()));
        }
        Ok(metadata)
    })();
    if let Some(cancelled) = transport.cancelled {
        return Err(cancelled);
    }
    exchange
}

pub(super) fn connect(
    endpoint: &Path,
    hello: ClientHello,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<ClientSessionConnection, ConnectClientError> {
    let stream = connect_stream(endpoint, deadline, wake)?;
    let mut transport = Admission {
        stream,
        deadline,
        wake,
        cancelled: None,
    };
    let exchange = (|| {
        write_message(&mut transport, &LocalClientMessage::ClientHello(hello))?;
        let generation = match read_message(&mut transport)? {
            LocalClientMessage::ClientAccepted { generation } => generation,
            LocalClientMessage::Rejected { reason } => {
                return Err(ConnectClientError::Rejected(reason));
            }
            LocalClientMessage::ClientHello(_)
            | LocalClientMessage::SiteStopRequest(_)
            | LocalClientMessage::SiteStopResult(_)
            | LocalClientMessage::ServerMetadataRequest
            | LocalClientMessage::ServerMetadata(_) => {
                return Err(ConnectClientError::UnexpectedResponse);
            }
        };
        check_wake(wake)?;
        if Instant::now() >= deadline {
            return Err(ConnectClientError::Io(io::ErrorKind::TimedOut.into()));
        }
        Ok(generation)
    })();
    if let Some(cancelled) = transport.cancelled {
        return Err(cancelled);
    }
    Ok(ClientSessionConnection {
        generation: exchange?,
        stream: transport.stream,
    })
}

fn connect_stream(
    endpoint: &Path,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<UnixStream, ConnectClientError> {
    let stream = loop {
        check_wake(wake)?;
        if Instant::now() >= deadline {
            return Err(ConnectClientError::Io(io::ErrorKind::TimedOut.into()));
        }
        let slice = deadline.min(Instant::now() + Duration::from_millis(10));
        match UnixStream::connect_before(endpoint, slice) {
            Ok(stream) => break stream,
            Err(error) if matches!(error.raw_os_error(), Some(121 | 231)) => {}
            Err(error) => return Err(ConnectClientError::ConnectFailed(error)),
        }
    };
    stream.set_nonblocking(true)?;
    Ok(stream)
}

pub(super) fn site_stop(
    endpoint: &Path,
    request: crate::SiteStopRequest,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<crate::SiteStopResult, super::SiteStopExchangeError> {
    let mut submitted = false;
    let result = (|| {
        let stream = connect_stream(endpoint, deadline, wake)?;
        let mut transport = Admission {
            stream,
            deadline,
            wake,
            cancelled: None,
        };
        let exchange = (|| {
            let id = request.id;
            write_message(
                &mut transport,
                &LocalClientMessage::SiteStopRequest(request),
            )?;
            submitted = true;
            let result = match read_message(&mut transport)? {
                LocalClientMessage::SiteStopResult(result) if result.id == id => result,
                LocalClientMessage::Rejected { reason } => {
                    return Err(ConnectClientError::Rejected(reason));
                }
                _ => {
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
    result.map_err(|failure| super::SiteStopExchangeError { failure, submitted })
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
impl Admission<'_> {
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
