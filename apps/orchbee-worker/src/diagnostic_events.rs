//! The Worker's diagnostic log records: its own start and end, each Terminal Host start
//! it attempts, a replacement Worker's adoption with its counts, the Host anomalies it contains with the class of each, the Server requests it
//! declines with their cause, a Server message it drops, an operation it could not prepare, and
//! background cleanup its end could not prove, so Host diagnostics reach the log through the
//! Worker. A record carries only closed tokens and numbers: an error kind, a site and reason, a
//! Host message kind number and its anomaly, a request kind and decline cause, and for a Host
//! start its stage and codes; no record holds terminal content, input, launch commands, messages
//! or paths.
use orchbee_local_runtime::{LogField, LogLevel, LogValue, log_event, log_panicked};
use std::{
    cell::Cell,
    error::Error,
    fmt, io,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
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

#[cfg(test)]
thread_local! {
    /// The contained-failure records this thread asked for, before a once-a-minute slot drops one,
    /// so a test that drives the serving loop reads where a contained failure was recorded.
    static NOTED: std::cell::RefCell<Vec<(&'static str, Vec<LogField>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn note(event: &'static str, fields: &[LogField]) {
    NOTED.with_borrow_mut(|noted| noted.push((event, fields.to_vec())));
}

/// The contained-failure records this thread asked for since the last call, oldest first, each
/// with its event name and fields.
#[cfg(test)]
pub(crate) fn take_noted() -> Vec<(&'static str, Vec<LogField>)> {
    NOTED.with_borrow_mut(std::mem::take)
}

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
    Titles,
    Cleanup,
    #[cfg(windows)]
    Handoff,
    HostStart,
    Shutdown,
    /// A replacement Worker adopting the Host of the Worker it replaces.
    Adopt,
    /// A move turn: a terminal's move from an older Host into the Worker's own.
    Move,
}

/// Each site keeps its own once-a-minute slot for `host-slow` and for `host-unexpected`.
const SITE_SLOTS: usize = 18;
const _: () = assert!((Site::Adopt as usize) < SITE_SLOTS);
const _: () = assert!((Site::Move as usize) < SITE_SLOTS);

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
            Self::Titles => "titles",
            Self::Cleanup => "cleanup",
            #[cfg(windows)]
            Self::Handoff => "handoff",
            Self::HostStart => "host-start",
            Self::Shutdown => "shutdown",
            Self::Adopt => "adopt",
            Self::Move => "move",
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

/// The Session's Keeper started and its first Host attached it.
pub(crate) fn keeper_started() {
    log_event(LogLevel::Info, "keeper-started", &[]);
}

/// A Keeper step that failed, which leaves the terminals running, unheld or in the Keeper's care:
/// its `step` token, the closed I/O error kind, and the operating-system error code it carries.
pub(crate) fn keeper_failed(step: &'static str, error: &(dyn Error + 'static)) {
    let mut fields = vec![error_kind(error), ("step", LogValue::Token(step))];
    if let Some(code) = os_error(error) {
        fields.push(("osError", LogValue::Count(u64::from(code))));
    }
    #[cfg(test)]
    note("keeper-failed", &fields);
    log_event(LogLevel::Warning, "keeper-failed", &fields);
}

/// An `orchbee-files` start that failed: its `stage` token (`previous`, `unverified` or `spawn`),
/// the closed I/O error kind, and the operating-system error code it carries.
pub(crate) fn files_start_failed(stage: &'static str, error: &(dyn Error + 'static)) {
    let mut fields = vec![error_kind(error), ("stage", LogValue::Token(stage))];
    if let Some(code) = os_error(error) {
        fields.push(("osError", LogValue::Count(u64::from(code))));
    }
    log_event(LogLevel::Warning, "files-start-failed", &fields);
}

/// A Host's epoch could not be written into the Session record: the closed I/O error kind.
pub(crate) fn host_epoch_unrecorded(error: &io::Error) {
    let fields = [error_kind(error)];
    log_event(LogLevel::Warning, "host-epoch-unrecorded", &fields);
}

/// An older Host in `slot`, from another installation, held no terminal and ran no program, and was
/// told to drain.
pub(crate) fn host_draining(slot: u8) {
    log_event(
        LogLevel::Info,
        "host-draining",
        &[("slot", LogValue::Count(u64::from(slot)))],
    );
}

/// The drained Host in `slot` ended: `released` once the Session record freed its slot.
pub(crate) fn host_drained(slot: u8, released: bool) {
    log_event(
        LogLevel::Info,
        "host-drained",
        &[
            ("slot", LogValue::Count(u64::from(slot))),
            ("released", LogValue::Flag(released)),
        ],
    );
}

/// A terminal moved from the Host in slot `from` to the one in `to`: its record's size, whether it
/// had settled, and the milliseconds from the quiesce to the claim.
pub(crate) fn terminal_moved(
    from: u8,
    to: u8,
    record_bytes: u32,
    settled: bool,
    milliseconds: u64,
) {
    log_event(
        LogLevel::Info,
        "terminal-moved",
        &[
            ("from", LogValue::Count(u64::from(from))),
            ("to", LogValue::Count(u64::from(to))),
            ("recordBytes", LogValue::Count(u64::from(record_bytes))),
            ("settled", LogValue::Flag(settled)),
            ("elapsedMs", LogValue::Count(milliseconds)),
        ],
    );
}

/// A move's `step` failed: the refusal's `class` and `reason`, and whether moves stopped for the
/// rest of the Worker's life.
pub(crate) fn terminal_move_failed(
    step: &'static str,
    class: &'static str,
    reason: &'static str,
    stopped: bool,
) {
    log_event(
        LogLevel::Warning,
        "terminal-move-failed",
        &[
            ("step", LogValue::Token(step)),
            ("class", LogValue::Token(class)),
            ("reason", LogValue::Token(reason)),
            ("stopped", LogValue::Flag(stopped)),
        ],
    );
}

/// The Session's Keeper ended or stopped answering, and another took its place.
pub(crate) fn keeper_replaced() {
    log_event(LogLevel::Info, "keeper-replaced", &[]);
}

/// The Worker updated the Session's Keeper to its own program: on Windows a new Keeper held
/// `terminals` terminal holdings and `programs` program holdings, and on macOS and Linux the Keeper
/// carried as many of each into the program it executed; the update took `elapsed` up to its
/// commit.
pub(crate) fn keeper_updated(terminals: usize, programs: usize, elapsed: Duration) {
    log_event(
        LogLevel::Info,
        "keeper-updated",
        &[
            ("terminals", count(terminals)),
            ("programs", count(programs)),
            ("elapsedMs", milliseconds(elapsed)),
        ],
    );
}

/// A Keeper update failed at `step` on its `attempt`, from 1, and was rolled back, so the old
/// Keeper stays: the closed I/O error kind and the operating-system error code it carries.
pub(crate) fn keeper_update_failed(
    step: &'static str,
    attempt: u32,
    error: &(dyn Error + 'static),
) {
    let mut fields = vec![
        error_kind(error),
        ("step", LogValue::Token(step)),
        ("attempt", LogValue::Count(u64::from(attempt))),
    ];
    if let Some(code) = os_error(error) {
        fields.push(("osError", LogValue::Count(u64::from(code))));
    }
    #[cfg(test)]
    note("keeper-update-failed", &fields);
    log_event(LogLevel::Warning, "keeper-update-failed", &fields);
}

fn count(value: usize) -> LogValue {
    LogValue::Count(u64::try_from(value).unwrap_or(u64::MAX))
}

/// The Host's dispatch stopped turning for `stalled`: `ended` when the Keeper held every terminal
/// it runs and the Worker ended it, so recovery follows; otherwise the Host is left running.
pub(crate) fn host_hung(ended: bool, stalled: Duration) {
    log_event(
        LogLevel::Warning,
        "host-hung",
        &[
            ("ended", LogValue::Flag(ended)),
            ("stalledMs", milliseconds(stalled)),
        ],
    );
}

/// What a replacement Worker's adoption found: its target, its outcome token, and the counts it
/// read. A failed adoption also writes an `exited` record at site `adopt`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdoptionRecord {
    /// `no-host`, `host`, or `recovery`.
    pub(crate) target: &'static str,
    /// `adopted` or the failure token.
    pub(crate) outcome: &'static str,
    pub(crate) imported: u32,
    pub(crate) quarantined: u32,
    pub(crate) missing: u32,
    pub(crate) pages: u32,
    pub(crate) inventory_restarts: u32,
    pub(crate) elapsed: Duration,
}

pub(crate) fn adoption(record: AdoptionRecord) {
    let fields = [
        ("target", LogValue::Token(record.target)),
        ("outcome", LogValue::Token(record.outcome)),
        ("imported", LogValue::Count(u64::from(record.imported))),
        (
            "quarantined",
            LogValue::Count(u64::from(record.quarantined)),
        ),
        ("missing", LogValue::Count(u64::from(record.missing))),
        ("pages", LogValue::Count(u64::from(record.pages))),
        (
            "inventoryRestarts",
            LogValue::Count(u64::from(record.inventory_restarts)),
        ),
        ("elapsedMs", milliseconds(record.elapsed)),
    ];
    #[cfg(test)]
    note("adoption", &fields);
    let level = if record.outcome == "adopted" {
        LogLevel::Info
    } else {
        LogLevel::Error
    };
    log_event(level, "adoption", &fields);
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
    #[cfg(test)]
    note("host-start-failed", &fields);
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

/// Why an intact Host message was dropped or refused instead of ending the Worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Anomaly {
    /// A correlated answer other than the acknowledgement; the request is refused.
    Answer,
    /// An event for an attachment the Worker does not hold; the event is dropped.
    UnknownAttachment,
    /// A response no request is waiting for, or one that names another request; it is dropped.
    Stray,
    /// A break in one attachment's flow or sequence; that viewer or controller is closed.
    Flow,
    /// An event whose envelope or payload breaks its contract; the event is dropped, and an
    /// attachment it names is closed.
    Contract,
}

impl Anomaly {
    const fn token(self) -> &'static str {
        match self {
            Self::Answer => "answer",
            Self::UnknownAttachment => "unknown-attachment",
            Self::Stray => "stray",
            Self::Flow => "flow",
            Self::Contract => "contract",
        }
    }
}

/// An intact Host message this Worker dropped or refused instead of ending, with its `anomaly`. At
/// most one record a minute for each site, with the number its slot suppressed since the last.
pub(crate) fn host_unexpected(site: Site, kind: u16, anomaly: Anomaly) {
    static SLOTS: SiteSlots = SiteSlots::new();
    let fields = [
        ("site", LogValue::Token(site.token())),
        ("kind", LogValue::Count(u64::from(kind))),
        ("anomaly", LogValue::Token(anomaly.token())),
    ];
    #[cfg(test)]
    note("host-unexpected", &fields);
    if let Some(suppressed) = SLOTS.admit(site) {
        let [site, kind, anomaly] = fields;
        log_event(
            LogLevel::Warning,
            "host-unexpected",
            &[
                site,
                kind,
                anomaly,
                ("suppressed", LogValue::Count(suppressed)),
            ],
        );
    }
}

/// Why a whole Server request was declined instead of served or ending the Worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeclineCause {
    /// A kind this Worker does not serve on its platform, or a terminal request before the first
    /// Launch.
    Unserved,
    /// A request addressed to another Worker or generation.
    Fence,
    /// An Agent drain acknowledgement above the watermark this Worker offered.
    Watermark,
    /// An ordinary request while a Server handoff is prepared.
    #[cfg(windows)]
    Handoff,
    /// A request to a replacement Worker before its adoption, or an adoption page that breaks
    /// the pages before it.
    Unavailable,
    /// The operation kernel refused it before anything reached the Host.
    State,
}

const _: () = assert!((DeclineCause::State as usize) < SITE_SLOTS);

impl DeclineCause {
    const fn token(self) -> &'static str {
        match self {
            Self::Unserved => "unserved",
            Self::Fence => "fence",
            Self::Watermark => "watermark",
            #[cfg(windows)]
            Self::Handoff => "handoff",
            Self::Unavailable => "unavailable",
            Self::State => "state",
        }
    }
}

/// A whole Server request this Worker declined instead of ending, with its `cause`. At most one
/// record a minute for each cause, with the number suppressed since the last.
pub(crate) fn request_declined(
    kind: orchbee_capability_node_protocol::DeclinedRequestKind,
    cause: DeclineCause,
) {
    static SLOTS: SiteSlots = SiteSlots::new();
    let fields = [
        ("kind", LogValue::Token(declined_token(kind))),
        ("cause", LogValue::Token(cause.token())),
    ];
    #[cfg(test)]
    note("request-declined", &fields);
    if let Some(suppressed) = SLOTS.admit_slot(cause as usize) {
        let [kind, cause] = fields;
        log_event(
            LogLevel::Warning,
            "request-declined",
            &[kind, cause, ("suppressed", LogValue::Count(suppressed))],
        );
    }
}

/// What a Worker received from the Server that was no request it could serve or decline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServerUnexpected {
    /// A message only a Worker sends, dropped unanswered: no Server request waits for it.
    WorkerOnly,
    /// A whole frame this Worker could not decode, answered as unreadable.
    Unreadable,
}

/// A Server message this Worker could neither serve nor decline. At most one record a minute,
/// with the number suppressed since the last.
pub(crate) fn server_unexpected(message: ServerUnexpected) {
    static SLOTS: SiteSlots = SiteSlots::new();
    let message = (
        "message",
        LogValue::Token(match message {
            ServerUnexpected::WorkerOnly => "worker-only",
            ServerUnexpected::Unreadable => "unreadable",
        }),
    );
    #[cfg(test)]
    note("server-unexpected", &[message]);
    if let Some(suppressed) = SLOTS.admit(Site::ServerReceive) {
        log_event(
            LogLevel::Warning,
            "server-unexpected",
            &[message, ("suppressed", LogValue::Count(suppressed))],
        );
    }
}

/// A terminal operation at `site` that failed before handoff because this Worker could not prepare
/// it, such as a home folder lookup that passed its bound or a request too large for one Host
/// record; nothing reached the Host. At most one record a minute for each site, with the number
/// suppressed since the last.
pub(crate) fn request_unprepared(site: Site, error: &(dyn Error + 'static)) {
    static SLOTS: SiteSlots = SiteSlots::new();
    let fields = [("site", LogValue::Token(site.token())), error_kind(error)];
    #[cfg(test)]
    note("request-unprepared", &fields);
    if let Some(suppressed) = SLOTS.admit(site) {
        let [site, error] = fields;
        log_event(
            LogLevel::Warning,
            "request-unprepared",
            &[site, error, ("suppressed", LogValue::Count(suppressed))],
        );
    }
}

/// A terminal send whose outcome this Worker built and the protocol refused, so the request was
/// answered with the conservative unknown outcome instead. At most one record a minute, with the
/// number suppressed since the last.
pub(crate) fn send_outcome_unbuildable() {
    static SLOTS: SiteSlots = SiteSlots::new();
    let fields = [("site", LogValue::Token(Site::Send.token()))];
    #[cfg(test)]
    note("send-outcome-unbuildable", &fields);
    if let Some(suppressed) = SLOTS.admit(Site::Send) {
        let [site] = fields;
        log_event(
            LogLevel::Warning,
            "send-outcome-unbuildable",
            &[site, ("suppressed", LogValue::Count(suppressed))],
        );
    }
}

/// Background jobs whose stop could not be proven when the serving loop ended, or when a shutdown
/// that keeps the terminals went on without that proof. The loop's own end keeps its `exited`
/// record; this one is written beside it.
pub(crate) fn cleanup_unproven(error: &io::Error) {
    log_event(LogLevel::Error, "cleanup-unproven", &[error_kind(error)]);
}

fn declined_token(kind: orchbee_capability_node_protocol::DeclinedRequestKind) -> &'static str {
    use orchbee_capability_node_protocol::DeclinedRequestKind as Kind;
    match kind {
        Kind::Launch => "launch",
        Kind::Start => "start",
        Kind::Viewer => "viewer",
        Kind::Control => "control",
        Kind::Lifecycle => "lifecycle",
        Kind::CwdValidation => "cwd-validation",
        Kind::Send => "send",
        Kind::WorktreeJob => "worktree-job",
        Kind::AgentDrain => "agent-drain",
        Kind::Shutdown => "shutdown",
        Kind::WorktreeMutation => "worktree-mutation",
        Kind::Titles => "titles",
        Kind::Handoff => "handoff",
        Kind::AgentIntegrationJob => "agent-integration-job",
        Kind::AgentProbeJob => "agent-probe-job",
        Kind::HostQuery => "host-query",
        Kind::TitleChanges => "title-changes",
        Kind::PluginJob => "plugin-job",
        Kind::GraphicsDecode => "graphics-decode",
        Kind::AdoptHost => "adopt-host",
        Kind::MoveTurn => "move-turn",
        Kind::FilesProcess => "files-process",
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
        self.admit_slot(site as usize)
    }

    /// [`Self::admit`] for the slot at `index`, for records keyed by something other than a site.
    fn admit_slot(&self, index: usize) -> Option<u64> {
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
        log_panicked(None, info.location(), None);
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
