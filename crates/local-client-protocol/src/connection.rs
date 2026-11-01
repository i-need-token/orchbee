//! Same-user Client connection admission and pre-admission control exchanges. Each attempt owns
//! its candidate stream until failure or transfer; callers retain deadline, endpoint, cancellation
//! arbitration and service-lifecycle policy.

use orchbee_local_ipc::LocalStream as UnixStream;
#[cfg(windows)]
use orchbee_local_ipc::{AsLocalIo, LocalIo};
#[cfg(unix)]
use std::os::fd::{AsFd, BorrowedFd};
use std::{
    fmt,
    io::{self, Read, Write},
    path::Path,
    time::Duration,
};

#[cfg(unix)]
use orchbee_local_peer_credentials::require_same_effective_user;

use crate::{
    ClientHello, LocalClientMessage, ProtocolError, ReadMessageError, RejectionReason,
    WriteMessageError, read_message, write_message,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

/// Observe the same-user Windows Server before exact-cohort Connection admission.
/// The candidate never enters an application phase and is dropped before returning.
#[cfg(windows)]
pub fn exchange_server_metadata_before(
    endpoint: &Path,
    deadline: std::time::Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<crate::ServerMetadata, ConnectClientError> {
    windows_admission::server_metadata(endpoint, deadline, wake)
}

/// Retains whether a complete stop control record was submitted before exchange failure.
#[cfg(windows)]
#[derive(Debug)]
pub struct SessionStopExchangeError {
    pub failure: ConnectClientError,
    pub submitted: bool,
}

/// One same-user stop/status control exchange, with no activation or mutation retry.
#[cfg(windows)]
pub fn exchange_session_stop_before(
    endpoint: &Path,
    request: crate::SessionStopRequest,
    deadline: std::time::Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<crate::SessionStopResult, SessionStopExchangeError> {
    windows_admission::session_stop(endpoint, request, deadline, wake)
}

/// Byte capacity of a filesystem-path Unix socket address on this platform's
/// socket ABI: `sockaddr_un.sun_path` minus the trailing NUL a pathname
/// address requires. Derived from the ABI struct, not assumed portable —
/// Linux yields 107 while BSD/Darwin layouts yield 103.
#[cfg(unix)]
pub const UNIX_SOCKET_PATH_CAPACITY: usize = std::mem::size_of::<nix::libc::sockaddr_un>()
    - std::mem::offset_of!(nix::libc::sockaddr_un, sun_path)
    - 1;

/// Whether `path` fits inside one `sockaddr_un` filesystem address on this
/// platform — the same `UnixAddr` construction admission performs before it
/// ever connects, so inspection and startup share one production judgment.
#[cfg(unix)]
pub fn unix_socket_path_fits(path: &Path) -> bool {
    nix::sys::socket::UnixAddr::new(path).is_ok()
}

#[cfg(target_os = "linux")]
const _: () = assert!(UNIX_SOCKET_PATH_CAPACITY == 107);

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod admission_deadline;

#[derive(Debug)]
pub struct ClientConnection {
    generation: std::num::NonZeroU64,
    stream: UnixStream,
}

impl ClientConnection {
    /// Admits one Unix connection within the caller's existing monotonic deadline.
    /// This neither selects a timeout nor activates, retries, or supervises a Session.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub fn connect_before(
        endpoint: &Path,
        cohort: &str,
        deadline: std::time::Instant,
    ) -> Result<Self, ConnectClientError> {
        Self::connect_before_cancellable(endpoint, cohort, deadline, None)
    }

    /// The same admission plus one optional borrowed wake descriptor observed
    /// during connect and handshake waits. The descriptor is a wake-up
    /// capability only: it is never owned, duplicated, read, drained, or
    /// closed here, no signal mask or disposition is touched, and what made it
    /// readable is never interpreted. `None` is exactly `connect_before`.
    /// `POLLIN` is a wake candidate; `POLLERR`/`POLLHUP`/`POLLNVAL` report a
    /// broken wake channel. Both surface as `CancellationReady` or
    /// `CancellationSourceFault` — provisional facts, not a cancellation
    /// verdict; the caller's own signal owner performs the sole arbitration
    /// read and decides the outcome.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub fn connect_before_cancellable(
        endpoint: &Path,
        cohort: &str,
        deadline: std::time::Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Self, ConnectClientError> {
        let hello = ClientHello::new(cohort)?;
        admission_deadline::connect_before_cancellable(endpoint, hello, deadline, wake)
    }

    #[cfg(unix)]
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
            LocalClientMessage::ClientHello(_)
            | LocalClientMessage::SessionStopRequest(_)
            | LocalClientMessage::SessionStopResult(_)
            | LocalClientMessage::ServerMetadataRequest
            | LocalClientMessage::ServerMetadata(_) => {
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

    /// Releases the admitted stream for a caller that relays its bytes unchanged.
    #[must_use]
    pub fn into_stream(self) -> UnixStream {
        self.stream
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

#[cfg(unix)]
impl AsFd for ClientConnection {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

impl Read for ClientConnection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buffer)
    }
}

impl Write for ClientConnection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[derive(Debug)]
pub enum ConnectClientError {
    /// Linux deadline adapter failed before any OBLC exchange began.
    ConnectFailed(io::Error),
    Io(io::Error),
    Protocol(ProtocolError),
    Rejected(RejectionReason),
    UnexpectedResponse,
    /// The borrowed wake descriptor became readable during admission — a
    /// provisional wake fact, not a cancellation verdict. The descriptor was
    /// never read here; the caller's own signal owner performs the sole
    /// arbitration read and decides the outcome.
    CancellationReady,
    /// The borrowed wake descriptor reported `POLLERR`, `POLLHUP`, or
    /// `POLLNVAL` — the wake channel itself is broken. The caller decides
    /// whether this counts toward its cancellation outcome.
    CancellationSourceFault(io::Error),
}

impl fmt::Display for ConnectClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectFailed(error) | Self::Io(error) | Self::CancellationSourceFault(error) => {
                error.fmt(formatter)
            }
            Self::Protocol(error) => error.fmt(formatter),
            Self::Rejected(reason) => write!(formatter, "local Client was rejected: {reason:?}"),
            Self::UnexpectedResponse => {
                formatter.write_str("local Client received an unexpected handshake response")
            }
            Self::CancellationReady => {
                formatter.write_str("local Client admission observed a cancellation wake")
            }
        }
    }
}

impl std::error::Error for ConnectClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectFailed(error) | Self::Io(error) | Self::CancellationSourceFault(error) => {
                Some(error)
            }
            Self::Protocol(error) => Some(error),
            Self::Rejected(_) | Self::UnexpectedResponse | Self::CancellationReady => None,
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

#[cfg(windows)]
mod windows_admission;
#[cfg(windows)]
pub const UNIX_SOCKET_PATH_CAPACITY: usize = 200;
#[cfg(windows)]
pub fn unix_socket_path_fits(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|path| path.starts_with(r"\\.\pipe\") && path.len() <= 209)
}
#[cfg(windows)]
impl ClientConnection {
    pub fn connect(endpoint: &Path, cohort: &str) -> Result<Self, ConnectClientError> {
        Self::connect_before(
            endpoint,
            cohort,
            std::time::Instant::now() + HANDSHAKE_TIMEOUT,
        )
    }
    pub fn connect_before(
        endpoint: &Path,
        cohort: &str,
        deadline: std::time::Instant,
    ) -> Result<Self, ConnectClientError> {
        Self::connect_before_cancellable(endpoint, cohort, deadline, None)
    }
    pub fn connect_before_cancellable(
        endpoint: &Path,
        cohort: &str,
        deadline: std::time::Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<Self, ConnectClientError> {
        windows_admission::connect(endpoint, ClientHello::new(cohort)?, deadline, wake)
    }
    pub fn shutdown_stream(&self) -> io::Result<UnixStream> {
        self.stream.try_clone()
    }
}
#[cfg(windows)]
impl AsLocalIo for ClientConnection {
    fn as_local_io(&self) -> LocalIo<'_> {
        self.stream.as_local_io()
    }
}
