//! The Worker's diagnostic log records: its own start and end, each Terminal Host start
//! it attempts, and the Host anomalies it contains, so Host diagnostics reach the log through the
//! Worker. A record carries only closed tokens and numbers: an error kind, a site and reason, a Host
//! message kind number, and for a Host start its stage and codes; no record holds terminal content,
//! input, launch commands, messages or paths.
use orchbee_local_runtime::{LogField, LogLevel, LogValue, log_event};
use std::{
    error::Error,
    fmt, io,
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

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

pub(crate) fn started() {
    log_event(
        LogLevel::Info,
        "started",
        &[("version", LogValue::Token(env!("CARGO_PKG_VERSION")))],
    );
}

pub(crate) fn exited(outcome: &Result<(), Box<dyn Error>>) {
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

/// A Host exchange that succeeded after more than a second, at most one record a minute.
pub(crate) fn host_slow(site: Site, waited: Duration) {
    static LAST: AtomicU64 = AtomicU64::new(0);
    if admit_once_a_minute(&LAST) {
        log_event(
            LogLevel::Warning,
            "host-slow",
            &[
                ("site", LogValue::Token(site.token())),
                ("waitedMs", milliseconds(waited)),
            ],
        );
    }
}

/// An intact Host message this Worker dropped or refused instead of ending: a late event, an event
/// for an attachment it does not hold, or a correlated answer other than the acknowledgement. At
/// most one record a minute.
pub(crate) fn host_unexpected(site: Site, kind: u16) {
    static LAST: AtomicU64 = AtomicU64::new(0);
    if admit_once_a_minute(&LAST) {
        log_event(
            LogLevel::Warning,
            "host-unexpected",
            &[
                ("site", LogValue::Token(site.token())),
                ("kind", LogValue::Count(u64::from(kind))),
            ],
        );
    }
}

/// Records `panicked` with the panicking file's stem and line, never the message, and then runs
/// the hook that was installed before, so a panic still reports as it did.
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
