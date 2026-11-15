//! The Worker's diagnostic log records: its own start and end, each Terminal Host start
//! it attempts, and the Host anomalies it contains, so Host diagnostics reach the log through the
//! Worker. A record carries only closed tokens and numbers: an error kind, a site and reason, a Host
//! message kind number, and for a Host start its stage and codes; no record holds terminal content,
//! input, launch commands, messages or paths.
use orchbee_local_runtime::{LogField, LogLevel, LogValue, log_event};
use std::{
    cell::Cell,
    error::Error,
    fmt, io,
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

thread_local! {
    /// The site the serving thread has reached, once `started` named it; `None` on every other
    /// thread, whose panic does not end the Worker.
    static SERVING: Cell<Option<Site>> = const { Cell::new(None) };
}

/// Whether this run's one `exited` record has been written.
static EXIT_RECORDED: AtomicBool = AtomicBool::new(false);

/// Where the Worker was serving when it ended or when a contained anomaly arose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Site {
    ServerReceive,
    ServerSend,
    Viewer,
    ProcessRead,
    Lifecycle,
    Graphics,
    Control,
    Send,
    Launch,
    Start,
    Drain,
    #[cfg(windows)]
    Titles,
    Cleanup,
    #[cfg(windows)]
    Handoff,
    HostStart,
    #[cfg(windows)]
    Shutdown,
}

/// Each site keeps its own once-a-minute slot for `host-slow` and for `host-unexpected`.
const SITE_SLOTS: usize = 16;
#[cfg(windows)]
const _: () = assert!((Site::Shutdown as usize) < SITE_SLOTS);
#[cfg(not(windows))]
const _: () = assert!((Site::HostStart as usize) < SITE_SLOTS);

impl Site {
    const fn token(self) -> &'static str {
        match self {
            Self::ServerReceive => "server-receive",
            Self::ServerSend => "server-send",
            Self::Viewer => "viewer",
            Self::ProcessRead => "process-read",
            Self::Lifecycle => "lifecycle",
            Self::Graphics => "graphics",
            Self::Control => "control",
            Self::Send => "send",
            Self::Launch => "launch",
            Self::Start => "start",
            Self::Drain => "drain",
            #[cfg(windows)]
            Self::Titles => "titles",
            Self::Cleanup => "cleanup",
            #[cfg(windows)]
            Self::Handoff => "handoff",
            Self::HostStart => "host-start",
            #[cfg(windows)]
            Self::Shutdown => "shutdown",
        }
    }
}

/// Why the serving loop ended, as one closed class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reason {
    /// A peer stayed silent past its hard bound.
    Deadline,
    HostClosed,
    /// The Host broke its framing or a contract this Worker cannot contain.
    HostProtocol,
    ServerClosed,
    ServerProtocol,
    /// A Worker-side invariant (the operation kernel or its slots) refused to continue.
    State,
    Local,
    /// The serving thread panicked.
    Panic,
}

impl Reason {
    const fn token(self) -> &'static str {
        match self {
            Self::Deadline => "deadline",
            Self::HostClosed => "host-closed",
            Self::HostProtocol => "host-protocol",
            Self::ServerClosed => "server-closed",
            Self::ServerProtocol => "server-protocol",
            Self::State => "state",
            Self::Local => "local",
            Self::Panic => "panic",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Peer {
    Host,
    Server,
    None,
}

impl Peer {
    const fn token(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Server => "server",
            Self::None => "none",
        }
    }
}

/// The classified cause the serving loop attaches to its final error. Its text is the wrapped
/// error's; the exit record reads only the closed fields.
#[derive(Debug)]
pub(crate) struct ExitCause {
    pub(crate) site: Site,
    pub(crate) reason: Reason,
    pub(crate) peer: Peer,
    /// How long the peer was waited for and the bound it passed, for a deadline.
    pub(crate) waited: Option<(Duration, Duration)>,
    pub(crate) source: Box<dyn Error>,
}

impl fmt::Display for ExitCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl Error for ExitCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Names the calling thread as the one serving the Server and records the start.
pub(crate) fn started() {
    SERVING.with(|serving| serving.set(Some(Site::ServerReceive)));
    log_event(
        LogLevel::Info,
        "started",
        &[("version", LogValue::Token(env!("CARGO_PKG_VERSION")))],
    );
}

/// Notes the site the serving thread has reached, so a panic there names it; other threads keep
/// none.
pub(crate) fn enter(site: Site) {
    let _ = SERVING.try_with(|serving| {
        if serving.get().is_some() {
            serving.set(Some(site));
        }
    });
}

/// The site a serving loop has reached before it has a Host connection to keep it, noted for a
/// panic there as well.
pub(crate) struct ServingSite(Cell<Site>);

impl ServingSite {
    pub(crate) fn new(site: Site) -> Self {
        enter(site);
        Self(Cell::new(site))
    }

    pub(crate) fn set(&self, site: Site) {
        enter(site);
        self.0.set(site);
    }

    pub(crate) fn get(&self) -> Site {
        self.0.get()
    }
}

/// Records how the run ended. A run has one such record: a panic on the serving thread may have
/// written it already.
pub(crate) fn exited(outcome: &Result<(), Box<dyn Error>>) {
    if EXIT_RECORDED.swap(true, Ordering::Relaxed) {
        return;
    }
    let error = match outcome {
        Ok(()) => {
            log_event(
                LogLevel::Info,
                "exited",
                &[("outcome", LogValue::Token("ok"))],
            );
            return;
        }
        Err(error) => error.as_ref(),
    };
    let mut fields = vec![("outcome", LogValue::Token("failed")), error_kind(error)];
    if let Some(cause) = error.downcast_ref::<ExitCause>() {
        fields.push(("site", LogValue::Token(cause.site.token())));
        fields.push(("reason", LogValue::Token(cause.reason.token())));
        fields.push(("peer", LogValue::Token(cause.peer.token())));
        if let Some((waited, budget)) = cause.waited {
            fields.push(("waitedMs", milliseconds(waited)));
            fields.push(("budgetMs", milliseconds(budget)));
        }
    }
    log_event(LogLevel::Error, "exited", &fields);
}

pub(crate) fn host_started() {
    log_event(LogLevel::Info, "host-started", &[]);
}

/// Names where a Host start failed: its `stage` token, the operating-system error code `error`
/// carries, the Host's exit code when it had exited, and the time since the start began.
pub(crate) fn host_start_failed(
    stage: &'static str,
    error: &(dyn Error + 'static),
    host_exit: Option<u32>,
    elapsed: Duration,
) {
    let mut fields = vec![
        error_kind(error),
        ("stage", LogValue::Token(stage)),
        ("elapsedMs", milliseconds(elapsed)),
    ];
    if let Some(code) = os_error(error) {
        fields.push(("osError", LogValue::Count(u64::from(code))));
    }
    if let Some(code) = host_exit {
        fields.push(("hostExit", LogValue::Count(u64::from(code))));
    }
    log_event(LogLevel::Error, "host-start-failed", &fields);
}

/// A Host exchange that succeeded after more than a second, at most one record a minute for each
/// site, with the number its slot suppressed since the last.
pub(crate) fn host_slow(site: Site, waited: Duration) {
    static SLOTS: SiteSlots = SiteSlots::new();
    if let Some(suppressed) = SLOTS.admit(site) {
        log_event(
            LogLevel::Warning,
            "host-slow",
            &[
                ("site", LogValue::Token(site.token())),
                ("waitedMs", milliseconds(waited)),
                ("suppressed", LogValue::Count(suppressed)),
            ],
        );
    }
}

/// An intact Host message this Worker dropped or refused instead of ending: a late event, an event
/// for an attachment it does not hold, or a correlated answer other than the acknowledgement. At
/// most one record a minute for each site, with the number its slot suppressed since the last.
pub(crate) fn host_unexpected(site: Site, kind: u16) {
    static SLOTS: SiteSlots = SiteSlots::new();
    if let Some(suppressed) = SLOTS.admit(site) {
        log_event(
            LogLevel::Warning,
            "host-unexpected",
            &[
                ("site", LogValue::Token(site.token())),
                ("kind", LogValue::Count(u64::from(kind))),
                ("suppressed", LogValue::Count(suppressed)),
            ],
        );
    }
}

/// A once-a-minute record slot for each site, counting the records it suppressed.
struct SiteSlots {
    last: [AtomicU64; SITE_SLOTS],
    suppressed: [AtomicU64; SITE_SLOTS],
}

impl SiteSlots {
    const fn new() -> Self {
        Self {
            last: [const { AtomicU64::new(0) }; SITE_SLOTS],
            suppressed: [const { AtomicU64::new(0) }; SITE_SLOTS],
        }
    }

    /// Admits `site`'s record at most once a minute and returns how many of its records were
    /// suppressed since the last one admitted; a record not admitted is counted instead.
    fn admit(&self, site: Site) -> Option<u64> {
        let index = site as usize;
        let (last, suppressed) = (self.last.get(index)?, self.suppressed.get(index)?);
        if admit_once_a_minute(last) {
            Some(suppressed.swap(0, Ordering::Relaxed))
        } else {
            suppressed.fetch_add(1, Ordering::Relaxed);
            None
        }
    }
}

/// Records `panicked` with the panicking file's stem and line, never the message, and then runs
/// the hook that was installed before, so a panic still reports as it did. A panic on the serving
/// thread ends the Worker, so it also writes the run's `exited` record, with reason `panic` and
/// no peer; a panic on any other thread writes `panicked` alone.
pub(crate) fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let (file, line) = info.location().map_or(("unknown", 0), |location| {
            (file_stem_token(location.file()), location.line())
        });
        log_event(
            LogLevel::Error,
            "panicked",
            &[
                ("file", LogValue::Token(file)),
                ("line", LogValue::Count(u64::from(line))),
            ],
        );
        if let Some(site) = SERVING.try_with(Cell::get).ok().flatten()
            && !EXIT_RECORDED.swap(true, Ordering::Relaxed)
        {
            log_event(
                LogLevel::Error,
                "exited",
                &[
                    ("outcome", LogValue::Token("failed")),
                    ("error", LogValue::Token("other")),
                    ("site", LogValue::Token(site.token())),
                    ("reason", LogValue::Token(Reason::Panic.token())),
                    ("peer", LogValue::Token(Peer::None.token())),
                ],
            );
        }
        previous(info);
    }));
}

/// A source file's stem as a log token. Tokens are `'static`, so a stem is leaked, at most a few
/// times per process; a stem outside the token grammar is written as `invalid` by the log.
fn file_stem_token(file: &str) -> &'static str {
    static LEAKED: AtomicUsize = AtomicUsize::new(0);
    let Some(stem) = Path::new(file).file_stem().and_then(|stem| stem.to_str()) else {
        return "unknown";
    };
    if stem.len() > 64 || LEAKED.fetch_add(1, Ordering::Relaxed) >= 8 {
        return "other";
    }
    Box::leak(stem.to_owned().into_boxed_str())
}

fn admit_once_a_minute(last: &AtomicU64) -> bool {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    // Zero means never logged, so the clock starts at one millisecond.
    let now = u64::try_from(ORIGIN.get_or_init(Instant::now).elapsed().as_millis())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let previous = last.load(Ordering::Relaxed);
    if previous != 0 && now.saturating_sub(previous) < 60_000 {
        return false;
    }
    last.compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
}

fn milliseconds(duration: Duration) -> LogValue {
    LogValue::Count(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}

/// The first operating-system error code in `error`'s chain, through wrapped I/O errors and
/// sources, as the unsigned value the system reports.
fn os_error(error: &(dyn Error + 'static)) -> Option<u32> {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(error) = error.downcast_ref::<io::Error>() {
            if let Some(code) = error.raw_os_error() {
                return Some(u32::from_ne_bytes(code.to_ne_bytes()));
            }
            if let Some(inner) = error.get_ref() {
                current = Some(inner);
                continue;
            }
        }
        current = error.source();
    }
    None
}

/// The kind of the first I/O error in `error`'s chain, so a classified cause still reports the
/// kind of the failure it wraps.
fn error_kind(error: &(dyn Error + 'static)) -> LogField {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(error) = error.downcast_ref::<io::Error>() {
            return ("error", error.kind().into());
        }
        current = error.source();
    }
    ("error", LogValue::Token("other"))
}
