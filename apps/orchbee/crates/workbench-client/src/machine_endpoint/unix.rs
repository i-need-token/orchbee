//! macOS and Linux machine endpoints: Unix sockets in this environment's private socket folder,
//! `/private/tmp/<stem>-<uid>` on macOS. A socket file outlives its listener, so a refused connect
//! reads as no endpoint, a listener removes its own file when it closes, and only a caller whose
//! lock proves no other listener serves a name removes a file left there. Every accepted and
//! connected stream proves its peer's effective user before a byte is read, and an accepted stream
//! blocks whatever its listener does.
use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_runtime::{ModeCheckedLocalSessionRendezvous, ModeCheckedSessionLock};
use std::{
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::{
        fd::AsFd,
        unix::{
            fs::{FileTypeExt, MetadataExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

/// Every socket file ends so.
pub(super) const SUFFIX: &str = ".sock";

pub(super) fn prefix(role: &str) -> String {
    format!(
        "{}/{role}-",
        ModeCheckedLocalSessionRendezvous::socket_root_path().display()
    )
}

/// The shared hold on the socket folder that keeps a cleaner that honours locks away from it.
pub struct FolderHold {
    _lock: ModeCheckedSessionLock,
}

/// Creates the socket folder when it is missing, admits its owner and mode, and holds it.
///
/// # Errors
///
/// A folder that is not this user's private folder, or that cannot be created or locked.
pub fn hold_folder() -> io::Result<FolderHold> {
    ModeCheckedLocalSessionRendezvous::hold_prepared_socket_root()
        .map(|lock| FolderHold { _lock: lock })
        .map_err(|error| match error {
            orchbee_local_runtime::RendezvousError::System { source, .. } => source,
            other => io::Error::other(other),
        })
}

/// Advisory byte-read readiness; probing never consumes a byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Readiness {
    /// A byte waits.
    Readable,
    /// Nothing waits and the peer has not closed.
    Pending,
    /// The peer closed and nothing is left to read.
    Closed,
}

/// A connected socket whose peer proved it runs as this user; clones share the one socket, as the
/// Windows pipe adapter's do.
#[derive(Clone, Debug)]
pub struct LocalStream(Arc<UnixStream>);

impl LocalStream {
    fn admit(stream: UnixStream) -> io::Result<Self> {
        require_same_effective_user(&stream)?;
        Ok(Self(Arc::new(stream)))
    }

    /// Connects before `deadline`. A missing socket file, and one no listener holds, are both
    /// `NotFound`; a listener of another user is `PermissionDenied`.
    ///
    /// # Errors
    ///
    /// The connect's failure, or the peer proof's.
    pub fn connect_before(path: &Path, deadline: Instant) -> io::Result<Self> {
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let stream = UnixStream::connect(path).map_err(|error| {
            if error.kind() == io::ErrorKind::ConnectionRefused {
                io::Error::from(io::ErrorKind::NotFound)
            } else {
                error
            }
        })?;
        Self::admit(stream)
    }

    /// Proves the peer again.
    ///
    /// # Errors
    ///
    /// `PermissionDenied` for another user's peer.
    pub fn require_same_user(&self) -> io::Result<()> {
        require_same_effective_user(&self.0)
    }

    /// Another handle to this same socket.
    ///
    /// # Errors
    ///
    /// Never; the result matches the Windows adapter's.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(self.clone())
    }

    /// # Errors
    ///
    /// The socket option's failure.
    pub fn set_nonblocking(&self, value: bool) -> io::Result<()> {
        self.0.set_nonblocking(value)
    }

    /// # Errors
    ///
    /// The socket option's failure; a zero duration is refused.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(timeout)
    }

    /// # Errors
    ///
    /// The socket option's failure; a zero duration is refused.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(timeout)
    }

    /// # Errors
    ///
    /// The shutdown's failure, such as on a socket whose peer already went.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }

    /// Whether a byte waits, nothing does yet, or the peer closed, without consuming a byte.
    ///
    /// # Errors
    ///
    /// The poll's or the byte count's failure.
    pub fn readiness(&self) -> io::Result<Readiness> {
        let mut sources = [PollFd::new(self.0.as_fd(), PollFlags::POLLIN)];
        if poll(&mut sources, PollTimeout::ZERO)? == 0 {
            return Ok(Readiness::Pending);
        }
        // A readable socket with no byte waiting has reached its end.
        Ok(if rustix::io::ioctl_fionread(&*self.0)? > 0 {
            Readiness::Readable
        } else {
            Readiness::Closed
        })
    }
}

impl Read for &LocalStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buffer)
    }
}

impl Write for &LocalStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        (&*self.0).write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

impl Read for LocalStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buffer)
    }
}

impl Write for LocalStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        (&*self.0).write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

/// A listening socket that removes its own file when it closes, unless another listener's file
/// replaced it.
#[derive(Debug)]
pub struct LocalListener {
    listener: UnixListener,
    path: PathBuf,
    /// The device and inode of the socket file this listener created.
    file: (u64, u64),
}

impl LocalListener {
    /// Binds `path`; an existing file there refuses with `AddrInUse`, so no listener is replaced.
    ///
    /// # Errors
    ///
    /// The bind's failure.
    pub fn bind(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let listener = UnixListener::bind(path)?;
        let metadata = fs::symlink_metadata(path)?;
        Ok(Self {
            listener,
            path: path.to_owned(),
            file: (metadata.dev(), metadata.ino()),
        })
    }

    /// # Errors
    ///
    /// The socket option's failure.
    pub fn set_nonblocking(&self, value: bool) -> io::Result<()> {
        self.listener.set_nonblocking(value)
    }

    /// Accepts one connection and proves its peer; a peer of another user is dropped unread.
    ///
    /// # Errors
    ///
    /// `WouldBlock` with nothing waiting on a nonblocking listener, the accept's failure, or the
    /// peer proof's.
    pub fn accept(&self) -> io::Result<(LocalStream, ())> {
        let (stream, _address) = self.listener.accept()?;
        // macOS hands an accepted socket its listener's nonblocking flag.
        stream.set_nonblocking(false)?;
        LocalStream::admit(stream).map(|stream| (stream, ()))
    }
}

impl Drop for LocalListener {
    fn drop(&mut self) {
        let ours = fs::symlink_metadata(&self.path)
            .is_ok_and(|metadata| (metadata.dev(), metadata.ino()) == self.file);
        if ours {
            let _removed = fs::remove_file(&self.path);
        }
    }
}

/// Whether a listener serves `path` now, by connecting and closing at once: a socket file no
/// listener holds is no endpoint.
///
/// # Errors
///
/// A connect failure other than a missing or refused endpoint.
pub fn endpoint_exists(path: &Path) -> io::Result<bool> {
    match UnixStream::connect(path) {
        Ok(_stream) => Ok(true),
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => Ok(false),
            io::ErrorKind::PermissionDenied => Ok(true),
            _ => Err(error),
        },
    }
}

/// Binds `path` for a caller whose lock proves no other listener of it serves, first removing a
/// socket file a listener that ended without closing left there.
///
/// # Errors
///
/// A file there that is not a socket, or the bind's failure.
pub fn bind_locked(path: &Path) -> io::Result<LocalListener> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => fs::remove_file(path)?,
        Ok(_) => return Err(io::ErrorKind::AlreadyExists.into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    LocalListener::bind(path)
}
