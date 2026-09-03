use std::{
    fmt,
    io::{self, Read, Write},
    os::fd::{AsFd, BorrowedFd},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

use orchbee_local_peer_credentials::require_same_effective_user;

use crate::{
    ClientHello, LocalClientMessage, ProtocolError, ReadMessageError, RejectionReason,
    WriteMessageError, read_message, write_message,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct ClientSessionConnection {
    generation: std::num::NonZeroU64,
    stream: UnixStream,
}

impl ClientSessionConnection {
    pub fn connect(endpoint: &Path, cohort: &str) -> Result<Self, ConnectClientError> {
        let hello = ClientHello::new(cohort)?;
        let mut stream = UnixStream::connect(endpoint)?;
        require_same_effective_user(&stream)?;
        stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
        stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;
        write_message(&mut stream, &LocalClientMessage::ClientHello(hello))?;

        let generation = match read_message(&mut stream)? {
            LocalClientMessage::ClientAccepted { generation } => generation,
            LocalClientMessage::Rejected { reason } => {
                return Err(ConnectClientError::Rejected(reason));
            }
            LocalClientMessage::ClientHello(_) => {
                return Err(ConnectClientError::UnexpectedResponse);
            }
        };
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        Ok(Self { generation, stream })
    }

    #[must_use]
    pub const fn generation(&self) -> std::num::NonZeroU64 {
        self.generation
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.stream.set_nonblocking(nonblocking)
    }

    pub fn wait_for_server_loss(&mut self) -> io::Result<()> {
        let mut byte = [0_u8; 1];
        loop {
            match self.read(&mut byte) {
                Ok(0) => return Ok(()),
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "local Client received a message after admission",
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::BrokenPipe
                    ) =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl AsFd for ClientSessionConnection {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

impl Read for ClientSessionConnection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buffer)
    }
}

impl Write for ClientSessionConnection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[derive(Debug)]
pub enum ConnectClientError {
    Io(io::Error),
    Protocol(ProtocolError),
    Rejected(RejectionReason),
    UnexpectedResponse,
}

impl fmt::Display for ConnectClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Protocol(error) => error.fmt(formatter),
            Self::Rejected(reason) => write!(formatter, "local Client was rejected: {reason:?}"),
            Self::UnexpectedResponse => {
                formatter.write_str("local Client received an unexpected handshake response")
            }
        }
    }
}

impl std::error::Error for ConnectClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Rejected(_) | Self::UnexpectedResponse => None,
        }
    }
}

impl From<io::Error> for ConnectClientError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProtocolError> for ConnectClientError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl From<ReadMessageError> for ConnectClientError {
    fn from(error: ReadMessageError) -> Self {
        match error {
            ReadMessageError::Io(error) => Self::Io(error),
            ReadMessageError::Protocol(error) => Self::Protocol(error),
        }
    }
}

impl From<WriteMessageError> for ConnectClientError {
    fn from(error: WriteMessageError) -> Self {
        match error {
            WriteMessageError::Io(error) => Self::Io(error),
            WriteMessageError::Protocol(error) => Self::Protocol(error),
        }
    }
}
