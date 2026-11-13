//! Local-only named pipes with same-token-user admission and bounded nonblocking I/O. Reads and
//! writes consult the kernel's pipe counters first, so a blocking-mode pipe never blocks a caller;
//! that mode is what lets one zero-byte read wait for the peer's next byte.
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::windows::io::{AsHandle, OwnedHandle},
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
};
use widestring::U16CString;
use winsafe::{HPROCESS, TokenInfo, co};

use crate::windows_native::{ReadWatch, WatchArm, pipe_counters};

const POLL_INTERVAL: Duration = Duration::from_millis(2);
const UNBOUNDED_WAIT_SLICE: Duration = Duration::from_secs(1);
const NONBLOCKING_WRITE_LIMIT: usize = 4096;
const ERROR_PIPE_BUSY: i32 = 231;
type Pipe = PipeStream<Bytes, Bytes>;

mod connection;
pub use connection::ConnectAttemptError;

/// The kernel token user's SID, never an environment-variable identity.
pub fn current_user_sid() -> io::Result<String> {
    process_user(&HPROCESS::GetCurrentProcess())
}

fn process_user(process: &HPROCESS) -> io::Result<String> {
    let token = process.OpenProcessToken(co::TOKEN::QUERY).map_err(system)?;
    let info = token
        .GetTokenInformation(co::TOKEN_INFORMATION_CLASS::User)
        .map_err(system)?;
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
    winsafe::ConvertSidToStringSid(sid).map_err(system)
}

/// A failed system call as the I/O error of its code, so a refusal keeps its kind and code.
fn system(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}

fn verify_peer(pipe: &Pipe) -> io::Result<()> {
    let process = HPROCESS::OpenProcess(
        co::PROCESS::QUERY_LIMITED_INFORMATION,
        false,
        pipe.peer_process_id()?,
    )
    .map_err(system)?;
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

/// Advisory byte-read readiness; probing never consumes a byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Readiness {
    Pending,
    Readable,
    Closed,
}

#[derive(Debug)]
struct Connection {
    pipe: Option<Pipe>,
    watch: Option<ReadWatch>,
    eof: bool,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    write_blocked_at: Option<Instant>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}

impl Connection {
    fn close(&mut self) {
        self.eof = true;
        // Settle the watch first: it holds the pipe open and its cancellation wakes every poller.
        self.watch.take();
        if let Some(pipe) = self.pipe.take() {
            pipe.evade_limbo();
        }
    }

    fn live_pipe(&self) -> Option<&Pipe> {
        self.pipe.as_ref().filter(|_| !self.eof)
    }
}

/// What one poller can wait on for this stream's next byte or closure.
pub(crate) enum ReadWait {
    /// A pending zero-byte read signals this event.
    Event(Arc<OwnedHandle>),
    /// The stream changed while arming, so probing again is the next step.
    Changed,
    /// Bytes are already waiting and a zero-byte read would complete at once. A poller reaches
    /// this only without read interest, so it keeps the bounded cadence to see closure.
    Unwatchable,
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
        // A zero-byte read pends only in blocking mode, and the kernel refuses to leave that mode
        // while one is outstanding. Counted transfers below never wait in it.
        pipe.set_nonblocking(false)?;
        verify_peer(&pipe)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(Connection {
                pipe: Some(pipe),
                watch: None,
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
        self.lock()?.close();
        Ok(())
    }

    /// Probes exact byte readiness from the kernel's counters without consuming a byte.
    pub fn readiness(&self) -> io::Result<Readiness> {
        let mut connection = self.lock()?;
        let Some(pipe) = connection.live_pipe() else {
            return Ok(Readiness::Closed);
        };
        let counters = pipe_counters(pipe.as_handle())?;
        if counters.read_available > 0 {
            return Ok(Readiness::Readable);
        }
        if counters.connected {
            return Ok(Readiness::Pending);
        }
        connection.eof = true;
        Ok(Readiness::Closed)
    }

    /// Arms this stream's next-byte-or-closure notification for one poller.
    pub(crate) fn read_wait(&self) -> io::Result<ReadWait> {
        let mut connection = self.lock()?;
        let Connection {
            pipe, watch, eof, ..
        } = &mut *connection;
        let Some(pipe) = pipe.as_ref().filter(|_| !*eof) else {
            return Ok(ReadWait::Changed);
        };
        if pipe_counters(pipe.as_handle())?.read_available > 0 {
            return Ok(ReadWait::Unwatchable);
        }
        let watch = match watch {
            Some(watch) => watch,
            empty => empty.insert(ReadWatch::new(pipe.as_handle())?),
        };
        Ok(match watch.arm()? {
            WatchArm::Pending => ReadWait::Event(watch.event()),
            WatchArm::Completed => ReadWait::Changed,
        })
    }

    fn read_once(&self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut connection = self.lock()?;
        let Some(pipe) = connection.live_pipe() else {
            return Ok(0);
        };
        let counters = pipe_counters(pipe.as_handle())?;
        if counters.read_available == 0 {
            if counters.connected {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            connection.eof = true;
            return Ok(0);
        }
        // Bytes already counted complete this read at once; only the lock owner consumes them.
        let length = buffer.len().min(counters.read_available as usize);
        let read = (&*pipe).read(&mut buffer[..length])?;
        if read == 0 {
            connection.eof = true;
        }
        Ok(read)
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
        let counters = pipe_counters(pipe.as_handle())?;
        if !counters.connected {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let result = if counters.write_quota > 0 {
            // Only the lock owner writes and the peer only frees quota, so a write within the
            // counted quota lands in the pipe's buffer without waiting for the peer.
            let length = buffer.len().min(counters.write_quota as usize);
            (&*pipe).write(&buffer[..length])
        } else {
            // The count alone cannot refuse a write: a peer's pending read of a large buffer
            // reduces it while the pipe is empty. Leaving blocking mode tells the two apart: the
            // kernel refuses it only for this end's pending read, which is cancelled here, or for
            // queued bytes, which prove the count right.
            connection.watch.take();
            let pipe = connection
                .pipe
                .as_ref()
                .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
            match pipe.set_nonblocking(true) {
                Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                    Err(io::ErrorKind::WouldBlock.into())
                }
                Err(error) => Err(error),
                Ok(()) => {
                    // PIPE_NOWAIT may accept zero bytes when one write exceeds the available quota,
                    // so this stays below the buffer size for a large record to keep progressing.
                    let attempt =
                        (&*pipe).write(&buffer[..buffer.len().min(NONBLOCKING_WRITE_LIMIT)]);
                    pipe.set_nonblocking(false)?;
                    match attempt {
                        Ok(0) => Err(io::ErrorKind::WouldBlock.into()),
                        other => other,
                    }
                }
            }
        };
        let pipe = connection
            .pipe
            .as_ref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        // Protocol receipts, never FlushFileBuffers, decide known delivery.
        pipe.assume_flushed();
        connection.write_blocked_at = result
            .as_ref()
            .err()
            .filter(|error| error.kind() == io::ErrorKind::WouldBlock)
            .map(|_| Instant::now());
        result
    }

    /// A pipe has no write-completion event, so a blocked writer keeps a bounded cadence.
    pub(crate) fn writable_advisory(&self) -> io::Result<bool> {
        let connection = self.lock()?;
        let Some(pipe) = connection.live_pipe() else {
            return Ok(false);
        };
        Ok(pipe_counters(pipe.as_handle())?.write_quota > 0
            || connection
                .write_blocked_at
                .is_none_or(|at| at.elapsed() >= POLL_INTERVAL))
    }

    fn transfer(
        &self,
        timeout: Option<Duration>,
        interest: crate::PollFlags,
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
                    // An unbounded caller reobserves its flag at this slice; a deadline never
                    // exceeds the poll grammar's range.
                    let slice = deadline.map_or(UNBOUNDED_WAIT_SLICE, |end| {
                        end.saturating_duration_since(Instant::now())
                            .min(UNBOUNDED_WAIT_SLICE)
                    });
                    crate::poll(
                        &mut [crate::PollFd::new(crate::LocalIo::Stream(self), interest)],
                        crate::PollTimeout::try_from(slice)?,
                    )?;
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
        let pipe = connection.pipe.take().ok_or(io::ErrorKind::BrokenPipe)?;
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

impl Read for &LocalStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let timeout = self.lock()?.read_timeout;
        self.transfer(timeout, crate::PollFlags::POLLIN, || self.read_once(buffer))
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
        self.transfer(timeout, crate::PollFlags::POLLOUT, || {
            self.write_once(buffer)
        })
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
    crate::windows_native::exact_name_exists(&query)
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
