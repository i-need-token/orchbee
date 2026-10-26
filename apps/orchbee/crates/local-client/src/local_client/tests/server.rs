use orchbee_local_ipc::{LocalListener as UnixListener, LocalStream as UnixStream};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
const SERVER_GUARD_TIMEOUT: Duration = Duration::from_secs(1);
/// Per-admission accept bound: a missing client must fail `finish` with a
/// clear `TimedOut` instead of leaving an unbounded blocking `accept` that
/// `join` waits on forever.
const ACCEPT_GUARD_TIMEOUT: Duration = Duration::from_secs(3);
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);
pub(crate) struct FakeServer {
    _directory: TestDirectory,
    endpoint: PathBuf,
    thread: thread::JoinHandle<io::Result<()>>,
    finish_signal: Option<std::sync::mpsc::SyncSender<()>>,
}

/// Poll a nonblocking listener until a client arrives or `deadline` passes.
/// `TimedOut` is returned explicitly so a caller can tell "nobody came" from
/// a socket error; `Interrupted` retries, anything else propagates.
fn accept_bounded(listener: &UnixListener, deadline: Instant) -> io::Result<UnixStream> {
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "fake Server accept deadline",
                    ));
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error),
        }
    }
}

impl FakeServer {
    pub(crate) fn start_connections(
        count: usize,
        mut handler: impl FnMut(usize, UnixStream) -> io::Result<()> + Send + 'static,
    ) -> io::Result<Self> {
        let directory = TestDirectory::new()?;
        #[cfg(unix)]
        let endpoint = directory.path.join("client.sock");
        #[cfg(windows)]
        let endpoint = crate::tests::windows_endpoints::endpoint();
        let listener = UnixListener::bind(&endpoint)?;
        listener.set_nonblocking(true)?;
        let thread = thread::spawn(move || {
            for index in 0..count {
                let stream = accept_bounded(&listener, Instant::now() + ACCEPT_GUARD_TIMEOUT)?;
                stream.set_read_timeout(Some(SERVER_GUARD_TIMEOUT))?;
                stream.set_write_timeout(Some(SERVER_GUARD_TIMEOUT))?;
                handler(index, stream)?;
            }
            Ok(())
        });
        Ok(Self {
            _directory: directory,
            endpoint,
            thread,
            finish_signal: None,
        })
    }

    /// Keep the peer alive until its consumers finish; canceled observations
    /// may omit admissions, but the maximum and every read remain bounded.
    pub(crate) fn start_optional_connections(
        maximum: usize,
        handler: impl Fn(usize, UnixStream) -> io::Result<()> + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let directory = TestDirectory::new()?;
        #[cfg(unix)]
        let endpoint = directory.path.join("client.sock");
        #[cfg(windows)]
        let endpoint = crate::tests::windows_endpoints::endpoint();
        let listener = UnixListener::bind(&endpoint)?;
        listener.set_nonblocking(true)?;
        let (signal, finish) = std::sync::mpsc::sync_channel(1);
        let handler = Arc::new(handler);
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + ACCEPT_GUARD_TIMEOUT;
            let mut admitted = 0;
            let mut lanes = Vec::with_capacity(maximum);
            let mut first_error = None;
            loop {
                match finish.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
                if Instant::now() >= deadline {
                    first_error = Some(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "fake Server finish deadline",
                    ));
                    break;
                }
                if admitted < maximum {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if let Err(error) = stream
                                .set_read_timeout(Some(SERVER_GUARD_TIMEOUT))
                                .and_then(|()| stream.set_write_timeout(Some(SERVER_GUARD_TIMEOUT)))
                            {
                                first_error = Some(error);
                                break;
                            }
                            let index = admitted;
                            let lane_handler = Arc::clone(&handler);
                            let lane = match thread::Builder::new()
                                .name(format!("orchbee-fake-server-{index}"))
                                .spawn(move || lane_handler(index, stream))
                            {
                                Ok(lane) => lane,
                                Err(error) => {
                                    first_error = Some(error);
                                    break;
                                }
                            };
                            lanes.push(lane);
                            admitted += 1;
                            continue;
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                            ) => {}
                        Err(error) => {
                            first_error = Some(error);
                            break;
                        }
                    }
                }
                thread::sleep(Duration::from_millis(1));
            }
            for lane in lanes {
                let result = lane
                    .join()
                    .map_err(|_panic| io::Error::other("fake Server handler panicked"))
                    .and_then(std::convert::identity);
                if let Err(error) = result
                    && first_error.is_none()
                {
                    first_error = Some(error);
                }
            }
            first_error.map_or(Ok(()), Err)
        });
        Ok(Self {
            _directory: directory,
            endpoint,
            thread,
            finish_signal: Some(signal),
        })
    }

    pub(crate) fn start(
        handler: impl FnOnce(UnixStream) -> io::Result<()> + Send + 'static,
    ) -> io::Result<Self> {
        Self::start_with_accept_deadline(handler, ACCEPT_GUARD_TIMEOUT)
    }

    /// Same single-admission server with a caller-chosen accept bound, so
    /// the timeout path is testable without spending the full guard.
    fn start_with_accept_deadline(
        handler: impl FnOnce(UnixStream) -> io::Result<()> + Send + 'static,
        accept_timeout: Duration,
    ) -> io::Result<Self> {
        let directory = TestDirectory::new()?;
        #[cfg(unix)]
        let endpoint = directory.path.join("client.sock");
        #[cfg(windows)]
        let endpoint = crate::tests::windows_endpoints::endpoint();
        let listener = UnixListener::bind(&endpoint)?;
        listener.set_nonblocking(true)?;
        let thread = thread::spawn(move || {
            let stream = accept_bounded(&listener, Instant::now() + accept_timeout)?;
            stream.set_read_timeout(Some(SERVER_GUARD_TIMEOUT))?;
            stream.set_write_timeout(Some(SERVER_GUARD_TIMEOUT))?;
            handler(stream)
        });
        Ok(Self {
            _directory: directory,
            endpoint,
            thread,
            finish_signal: None,
        })
    }

    pub(crate) fn endpoint(&self) -> &Path {
        &self.endpoint
    }

    pub(crate) fn finish(self) -> io::Result<()> {
        if let Some(signal) = self.finish_signal {
            // If the peer already failed, join below retains its actual error.
            let _closed = signal.send(());
        }
        self.thread
            .join()
            .map_err(|_panic| io::Error::other("fake Server panicked"))?
    }
}

pub(super) struct TestDirectory {
    pub(super) path: PathBuf,
}

impl TestDirectory {
    pub(super) fn new() -> io::Result<Self> {
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchbee-local-client-{}-{fixture_id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique local Client test directory",
        ))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn shutdown_handle(stream: UnixStream) -> super::super::LocalClientShutdown {
    super::super::LocalClientShutdown(super::super::ShutdownTarget::Direct(stream))
}

pub(crate) fn connect_with_checkpoint_clock(
    endpoint: &Path,
    cohort: &str,
    expired: Arc<AtomicBool>,
) -> Result<super::super::LocalClient, super::super::LocalClientConnectError> {
    super::super::LocalClient::connect_with_submission_runtime(
        endpoint,
        cohort,
        super::super::REQUEST_TOTAL_TIMEOUT,
        Box::new(CheckpointClock {
            origin: Instant::now(),
            expired,
        }),
        super::super::IoChunkLimits::UNBOUNDED,
    )
}

pub(crate) fn connect_with_adjustable_clock(
    endpoint: &Path,
    cohort: &str,
    elapsed_millis: Arc<AtomicU64>,
) -> Result<super::super::LocalClient, super::super::LocalClientConnectError> {
    super::super::LocalClient::connect_with_submission_runtime(
        endpoint,
        cohort,
        super::super::REQUEST_TOTAL_TIMEOUT,
        Box::new(AdjustableClock {
            origin: Instant::now(),
            elapsed_millis,
        }),
        super::super::IoChunkLimits::UNBOUNDED,
    )
}

pub(crate) fn adjustable_operation_clock(
    elapsed_millis: Arc<AtomicU64>,
) -> super::super::LocalOperationClock {
    super::super::LocalOperationClock(Arc::new(AdjustableClock {
        origin: Instant::now(),
        elapsed_millis,
    }))
}

/// A lane-admission origin whose endpoint never answers — enough to
/// spawn a real agent-board lane (real pipe + thread) in tests without
/// standing up a live OBAP socket.
pub(crate) fn disconnected_origin() -> super::super::LocalClientOrigin {
    super::super::LocalClientOrigin {
        multiplex: None,
        endpoint: PathBuf::from("/nonexistent-orchbee-agent-board.sock"),
        cohort: "agent-board-tests".to_owned(),
        local: true,
        carrier: None,
    }
}

/// A typed remote origin without an SSH process, pipe or running writer.
pub(crate) fn disconnected_remote_origin() -> super::super::LocalClientOrigin {
    let mut origin = disconnected_origin();
    origin.local = false;
    origin
}

pub(crate) fn connect_with_gated_prewrite_clock(
    endpoint: &Path,
    cohort: &str,
    elapsed_millis: Arc<AtomicU64>,
    prewrite_gate: Arc<PrewriteGate>,
) -> Result<super::super::LocalClient, super::super::LocalClientConnectError> {
    super::super::LocalClient::connect_with_submission_runtime(
        endpoint,
        cohort,
        super::super::REQUEST_TOTAL_TIMEOUT,
        Box::new(GatedPrewriteClock {
            origin: Instant::now(),
            elapsed_millis,
            prewrite_gate,
        }),
        super::super::IoChunkLimits::UNBOUNDED,
    )
}

pub(crate) fn connect_with_switchable_ceiling_clock(
    endpoint: &Path,
    cohort: &str,
    elapsed_millis: Arc<AtomicU64>,
    at_ceiling: Arc<AtomicBool>,
) -> io::Result<super::super::LocalClient> {
    let origin = Instant::now();
    let near_ceiling =
        near_ceiling_from(origin, super::super::REQUEST_TOTAL_TIMEOUT).ok_or_else(|| {
            io::Error::other("platform Instant range cannot provide the required test ceiling")
        })?;
    super::super::LocalClient::connect_with_submission_runtime(
        endpoint,
        cohort,
        super::super::REQUEST_TOTAL_TIMEOUT,
        Box::new(SwitchableCeilingClock {
            origin,
            near_ceiling,
            elapsed_millis,
            at_ceiling,
        }),
        super::super::IoChunkLimits::UNBOUNDED,
    )
    .map_err(io::Error::other)
}

fn near_ceiling_from(origin: Instant, required_overflow: Duration) -> Option<Instant> {
    let mut lower = 0_u64;
    let mut upper = u64::MAX;
    while lower < upper {
        let width = upper - lower;
        let midpoint = lower + width / 2 + width % 2;
        if origin.checked_add(Duration::from_secs(midpoint)).is_some() {
            lower = midpoint;
        } else {
            upper = midpoint - 1;
        }
    }
    let near_ceiling = origin.checked_add(Duration::from_secs(lower))?;
    (near_ceiling > origin && near_ceiling.checked_add(required_overflow).is_none())
        .then_some(near_ceiling)
}

pub(crate) fn connect_with_next_ui_sample_expiry_clock(
    endpoint: &Path,
    cohort: &str,
    phase: Arc<AtomicU8>,
) -> Result<super::super::LocalClient, super::super::LocalClientConnectError> {
    connect_with_ui_sample_expiry_clock(endpoint, cohort, phase, 1)
}

pub(crate) fn connect_with_ui_sample_expiry_clock(
    endpoint: &Path,
    cohort: &str,
    phase: Arc<AtomicU8>,
    safe_samples: u8,
) -> Result<super::super::LocalClient, super::super::LocalClientConnectError> {
    super::super::LocalClient::connect_with_submission_runtime(
        endpoint,
        cohort,
        super::super::REQUEST_TOTAL_TIMEOUT,
        Box::new(NextUiSampleExpiryClock {
            origin: Instant::now(),
            phase,
            ui_thread: thread::current().id(),
            safe_samples,
        }),
        super::super::IoChunkLimits::UNBOUNDED,
    )
}

#[derive(Debug)]
struct AdjustableClock {
    origin: Instant,
    elapsed_millis: Arc<AtomicU64>,
}

#[derive(Debug)]
struct GatedPrewriteClock {
    origin: Instant,
    elapsed_millis: Arc<AtomicU64>,
    prewrite_gate: Arc<PrewriteGate>,
}

#[derive(Debug, Default)]
struct PrewriteGateState {
    armed: bool,
    blocked: bool,
    released: bool,
}

#[derive(Debug, Default)]
pub(crate) struct PrewriteGate {
    state: Mutex<PrewriteGateState>,
    changed: Condvar,
}

impl PrewriteGate {
    pub(crate) fn arm(&self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if state.armed || state.blocked {
            return Err(io::Error::other("prewrite gate is already armed"));
        }
        state.armed = true;
        state.released = false;
        Ok(())
    }

    pub(crate) fn await_blocked_before(&self, wall_deadline: Instant) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        while !state.blocked {
            let remaining = wall_deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, timed_out) = match self.changed.wait_timeout(state, remaining) {
                Ok((state, result)) => (state, result.timed_out()),
                Err(poison) => {
                    let (state, result) = poison.into_inner();
                    (state, result.timed_out())
                }
            };
            state = next;
            if timed_out && !state.blocked {
                return false;
            }
        }
        true
    }

    pub(crate) fn release(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.released = true;
        self.changed.notify_all();
    }

    fn block_if_armed(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !state.armed {
            return;
        }
        state.blocked = true;
        self.changed.notify_all();
        while !state.released {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(|poison| poison.into_inner());
        }
        state.armed = false;
        state.blocked = false;
        state.released = false;
        self.changed.notify_all();
    }
}

#[derive(Debug)]
struct SwitchableCeilingClock {
    origin: Instant,
    near_ceiling: Instant,
    elapsed_millis: Arc<AtomicU64>,
    at_ceiling: Arc<AtomicBool>,
}

impl super::super::MonotonicClock for SwitchableCeilingClock {
    fn now(&self) -> Instant {
        if self.at_ceiling.load(Ordering::SeqCst) {
            self.near_ceiling
        } else {
            self.origin
                .checked_add(Duration::from_millis(
                    self.elapsed_millis.load(Ordering::SeqCst),
                ))
                .expect("switchable test clock must remain below its ceiling before the switch")
        }
    }
}

impl super::super::MonotonicClock for AdjustableClock {
    fn now(&self) -> Instant {
        self.origin
            .checked_add(Duration::from_millis(
                self.elapsed_millis.load(Ordering::SeqCst),
            ))
            .expect("adjustable test clock must remain inside the monotonic clock range")
    }
}

impl super::super::MonotonicClock for GatedPrewriteClock {
    fn now(&self) -> Instant {
        self.origin
            .checked_add(Duration::from_millis(
                self.elapsed_millis.load(Ordering::SeqCst),
            ))
            .expect("gated prewrite test clock must remain inside the monotonic clock range")
    }

    fn before_request_write(&self) {
        self.prewrite_gate.block_if_armed();
    }
}

#[derive(Debug)]
struct CheckpointClock {
    origin: Instant,
    expired: Arc<AtomicBool>,
}

#[derive(Debug)]
struct NextUiSampleExpiryClock {
    origin: Instant,
    phase: Arc<AtomicU8>,
    ui_thread: thread::ThreadId,
    safe_samples: u8,
}

impl super::super::MonotonicClock for NextUiSampleExpiryClock {
    fn now(&self) -> Instant {
        let phase = self.phase.load(Ordering::SeqCst);
        if phase == 0 || thread::current().id() != self.ui_thread {
            return if phase < self.safe_samples + 2 {
                self.origin
            } else {
                self.origin + Duration::from_secs(120)
            };
        }
        if phase <= self.safe_samples {
            self.phase.store(phase + 1, Ordering::SeqCst);
            self.origin
        } else {
            self.phase.store(self.safe_samples + 2, Ordering::SeqCst);
            self.origin + Duration::from_secs(120)
        }
    }
}

impl super::super::MonotonicClock for CheckpointClock {
    fn now(&self) -> Instant {
        if self.expired.load(Ordering::SeqCst) {
            self.origin + Duration::from_secs(120)
        } else {
            self.origin
        }
    }
}

#[cfg(test)]
mod tests;
