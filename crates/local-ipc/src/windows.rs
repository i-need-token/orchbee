//! Local-only named pipes with same-token-user admission and bounded nonblocking I/O.
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::windows::io::AsHandle,
    path::Path,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use interprocess::{
    ConnectWaitMode,
    os::windows::{
        named_pipe::{PipeListener, PipeListenerOptions, PipeStream, pipe_mode::Bytes},
        security_descriptor::SecurityDescriptor,
    },
    unnamed_pipe::Recver,
};
use widestring::U16CString;
use winsafe::{HPROCESS, TokenInfo, co};

const POLL_INTERVAL: Duration = Duration::from_millis(2);
type Pipe = PipeStream<Bytes, Bytes>;

mod connection;
pub use connection::ConnectAttemptError;

/// The kernel token user's SID, never an environment-variable identity.
pub fn current_user_sid() -> io::Result<String> {
    process_user(&HPROCESS::GetCurrentProcess())
}

fn process_user(process: &HPROCESS) -> io::Result<String> {
    let token = process
        .OpenProcessToken(co::TOKEN::QUERY)
        .map_err(io::Error::other)?;
    let info = token
        .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::User)
        .map_err(io::Error::other)?;
    let TokenInfo::User(user) = info else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "token user missing",
        ));
    };
    let sid = user
        .User
        .Sid()
        .ok_or_else(|| io::Error::other("token SID missing"))?;
    winsafe::ConvertSidToStringSid(sid).map_err(io::Error::other)
}

fn verify_peer(pipe: &Pipe) -> io::Result<()> {
    let process = HPROCESS::OpenProcess(
        co::PROCESS::QUERY_LIMITED_INFORMATION,
        false,
        pipe.peer_process_id()?,
    )
    .map_err(io::Error::other)?;
    if process_user(&process)? != current_user_sid()? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "local peer user differs",
        ));
    }
    Ok(())
}

fn pipe_name(path: &Path) -> io::Result<&std::ffi::OsStr> {
    let text = path
        .to_str()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let suffix = text
        .strip_prefix(r"\\.\pipe\")
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    if suffix.is_empty()
        || suffix.len() > 200
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(path.as_os_str())
}

/// Advisory byte-read readiness; probing never discards the prefetched byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Readiness {
    Pending,
    Readable,
    Closed,
}

#[derive(Debug)]
struct Connection {
    pipe: Option<Pipe>,
    reader: Option<Recver>,
    prefetched: Option<u8>,
    eof: bool,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    write_blocked_at: Option<Instant>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(pipe) = self.pipe.take() {
            pipe.evade_limbo();
        }
    }
}

/// Clones share one connection and cancellation owner, not independent byte cursors.
#[derive(Clone, Debug)]
pub struct LocalStream {
    connection: Arc<Mutex<Connection>>,
    nonblocking: Arc<AtomicBool>,
}

impl LocalStream {
    /// Revalidates the kernel token identity of the admitted peer.
    pub fn require_same_user(&self) -> io::Result<()> {
        verify_peer(
            self.lock()?
                .pipe
                .as_ref()
                .ok_or(io::ErrorKind::BrokenPipe)?,
        )
    }

    /// Creates one private connected pair, including both admission checks.
    pub fn pair() -> io::Result<(Self, Self)> {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = format!(
            r"\\.\pipe\orchbee-pair-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let listener = LocalListener::bind(&path)?;
        let client = Self::connect(&path)?;
        let (server, ()) = listener.accept()?;
        Ok((client, server))
    }
    /// Opens one local pipe with a bounded admission wait and proves its peer before I/O.
    pub fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::connect_before(path.as_ref(), Instant::now() + Duration::from_secs(2))
    }

    /// Connects within the caller's absolute deadline; no application request is sent.
    pub fn connect_before(path: &Path, deadline: Instant) -> io::Result<Self> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;
        let pipe = Pipe::connect_by_path_with_wait_mode(
            pipe_name(path)?,
            ConnectWaitMode::Timeout(remaining),
        )?;
        let stream = Self::admit(pipe)?;
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(stream)
    }

    fn admit(pipe: Pipe) -> io::Result<Self> {
        pipe.set_nonblocking(true)?;
        verify_peer(&pipe)?;
        Self::from_nonblocking_pipe(pipe)
    }

    fn from_nonblocking_pipe(pipe: Pipe) -> io::Result<Self> {
        // Preserve ERROR_NO_DATA: the high-level pipe Read implementation
        // folds it into EOF, although PIPE_NOWAIT uses it for an idle peer.
        let reader = Recver::from(pipe.as_handle().try_clone_to_owned()?);
        Ok(Self {
            connection: Arc::new(Mutex::new(Connection {
                pipe: Some(pipe),
                reader: Some(reader),
                prefetched: None,
                eof: false,
                read_timeout: None,
                write_timeout: None,
                write_blocked_at: None,
            })),
            nonblocking: Arc::new(AtomicBool::new(false)),
        })
    }

    fn lock(&self) -> io::Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| io::Error::other("local transport poisoned"))
    }

    /// Creates another cancellation/read handle to this same byte stream.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(self.clone())
    }

    /// Selects immediate would-block versus bounded polling, without blocking kernel I/O.
    pub fn set_nonblocking(&self, value: bool) -> io::Result<()> {
        self.nonblocking.store(value, Ordering::Release);
        Ok(())
    }

    /// Sets the per-call read budget; protocol deadlines remain the caller's responsibility.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        reject_zero(timeout)?;
        self.lock()?.read_timeout = timeout;
        Ok(())
    }

    /// Sets the per-call write budget; partial writes never renew a protocol deadline.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        reject_zero(timeout)?;
        self.lock()?.write_timeout = timeout;
        Ok(())
    }

    /// Cancels both directions for every clone without an unbounded flush or worker thread.
    pub fn shutdown(&self, _how: Shutdown) -> io::Result<()> {
        let mut connection = self.lock()?;
        connection.eof = true;
        connection.prefetched = None;
        connection.reader.take();
        if let Some(pipe) = connection.pipe.take() {
            pipe.evade_limbo();
        }
        Ok(())
    }

    /// Probes exact byte readiness, retaining at most one byte until the next read.
    pub fn readiness(&self) -> io::Result<Readiness> {
        let mut connection = self.lock()?;
        if connection.prefetched.is_some() {
            return Ok(Readiness::Readable);
        }
        if connection.eof {
            return Ok(Readiness::Closed);
        }
        let mut byte = [0];
        let result = match &mut connection.reader {
            Some(reader) => reader.read(&mut byte),
            None => return Ok(Readiness::Closed),
        };
        match normalize_read(result) {
            Ok(0) => {
                connection.eof = true;
                Ok(Readiness::Closed)
            }
            Ok(_) => {
                connection.prefetched = Some(byte[0]);
                Ok(Readiness::Readable)
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(Readiness::Pending),
            Err(error) => Err(error),
        }
    }

    fn read_once(&self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut connection = self.lock()?;
        let prefetched = if let Some(byte) = connection.prefetched.take() {
            buffer[0] = byte;
            1
        } else {
            0
        };
        if connection.eof || prefetched == buffer.len() {
            return Ok(prefetched);
        }
        let result = match &mut connection.reader {
            Some(reader) => normalize_read(reader.read(&mut buffer[prefetched..])),
            None => Ok(0),
        };
        if matches!(result, Ok(0)) {
            connection.eof = true;
        }
        match result {
            Ok(read) => Ok(prefetched + read),
            Err(_) if prefetched > 0 => Ok(prefetched),
            Err(error) => Err(error),
        }
    }

    fn write_once(&self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut connection = self.lock()?;
        let pipe = connection
            .pipe
            .as_ref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        // PIPE_NOWAIT may accept zero bytes when one write exceeds the
        // available quota. Bound each attempt below our pipe buffer size so
        // large protocol records make progress as the peer drains them.
        let result = (&*pipe).write(&buffer[..buffer.len().min(4096)]);
        // Protocol receipts, never FlushFileBuffers, decide known delivery.
        pipe.assume_flushed();
        let result = match result {
            Ok(0) => Err(io::ErrorKind::WouldBlock.into()),
            other => other,
        };
        connection.write_blocked_at = result
            .as_ref()
            .err()
            .filter(|error| error.kind() == io::ErrorKind::WouldBlock)
            .map(|_| Instant::now());
        result
    }

    pub(crate) fn writable_advisory(&self) -> io::Result<bool> {
        let connection = self.lock()?;
        Ok(!connection.eof
            && connection
                .write_blocked_at
                .is_none_or(|at| at.elapsed() >= POLL_INTERVAL))
    }

    fn transfer(
        &self,
        timeout: Option<Duration>,
        mut attempt: impl FnMut() -> io::Result<usize>,
    ) -> io::Result<usize> {
        let deadline = timeout
            .map(|budget| {
                Instant::now()
                    .checked_add(budget)
                    .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))
            })
            .transpose()?;
        loop {
            match attempt() {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if self.nonblocking.load(Ordering::Acquire) {
                        return Err(error);
                    }
                    if deadline.is_some_and(|end| Instant::now() >= end) {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    thread::sleep(deadline.map_or(POLL_INTERVAL, |end| {
                        end.saturating_duration_since(Instant::now())
                            .min(POLL_INTERVAL)
                    }));
                }
                result => return result,
            }
        }
    }
}

/// A nonblocking reader for a child process's owned stdout or stderr pipe.
/// It grants no duplex or peer-authenticated protocol capability.
#[derive(Debug)]
pub struct ChildOutputReader(LocalStream);

impl ChildOutputReader {
    /// Creates an owned pipe for Command::stdout/stderr. The child writer is
    /// blocking; only the parent's reader is nonblocking, so output is not lost
    /// when the child fills its buffer.
    pub fn pipe() -> io::Result<(Self, std::process::Stdio)> {
        let (writer, reader) = LocalStream::pair()?;
        reader.set_nonblocking(true)?;
        let mut connection = writer.lock()?;
        connection.reader.take();
        let pipe = connection.pipe.take().ok_or(io::ErrorKind::BrokenPipe)?;
        pipe.set_nonblocking(false)?;
        let handle = std::os::windows::io::OwnedHandle::try_from(pipe)
            .map_err(|_| io::Error::other("child pipe ownership unavailable"))?;
        Ok((Self(reader), std::process::Stdio::from(handle)))
    }
}

impl Read for ChildOutputReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer)
    }
}

impl crate::AsLocalIo for ChildOutputReader {
    fn as_local_io(&self) -> crate::LocalIo<'_> {
        crate::LocalIo::Stream(&self.0)
    }
}

/// Nonblocking parent writer paired with a blocking child stdin. No flush thread is created.
pub struct ChildInputWriter(LocalStream);

impl ChildInputWriter {
    pub fn pipe() -> io::Result<(Self, std::process::Stdio)> {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = format!(
            r"\\.\pipe\orchbee-child-input-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let listener = LocalListener::bind(&path)?;
        // Standard child readers use synchronous ReadFile. A LocalStream handle is
        // overlapped even in PIPE_WAIT mode: inheriting it can strand the next read.
        let reader = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)?;
        let (writer, ()) = listener.accept()?;
        writer.set_nonblocking(true)?;
        Ok((Self(writer), std::process::Stdio::from(reader)))
    }
}
impl Write for ChildInputWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

fn reject_zero(timeout: Option<Duration>) -> io::Result<()> {
    if timeout.is_some_and(|value| value.is_zero()) {
        Err(io::ErrorKind::InvalidInput.into())
    } else {
        Ok(())
    }
}

fn normalize_read(result: io::Result<usize>) -> io::Result<usize> {
    match result {
        Err(error) if matches!(error.raw_os_error(), Some(109 | 233)) => Ok(0),
        Err(error) if error.raw_os_error() == Some(232) => Err(io::ErrorKind::WouldBlock.into()),
        result => result,
    }
}

impl Read for &LocalStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let timeout = self.lock()?.read_timeout;
        self.transfer(timeout, || self.read_once(buffer))
    }
}
impl Read for LocalStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buffer)
    }
}
impl Write for &LocalStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let timeout = self.lock()?.write_timeout;
        self.transfer(timeout, || self.write_once(buffer))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Write for LocalStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        (&*self).write(buffer)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A first-instance, local-only listener protected by the current token user's DACL.
pub struct LocalListener {
    pipe: PipeListener<Bytes, Bytes>,
    nonblocking: AtomicBool,
    pending: Mutex<Option<LocalStream>>,
}

impl LocalListener {
    /// Reserves a fresh named pipe; an existing listener is never adopted or replaced.
    pub fn bind(path: impl AsRef<Path>) -> io::Result<Self> {
        let descriptor = U16CString::from_str(format!("D:P(A;;GA;;;{})", current_user_sid()?))
            .map_err(io::Error::other)?;
        let pipe = PipeListenerOptions::new()
            .path(pipe_name(path.as_ref())?)
            .nonblocking(true)
            .accept_remote(false)
            .inheritable(false)
            .input_buffer_size_hint(64 * 1024)
            .output_buffer_size_hint(64 * 1024)
            .security_descriptor(Some(SecurityDescriptor::deserialize(&descriptor)?))
            .create_duplex::<Bytes>()?;
        Ok(Self {
            pipe,
            nonblocking: AtomicBool::new(false),
            pending: Mutex::new(None),
        })
    }

    /// Selects immediate accept or bounded-cadence waiting; kernel accepts stay nonblocking.
    pub fn set_nonblocking(&self, value: bool) -> io::Result<()> {
        self.nonblocking.store(value, Ordering::Release);
        Ok(())
    }

    /// Accepts and verifies one peer before exposing its stream to a protocol owner.
    pub fn accept(&self) -> io::Result<(LocalStream, ())> {
        if let Some(stream) = self
            .pending
            .lock()
            .map_err(|_| io::Error::other("listener poisoned"))?
            .take()
        {
            return Ok((stream, ()));
        }
        loop {
            match self.pipe.accept() {
                Ok(pipe) => return LocalStream::admit(pipe).map(|stream| (stream, ())),
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        && !self.nonblocking.load(Ordering::Acquire) =>
                {
                    thread::sleep(POLL_INTERVAL)
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn readiness(&self) -> io::Result<Readiness> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| io::Error::other("listener poisoned"))?;
        if pending.is_some() {
            return Ok(Readiness::Readable);
        }
        match self.pipe.accept() {
            Ok(pipe) => {
                *pending = Some(LocalStream::admit(pipe)?);
                Ok(Readiness::Readable)
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(Readiness::Pending),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests;

/// Observes the local pipe namespace without opening or authenticating a peer.
/// Presence is advisory; connect/admit is the only readiness authority.
pub fn endpoint_exists(path: &Path) -> io::Result<bool> {
    let name = pipe_name(path)?
        .to_str()
        .ok_or(io::ErrorKind::InvalidInput)?;
    // Enumerating every pipe can skip a retained listener when unrelated names
    // change between batches. Validation excludes wildcards from this exact query.
    // The extended prefix preserves literal names, including trailing periods.
    let query = name.replacen(r"\\.\", r"\\?\", 1);
    crate::windows_name_query::exact_name_exists(&query)
}

/// Read-only name observation; `None` never claims absence after a budget limit.
pub fn endpoint_exists_before(path: &Path, deadline: Instant) -> io::Result<Option<bool>> {
    pipe_name(path)?;
    if Instant::now() >= deadline {
        return Ok(None);
    }
    let found = endpoint_exists(path)?;
    if Instant::now() >= deadline {
        return Ok(None);
    }
    Ok(Some(found))
}
