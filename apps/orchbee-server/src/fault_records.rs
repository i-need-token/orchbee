//! Records the Server's contained and uncontained faults in the daily diagnostic log: a listener
//! that drops one candidate and keeps serving, a Worker request that failed alone or a Worker reply
//! dropped while its connection kept serving, a Client stream closed while its connection kept
//! serving, a Worker connection that ended, a thread that panicked and a lock left poisoned. A
//! record holds static tokens, counts and flags only, never a panic message, request or terminal
//! content, a path, or an error's text. Records that repeat at one site are written at most once a
//! minute, and the next record from that site counts the ones left out.

use std::{
    io,
    panic::PanicHookInfo,
    sync::{
        LockResult, Mutex, MutexGuard, Once, PoisonError, TryLockError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use orchbee_local_runtime::{LogField, LogLevel, LogValue};

/// The shortest interval between two records from one site.
const RECORD_INTERVAL: Duration = Duration::from_secs(60);
/// The sites whose interval is tracked; a site beyond them is written without a limit.
const MAXIMUM_SITES: usize = 64;
/// A listener's first pause after a fault, doubled by each fault that follows within
/// [`QUIET_RESET`] up to [`LONGEST_PAUSE`].
const FIRST_PAUSE: Duration = Duration::from_millis(100);
const LONGEST_PAUSE: Duration = Duration::from_secs(1);
/// A listener fault this long after the previous one pauses for [`FIRST_PAUSE`] again.
const QUIET_RESET: Duration = Duration::from_secs(5);
/// Thread names and source-file stems a panic record may name; later names are written `other`.
const MAXIMUM_NAMES: usize = 128;

const LISTENER_FAULT: &str = "listener-fault";
const CANDIDATE_DROPPED: &str = "candidate-dropped";
const WORKER_REQUEST_FAILED: &str = "worker-request-failed";
const WORKER_REPLY_DROPPED: &str = "worker-reply-dropped";
const CLIENT_STREAM_FAILED: &str = "client-stream-failed";
const WORKER_CONNECTION_ENDED: &str = "worker-connection-ended";
const PANICKED: &str = "panicked";
const STATE_POISONED: &str = "state-poisoned";

fn write(level: LogLevel, event: &'static str, fields: &[LogField]) {
    #[cfg(test)]
    capture::push(event, fields);
    orchbee_local_runtime::log_event(level, event, fields);
}

/// The daily-log token of an I/O error kind.
pub(crate) fn io_token(kind: io::ErrorKind) -> &'static str {
    match LogValue::from(kind) {
        LogValue::Token(token) => token,
        LogValue::Count(_) | LogValue::Flag(_) => "other",
    }
}

/// One place that writes records; `line` distinguishes panic sites within one file.
#[derive(Clone, Copy, Eq, PartialEq)]
struct Site {
    event: &'static str,
    unit: &'static str,
    stage: &'static str,
    line: u32,
}

struct SiteInterval {
    site: Site,
    written: Instant,
    suppressed: u64,
}

static INTERVALS: Mutex<Vec<SiteInterval>> = Mutex::new(Vec::new());

/// Whether `site` may write now, and how many of its records were left out since its last one.
/// The panic hook never waits for the table: a panic raised while it is held writes without a
/// limit rather than deadlock.
fn admit(site: Site, now: Instant, wait: bool) -> Option<u64> {
    let mut intervals = if wait {
        INTERVALS.lock().unwrap_or_else(PoisonError::into_inner)
    } else {
        match INTERVALS.try_lock() {
            Ok(intervals) => intervals,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return Some(0),
        }
    };
    if let Some(interval) = intervals.iter_mut().find(|interval| interval.site == site) {
        if now.saturating_duration_since(interval.written) < RECORD_INTERVAL {
            interval.suppressed = interval.suppressed.saturating_add(1);
            return None;
        }
        interval.written = now;
        return Some(std::mem::take(&mut interval.suppressed));
    }
    if intervals.len() < MAXIMUM_SITES {
        intervals.push(SiteInterval {
            site,
            written: now,
            suppressed: 0,
        });
    }
    Some(0)
}

/// Paces one listener through faults it survives: each fault drops only its own candidate, writes
/// a `listener-fault` record (at most one a minute per stage), and returns how long the listener
/// pauses before it accepts again.
pub(crate) struct ListenerRecovery {
    listener: &'static str,
    pause: Duration,
    last_fault: Option<Instant>,
}

impl ListenerRecovery {
    pub(crate) const fn new(listener: &'static str) -> Self {
        Self {
            listener,
            pause: FIRST_PAUSE,
            last_fault: None,
        }
    }

    /// Records one fault at `stage`, with its I/O error kind when there is one, and returns the
    /// pause: 100 ms, doubling while faults repeat, at most 1 s.
    pub(crate) fn fault(&mut self, stage: &'static str, error: Option<io::ErrorKind>) -> Duration {
        let now = Instant::now();
        self.pause = match self.last_fault {
            Some(last) if now.saturating_duration_since(last) < QUIET_RESET => {
                self.pause.saturating_mul(2).min(LONGEST_PAUSE)
            }
            _ => FIRST_PAUSE,
        };
        self.last_fault = Some(now);
        let site = Site {
            event: LISTENER_FAULT,
            unit: self.listener,
            stage,
            line: 0,
        };
        if let Some(suppressed) = admit(site, now, true) {
            write(
                LogLevel::Warning,
                LISTENER_FAULT,
                &[
                    ("listener", LogValue::Token(self.listener)),
                    ("stage", LogValue::Token(stage)),
                    ("error", LogValue::Token(error.map_or("none", io_token))),
                    ("pauseMs", LogValue::Count(milliseconds(self.pause))),
                    ("suppressed", LogValue::Count(suppressed)),
                ],
            );
        }
        self.pause
    }
}

/// Records a candidate a listener closed before serving it: the operation and stage that dropped
/// it and the cause's token. At most one record a minute per operation and stage.
pub(crate) fn candidate_dropped(operation: &'static str, stage: &'static str, cause: &'static str) {
    let now = Instant::now();
    let site = Site {
        event: CANDIDATE_DROPPED,
        unit: operation,
        stage,
        line: 0,
    };
    if let Some(suppressed) = admit(site, now, true) {
        write(
            LogLevel::Warning,
            CANDIDATE_DROPPED,
            &[
                ("operation", LogValue::Token(operation)),
                ("stage", LogValue::Token(stage)),
                ("cause", LogValue::Token(cause)),
                ("suppressed", LogValue::Count(suppressed)),
            ],
        );
    }
}

/// Records one Worker request that failed while its connection kept serving: its kind, the
/// failure, the failure's I/O error kind or protocol error, and how long it had run. At most one
/// record a minute per kind and failure.
pub(crate) fn worker_request_failed(
    request: &'static str,
    failure: &'static str,
    error: &'static str,
    elapsed: Duration,
) {
    let now = Instant::now();
    let site = Site {
        event: WORKER_REQUEST_FAILED,
        unit: request,
        stage: failure,
        line: 0,
    };
    if let Some(suppressed) = admit(site, now, true) {
        write(
            LogLevel::Warning,
            WORKER_REQUEST_FAILED,
            &[
                ("request", LogValue::Token(request)),
                ("failure", LogValue::Token(failure)),
                ("error", LogValue::Token(error)),
                ("elapsedMs", LogValue::Count(milliseconds(elapsed))),
                ("suppressed", LogValue::Count(suppressed)),
            ],
        );
    }
}

/// Why a Worker connection dropped a reply that reached no request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DroppedReply {
    /// It answered a request that had already failed, this long after the failure.
    Late(Duration),
    /// It answered no request the connection remembers.
    Unmatched,
    /// Its frame was whole but its content invalid.
    Invalid,
}

/// Records a Worker reply the connection dropped and kept serving: why, the kind of the request it
/// answered (`none` when unknown), and its protocol error. At most one record a minute per cause
/// and request kind.
pub(crate) fn worker_reply_dropped(
    reply: DroppedReply,
    request: &'static str,
    error: &'static str,
) {
    let cause = match reply {
        DroppedReply::Late(_) => "late",
        DroppedReply::Unmatched => "unmatched",
        DroppedReply::Invalid => "invalid",
    };
    let now = Instant::now();
    let site = Site {
        event: WORKER_REPLY_DROPPED,
        unit: request,
        stage: cause,
        line: 0,
    };
    if let Some(suppressed) = admit(site, now, true) {
        let mut fields: Vec<LogField> = Vec::with_capacity(5);
        fields.push(("cause", LogValue::Token(cause)));
        fields.push(("request", LogValue::Token(request)));
        fields.push(("error", LogValue::Token(error)));
        if let DroppedReply::Late(late) = reply {
            fields.push(("lateMs", LogValue::Count(milliseconds(late))));
        }
        fields.push(("suppressed", LogValue::Count(suppressed)));
        write(LogLevel::Warning, WORKER_REPLY_DROPPED, &fields);
    }
}

/// Records one Client stream the Server closed while that Client's connection kept serving: the
/// stage that failed it and the failure's class. At most one record a minute per stage and failure.
pub(crate) fn client_stream_failed(stage: &'static str, failure: &'static str) {
    let now = Instant::now();
    let site = Site {
        event: CLIENT_STREAM_FAILED,
        unit: stage,
        stage: failure,
        line: 0,
    };
    if let Some(suppressed) = admit(site, now, true) {
        write(
            LogLevel::Warning,
            CLIENT_STREAM_FAILED,
            &[
                ("stage", LogValue::Token(stage)),
                ("failure", LogValue::Token(failure)),
                ("suppressed", LogValue::Count(suppressed)),
            ],
        );
    }
}

/// What the Server observed of its Worker process when that Worker's connection ended. Only a
/// Windows Session holds its Worker's handle after admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerProcessState {
    /// The Server holds no handle to the Worker process.
    NotHeld,
    #[cfg(windows)]
    Running,
    #[cfg(windows)]
    Exited(u32),
    /// The handle could not be read.
    #[cfg(windows)]
    Unknown,
}

/// One ended Worker connection, as its record states it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerConnectionEnd {
    /// How the connection ended: the carrier's exit, `panicked`, or `not-started`.
    pub(crate) exit: &'static str,
    /// The failure that ended it, `none` for an ordinary end.
    pub(crate) failure: &'static str,
    /// The failure's I/O error kind or protocol error, `none` without one.
    pub(crate) error: &'static str,
    /// The request in flight when the connection ended, with how long it had run.
    pub(crate) request: Option<(&'static str, Duration)>,
    /// Whether the Worker had answered a shutdown request before the end.
    pub(crate) after_shutdown: bool,
    pub(crate) process: WorkerProcessState,
}

impl WorkerConnectionEnd {
    /// The Server stopped the connection, or the Worker left after it answered a shutdown.
    fn ordinary(&self) -> bool {
        self.exit == "shutdown" || (self.after_shutdown && self.failure == "none")
    }
}

/// Records how the Worker connection ended. A Session has one Worker connection, so this record is
/// never limited.
pub(crate) fn worker_connection_ended(end: &WorkerConnectionEnd) {
    let mut fields: Vec<LogField> = Vec::with_capacity(8);
    fields.push(("exit", LogValue::Token(end.exit)));
    fields.push(("failure", LogValue::Token(end.failure)));
    fields.push(("error", LogValue::Token(end.error)));
    match end.request {
        Some((request, elapsed)) => {
            fields.push(("request", LogValue::Token(request)));
            fields.push(("elapsedMs", LogValue::Count(milliseconds(elapsed))));
        }
        None => fields.push(("request", LogValue::Token("none"))),
    }
    fields.push(("afterShutdown", LogValue::Flag(end.after_shutdown)));
    let process = match end.process {
        WorkerProcessState::NotHeld => "not-held",
        #[cfg(windows)]
        WorkerProcessState::Running => "running",
        #[cfg(windows)]
        WorkerProcessState::Exited(_) => "exited",
        #[cfg(windows)]
        WorkerProcessState::Unknown => "unknown",
    };
    fields.push(("workerProcess", LogValue::Token(process)));
    #[cfg(windows)]
    if let WorkerProcessState::Exited(code) = end.process {
        fields.push(("workerExitCode", LogValue::Count(u64::from(code))));
    }
    let level = if end.ordinary() {
        LogLevel::Info
    } else {
        LogLevel::Error
    };
    write(level, WORKER_CONNECTION_ENDED, &fields);
}

static PANIC_HOOK: Once = Once::new();

/// Installs, once per process, a panic hook that records the panic's thread and source site before
/// it runs the hook installed before it. The record never carries the panic's message.
pub(crate) fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            record_panic(info);
            previous(info);
        }));
    });
}

/// Runs inside the panic hook, where a second panic would abort the process: every step here is
/// infallible or falls back to a token.
fn record_panic(info: &PanicHookInfo<'_>) {
    let (file, line) = info.location().map_or(("unknown", 0), |location| {
        (intern(file_stem(location.file())), location.line())
    });
    let thread = thread_token();
    let site = Site {
        event: PANICKED,
        unit: file,
        stage: thread,
        line,
    };
    if let Some(suppressed) = admit(site, Instant::now(), false) {
        write(
            LogLevel::Error,
            PANICKED,
            &[
                ("thread", LogValue::Token(thread)),
                ("file", LogValue::Token(file)),
                ("line", LogValue::Count(u64::from(line))),
                ("suppressed", LogValue::Count(suppressed)),
            ],
        );
    }
}

/// The file name without its directories or `.rs`, so a record never carries a build path.
fn file_stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.strip_suffix(".rs").unwrap_or(name)
}

/// The panicking thread's name when it is one of this program's own static names: the main thread
/// or a thread named `orchbee-…` or `workbench-…`.
fn thread_token() -> &'static str {
    let thread = std::thread::current();
    match thread.name() {
        None => "unnamed",
        Some(name)
            if name == "main" || name.starts_with("orchbee-") || name.starts_with("workbench-") =>
        {
            intern(name)
        }
        Some(_) => "other",
    }
}

static NAMES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// A static copy of `text` for a record. Text outside the log's token grammar is `other`, and so is
/// every name after the first [`MAXIMUM_NAMES`]; a held table is `unavailable` rather than waited
/// for.
fn intern(text: &str) -> &'static str {
    let token = (1..=64).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if !token {
        return "other";
    }
    let mut names = match NAMES.try_lock() {
        Ok(names) => names,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return "unavailable",
    };
    if let Some(name) = names.iter().find(|name| **name == text) {
        return name;
    }
    if names.len() >= MAXIMUM_NAMES {
        return "other";
    }
    // Bounded by MAXIMUM_NAMES short names for the life of the process.
    let name: &'static str = Box::leak(text.to_owned().into_boxed_str());
    names.push(name);
    name
}

/// A Session-wide lock whose poisoning leaves every later request that needs it unavailable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GuardedLock {
    /// The Session coordinator: the Workbench authority and terminal coordination.
    Coordinator,
    /// The canonical Agent state's fold cell.
    AgentFold,
}

static COORDINATOR_POISONED: AtomicBool = AtomicBool::new(false);
static AGENT_FOLD_POISONED: AtomicBool = AtomicBool::new(false);

/// Records the first observation of `lock` poisoned; later observations write nothing.
pub(crate) fn state_poisoned(lock: GuardedLock) {
    let (observed, token) = match lock {
        GuardedLock::Coordinator => (&COORDINATOR_POISONED, "coordinator"),
        GuardedLock::AgentFold => (&AGENT_FOLD_POISONED, "agent-fold"),
    };
    if !observed.swap(true, Ordering::AcqRel) {
        write(
            LogLevel::Error,
            STATE_POISONED,
            &[("lock", LogValue::Token(token))],
        );
    }
}

/// A mutex that records its first observed poisoning as `state-poisoned`. Callers keep the poison
/// result and their own unavailable answer; the record only makes the loss visible.
pub(crate) struct RecordedLock<T> {
    inner: Mutex<T>,
    lock: GuardedLock,
}

impl<T> RecordedLock<T> {
    pub(crate) const fn new(value: T, lock: GuardedLock) -> Self {
        Self {
            inner: Mutex::new(value),
            lock,
        }
    }

    pub(crate) fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        let guard = self.inner.lock();
        if guard.is_err() {
            state_poisoned(self.lock);
        }
        guard
    }

    #[cfg(windows)]
    pub(crate) fn get_mut(&mut self) -> LockResult<&mut T> {
        let value = self.inner.get_mut();
        if value.is_err() {
            state_poisoned(self.lock);
        }
        value
    }

    #[cfg(test)]
    pub(crate) fn try_lock(&self) -> std::sync::TryLockResult<MutexGuard<'_, T>> {
        let guard = self.inner.try_lock();
        if matches!(guard, Err(TryLockError::Poisoned(_))) {
            state_poisoned(self.lock);
        }
        guard
    }
}

fn milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// The records written on one thread, for tests that prove what a fault records.
#[cfg(test)]
pub(crate) mod capture {
    use std::cell::RefCell;

    use orchbee_local_runtime::LogField;

    /// One written record.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) struct Record {
        pub(crate) event: &'static str,
        pub(crate) fields: Vec<LogField>,
    }

    thread_local! {
        static RECORDS: RefCell<Option<Vec<Record>>> = const { RefCell::new(None) };
    }

    /// Runs `run` and returns its result with every record this thread wrote meanwhile.
    pub(crate) fn records<R>(run: impl FnOnce() -> R) -> (R, Vec<Record>) {
        RECORDS.with(|records| *records.borrow_mut() = Some(Vec::new()));
        let result = run();
        let records = RECORDS.with(|records| records.borrow_mut().take().unwrap_or_default());
        (result, records)
    }

    /// Never panics: the panic hook calls it.
    pub(super) fn push(event: &'static str, fields: &[LogField]) {
        let _ = RECORDS.try_with(|records| {
            if let Ok(mut records) = records.try_borrow_mut()
                && let Some(records) = records.as_mut()
            {
                records.push(Record {
                    event,
                    fields: fields.to_vec(),
                });
            }
        });
    }
}

#[cfg(test)]
mod tests;
