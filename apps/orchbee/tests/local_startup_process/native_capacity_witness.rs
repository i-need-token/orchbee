//! C1 carrier-capacity process witness: a real Server + Worker + Host + TUI
//! under the authorized load map (sessions 32 / client connections 48 /
//! worker viewers 24 / controllers 8), boundary probes at every owned limit,
//! and slot-release witnesses on graceful and abrupt teardown. The map holds
//! exactly TWO TUI-class participants — the provisioned real TUI plus one
//! synthetic tui-like tenant — for eight standing viewer attachments.
//! Synthetic participants serialize on one admitted session each — at most
//! one outstanding request per session, matching the CLI contract — and every
//! refusal must arrive as a typed protocol outcome or a labeled transport
//! boundary event, never as a hang, a crash, or an outcome-unknown.

use super::native_cli_query::run_json;
use super::native_context::{DeadlineStream, WorkbenchPeer};
use super::native_multiview;
use super::*;
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ControlRejectedReason, ControlUnavailableReason, ControllerId,
    RequestId, SendText, SendUnavailableReason, ServerMessage, SiteId as ObcsSiteId,
    TerminalControlAction, TerminalControlOutcome, TerminalControlRequest,
    TerminalId as ObcsTerminalId, TerminalSendOutcome, TerminalSendPayload, TerminalSendRequest,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerId,
    ViewerUnavailableReason, WorkerId as ObcsWorkerId, read_server_message, write_client_message,
};
use orchbee_local_client_protocol::{ConnectClientError, RejectionReason};
use orchbee_workbench_protocol::{
    PanelNode, ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchAction,
    WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchPanelId, WorkbenchPanelSplitDirection,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchTabId, WorkbenchTerminalGeometry, read_workbench_response, write_workbench_request,
};
use std::cell::Cell;
use std::io::BufReader;
use std::num::NonZeroU64;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(15);
const WARM_WINDOW: Duration = Duration::from_secs(4);
/// In-process participants must stamp a first qualified success within this
/// cut of the common armed start; burst (real CLI process) slots share the
/// same start and get the 5s cohort + 5s network envelope plus a named 2s
/// fixture spawn/wakeup/output/scheduling allowance — one fixed boundary per
/// slot class, never an adaptive extension.
const RECOVERY_WINDOW: Duration = Duration::from_secs(3);
const BURST_RECOVERY_WINDOW: Duration = Duration::from_secs(12);
/// Every owned CLI child is watchdogged from its own spawn instant; the
/// recovery verdict can end it earlier via the stop signal.
const CLI_CHILD_WATCHDOG: Duration = Duration::from_secs(12);
/// Collected stream bounds: stdout follows the CLI's own 8MiB document
/// bound (cli/output.rs LIMIT); stderr is free-form diagnostics and gets a
/// documented 1MiB fixture guard. Overflow is a diagnosed defect, never
/// truncated into success.
const CLI_CHILD_STDOUT_LIMIT: u64 = 8 * 1024 * 1024;
const CLI_CHILD_STDERR_LIMIT: u64 = 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const RELEASE_WINDOW: Duration = Duration::from_secs(15);
const CONNECTION_PROBE_ROUNDS: usize = 6;
const HOLDER_CADENCE: Duration = Duration::from_millis(100);
const TUI_LIKE_CADENCE: Duration = Duration::from_millis(50);
/// One attachment per rendered pane: the provisioned TUI keeps four leaves
/// live in its Tab, so it owns four viewer attachments and a controller
/// grant per leaf. The dedicated second-Tab leaves are not in its reconciled
/// set, so they own no TUI attachment.
const TUI_STANDING_ATTACHMENTS: usize = 4;
/// Bounded retry window for transient carrier pressure. `ServerBusy` maps both
/// queue pressure and a saturated worker viewer table, so it is never taken as
/// a capacity oracle on its own: a fill attach that stays refused through this
/// window fails the witness with its actual placed count. Capacity evidence is
/// only the known-live count reaching the boundary plus one distinct valid
/// overflow request receiving the typed refusal — never a duration.
const PRESSURE_RETRY: Duration = Duration::from_secs(3);

/// Capacity table under test. `target` is the authorized C1 profile; `base`
/// documents the pre-change limits with a reduced participant map — same code
/// path, and the standing attachments alone already fill the base viewer
/// table. The authorized map admits exactly two TUI-class tenants: the real
/// provisioned TUI and `tui_like` synthetic participants.
struct Profile {
    sessions: usize,
    connections: usize,
    viewers: usize,
    wait_like: usize,
    tui_like: usize,
    burst_slots: usize,
}

fn profile() -> Profile {
    match std::env::var("ORCHBEE_WITNESS_PROFILE").as_deref() {
        Ok("base") => Profile {
            sessions: 8,
            connections: 16,
            viewers: 8,
            wait_like: 1,
            tui_like: 1,
            burst_slots: 2,
        },
        _ => Profile {
            sessions: 32,
            connections: 48,
            viewers: 24,
            wait_like: 9,
            tui_like: 1,
            burst_slots: 4,
        },
    }
}

/// Outcome of a single capacity-probe attach: admitted, or refused with the
/// `ServerBusy` envelope the transport uses for both queue pressure and a
/// saturated worker viewer table.
enum AttachProbe {
    Admitted(ViewerId),
    Refused,
}

/// Cross-thread outcome ledger. `unknown` and every untyped failure must stay
/// zero: an outcome-unknown or an unclassified refusal under capacity
/// pressure is a defect, never an acceptable busy. `progress` is the
/// per-participant end ledger — every promised participant must advance it,
/// not merely contribute to the aggregate cycle count. `epoch`,
/// `cycle_begin`, and `cycle_end` are phase instrumentation: each participant
/// stamps the millis-since-epoch of its latest cycle start and completion so
/// a stalled participant's in-flight phase is visible at any cut.
struct Counters {
    attached: AtomicUsize,
    detached: AtomicUsize,
    outstanding: AtomicUsize,
    polled_empty: AtomicUsize,
    records: AtomicUsize,
    acquired: AtomicUsize,
    input_accepted: AtomicUsize,
    send_accepted: AtomicUsize,
    rejected: AtomicUsize,
    unavailable: AtomicUsize,
    unknown: AtomicUsize,
    cycles: AtomicUsize,
    progress: Vec<AtomicUsize>,
    epoch: Instant,
    cycle_begin: Vec<AtomicU64>,
    cycle_end: Vec<AtomicU64>,
}

impl Counters {
    fn with_participants(participants: usize) -> Self {
        Self {
            attached: AtomicUsize::new(0),
            detached: AtomicUsize::new(0),
            outstanding: AtomicUsize::new(0),
            polled_empty: AtomicUsize::new(0),
            records: AtomicUsize::new(0),
            acquired: AtomicUsize::new(0),
            input_accepted: AtomicUsize::new(0),
            send_accepted: AtomicUsize::new(0),
            rejected: AtomicUsize::new(0),
            unavailable: AtomicUsize::new(0),
            unknown: AtomicUsize::new(0),
            cycles: AtomicUsize::new(0),
            progress: (0..participants).map(|_| AtomicUsize::new(0)).collect(),
            epoch: Instant::now(),
            cycle_begin: (0..participants).map(|_| AtomicU64::new(0)).collect(),
            cycle_end: (0..participants).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    fn since_epoch(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    fn mark_begin(&self, slot: usize) {
        self.cycle_begin[slot].store(self.since_epoch(), Ordering::Relaxed);
    }

    fn mark_end(&self, slot: usize) {
        self.cycle_end[slot].store(self.since_epoch(), Ordering::Relaxed);
    }

    /// Per-participant phase state at a cut: progress value, last cycle
    /// begin/end stamps — a participant whose begin stamp is newer than its
    /// end stamp was still inside a cycle.
    fn phase_line(&self, slot: usize) -> String {
        format!(
            "progress={} cycle_begin={}ms cycle_end={}ms",
            self.progress[slot].load(Ordering::Relaxed),
            self.cycle_begin[slot].load(Ordering::Relaxed),
            self.cycle_end[slot].load(Ordering::Relaxed),
        )
    }
}

fn bump(counter: &AtomicUsize) {
    counter.fetch_add(1, Ordering::Relaxed);
}

/// Acceptance-44 recovery oracle: one optional armed start plus exactly one
/// `Option<Instant>` first-qualified-success per participant, both owned by a
/// single test-local mutex. The single arm operation clears the slots,
/// samples the common start, and publishes `pause=false` while holding that
/// same lock — serializing the standing (never-pausing) participant's
/// observations against arming, so a completion stamped before the boundary
/// is excluded and any observation recorded after arming is continuity
/// evidence. Refusals and generic cycle-end marks never enter this oracle;
/// verdicts use `Instant` precision, not epoch milliseconds.
struct RecoveryObservation {
    armed_start: Option<Instant>,
    first_success: Vec<Option<Instant>>,
}

impl RecoveryObservation {
    fn new(participants: usize) -> Self {
        Self {
            armed_start: None,
            first_success: vec![None; participants],
        }
    }

    fn arm_at(&mut self, start: Instant) {
        for slot in &mut self.first_success {
            *slot = None;
        }
        self.armed_start = Some(start);
    }

    fn record_at(&mut self, slot: usize, now: Instant) {
        if self.armed_start.is_some() && self.first_success[slot].is_none() {
            self.first_success[slot] = Some(now);
        }
    }

    fn first(&self, slot: usize) -> Option<Instant> {
        self.first_success[slot]
    }

    fn qualified(&self, slot: usize, deadline: Instant) -> bool {
        self.first_success[slot].is_some_and(|stamp| stamp <= deadline)
    }
}

/// Shared recovery state: the pause flag the transient producers gate on,
/// plus the observation oracle. Publishing `pause=false` inside the
/// observation lock is what makes the common start authoritative.
struct Recovery {
    pause: AtomicBool,
    observation: Mutex<RecoveryObservation>,
}

impl Recovery {
    fn new(participants: usize) -> Self {
        Self {
            pause: AtomicBool::new(false),
            observation: Mutex::new(RecoveryObservation::new(participants)),
        }
    }

    /// The single arm operation: clear every slot, sample the common start,
    /// publish `pause=false` — all while holding the observation lock.
    fn arm(&self) -> Instant {
        let mut observation = self.observation.lock().unwrap();
        observation.arm_at(Instant::now());
        self.pause.store(false, Ordering::SeqCst);
        observation.armed_start.unwrap()
    }

    /// Record one qualified served success for `slot` inside the lock, after
    /// checking it is armed; only the first observation is retained.
    fn record_success(&self, slot: usize) {
        let mut observation = self.observation.lock().unwrap();
        observation.record_at(slot, Instant::now());
    }
}

/// How an owned CLI child ended. `Canceled` is the internal stop-triggered
/// teardown — a distinct internal cancellation, never reported as a served,
/// refused, or unknown cycle.
enum CliChildEnd {
    Exited(ExitStatus),
    TimedOut,
    Canceled,
}

/// A spawned child that is always reaped: any early return or unwind still
/// kills and waits it instead of leaking a zombie.
struct OwnedChild(Option<Child>);

impl OwnedChild {
    fn spawn(command: &mut Command) -> io::Result<Self> {
        command.spawn().map(|child| Self(Some(child)))
    }

    fn kill_and_reap(&mut self) {
        if let Some(mut child) = self.0.take() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        self.kill_and_reap();
    }
}

/// Drive a prepared command to an owned conclusion: streams redirected to
/// private regular files (no blocking pipe wait, no pipe-fill deadlock),
/// `try_wait` polled against `stop` and a fixed watchdog measured from
/// spawn. `TimedOut`/`Canceled` both kill and reap the child before
/// returning; only the direct child is killed — CLI queries spawn no
/// descendants and no process group is signaled.
fn run_owned_child(
    command: &mut Command,
    stdout_path: &Path,
    stderr_path: &Path,
    stop: &AtomicBool,
    watchdog: Duration,
) -> Result<CliChildEnd, String> {
    let stdout_file =
        fs::File::create(stdout_path).map_err(|error| format!("child stdout file: {error}"))?;
    let stderr_file =
        fs::File::create(stderr_path).map_err(|error| format!("child stderr file: {error}"))?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    let spawned = Instant::now();
    let mut owned =
        OwnedChild::spawn(command).map_err(|error| format!("spawn CLI child: {error}"))?;
    loop {
        let ended = match owned.0.as_mut().unwrap().try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => {
                if stop.load(Ordering::Relaxed) {
                    owned.kill_and_reap();
                    return Ok(CliChildEnd::Canceled);
                }
                if spawned.elapsed() >= watchdog {
                    owned.kill_and_reap();
                    return Ok(CliChildEnd::TimedOut);
                }
                thread::sleep(Duration::from_millis(20));
                continue;
            }
            Err(error) => return Err(format!("child wait: {error}")),
        };
        owned.0 = None;
        return Ok(CliChildEnd::Exited(ended));
    }
}

/// Collect one private stream file after the child exited, bounded — an
/// overflow is a diagnosed defect, never truncated into success.
fn collect_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let size = fs::metadata(path)
        .map_err(|error| format!("child stream {}: {error}", path.display()))?
        .len();
    if size > limit {
        return Err(format!(
            "child stream {} produced {size}B over the {limit}B bound",
            path.display()
        ));
    }
    fs::read(path).map_err(|error| format!("child stream {}: {error}", path.display()))
}

/// One C1 real CLI invocation: `orchbee` from the staged cohort against the
/// fixture site, owned end-to-end. `stem` names the per-invocation output
/// files under `output_dir`.
fn c1_cli_json(
    fixture: &Fixture,
    output_dir: &Path,
    stem: &str,
    stop: &AtomicBool,
    arguments: &[&str],
) -> Result<(CliChildEnd, Duration, serde_json::Value), String> {
    let stdout_path = output_dir.join(format!("{stem}.out"));
    let stderr_path = output_dir.join(format!("{stem}.err"));
    let mut command = Command::new(fixture.cohort.join("orchbee"));
    command
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
        .env("LANG", "en_US.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES");
    let start = Instant::now();
    let end = run_owned_child(
        &mut command,
        &stdout_path,
        &stderr_path,
        stop,
        CLI_CHILD_WATCHDOG,
    )?;
    let elapsed = start.elapsed();
    let stdout = collect_bounded(&stdout_path, CLI_CHILD_STDOUT_LIMIT)?;
    let stderr = collect_bounded(&stderr_path, CLI_CHILD_STDERR_LIMIT)?;
    let document: serde_json::Value = match &end {
        CliChildEnd::Exited(_) => serde_json::from_slice(&stdout).map_err(|error| {
            format!(
                "CLI JSON document: {error} (stderr: {})",
                String::from_utf8_lossy(&stderr)
            )
        })?,
        _ => serde_json::Value::Null,
    };
    Ok((end, elapsed, document))
}

/// One resolved terminal incarnation usable for AttachBinding and send.
#[derive(Clone)]
struct Binding {
    worker: ObcsWorkerId,
    terminal: ObcsTerminalId,
}

/// One admitted session driving serialized OBCS + OBWP exchanges on a single
/// socket — the same frame-magic demultiplex the real clients use. Dropping
/// the peer closes the socket; the Server releases the session and every
/// attachment it owned on its own schedule.
struct SessionPeer {
    _admitted: ClientSessionConnection,
    writer: DeadlineStream,
    reader: BufReader<DeadlineStream>,
    deadline: Rc<Cell<Instant>>,
    tag: u32,
    next: u64,
}

impl SessionPeer {
    /// `cohort` is loaded once for the whole witness — the staged fixture
    /// cohort is private and immutable, so per-connect re-hashing only adds
    /// load the workload itself does not produce.
    fn connect(
        fixture: &Fixture,
        cohort: &NativeCohort,
        tag: u32,
        deadline: Instant,
    ) -> Result<Self, String> {
        let admitted = ClientSessionConnection::connect_before(
            fixture.site().client_socket_path(),
            cohort.identity(),
            deadline,
        )
        .map_err(|error| format!("admit peer {tag:#x}: {error}"))?;
        let stream = UnixStream::from(
            admitted
                .as_fd()
                .try_clone_to_owned()
                .map_err(|error| format!("clone session fd: {error}"))?,
        );
        let deadline = Rc::new(Cell::new(Instant::now()));
        let reader = BufReader::new(DeadlineStream::new(
            stream
                .try_clone()
                .map_err(|error| format!("clone session stream: {error}"))?,
            deadline.clone(),
        ));
        Ok(Self {
            _admitted: admitted,
            writer: DeadlineStream::new(stream, deadline.clone()),
            reader,
            deadline,
            tag,
            next: 1,
        })
    }

    fn next_encoded(&mut self) -> String {
        let encoded = format!("{:08x}{:024x}", self.tag, self.next);
        self.next += 1;
        encoded
    }

    fn request_id(&mut self) -> Result<RequestId, String> {
        RequestId::parse(&self.next_encoded()).map_err(|error| format!("request id: {error}"))
    }

    fn exchange(
        &mut self,
        message: ClientMessage,
        deadline: Instant,
    ) -> Result<ServerMessage, String> {
        self.deadline.set(deadline);
        write_client_message(&mut self.writer, &message)
            .map_err(|error| format!("request write: {error}"))?;
        read_server_message(&mut self.reader).map_err(|error| format!("result read: {error}"))
    }

    fn viewer(
        &mut self,
        action: TerminalViewerAction,
        deadline: Instant,
    ) -> Result<TerminalViewerOutcome, String> {
        let request_id = self.request_id()?;
        let message =
            ClientMessage::TerminalViewer(TerminalViewerRequest::new(request_id.clone(), action));
        let ServerMessage::TerminalViewerResult(result) = self.exchange(message, deadline)? else {
            return Err("viewer request answered by another result kind".into());
        };
        if result.request_id() != &request_id {
            return Err("viewer result correlation mismatch".into());
        }
        Ok(result.into_outcome())
    }

    fn control(
        &mut self,
        action: TerminalControlAction,
        deadline: Instant,
    ) -> Result<TerminalControlOutcome, String> {
        let request_id = self.request_id()?;
        let message =
            ClientMessage::TerminalControl(TerminalControlRequest::new(request_id.clone(), action));
        let ServerMessage::TerminalControlResult(result) = self.exchange(message, deadline)? else {
            return Err("control request answered by another result kind".into());
        };
        if result.request_id() != &request_id {
            return Err("control result correlation mismatch".into());
        }
        Ok(result.into_outcome())
    }

    fn send(
        &mut self,
        site: &ObcsSiteId,
        binding: &Binding,
        payload: TerminalSendPayload,
        deadline: Instant,
    ) -> Result<TerminalSendOutcome, String> {
        let request_id = self.request_id()?;
        let message = ClientMessage::TerminalSend(TerminalSendRequest::new(
            request_id.clone(),
            site.clone(),
            binding.worker.clone(),
            binding.terminal.clone(),
            payload,
        ));
        loop {
            let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(deadline);
            let ServerMessage::TerminalSendResult(result) =
                self.exchange(message.clone(), per_op)?
            else {
                return Err("send request answered by another result kind".into());
            };
            if result.request_id() != &request_id {
                return Err("send result correlation mismatch".into());
            }
            match result.into_outcome() {
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailableReason::ServerBusy,
                    ..
                } => {
                    if Instant::now() >= deadline {
                        return Err("send stayed ServerBusy past its deadline".into());
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                outcome => return Ok(outcome),
            }
        }
    }

    fn snapshot(&mut self, deadline: Instant) -> Result<Snapshot, String> {
        self.deadline.set(deadline);
        let request_id = WorkbenchRequestId::parse(&self.next_encoded())
            .map_err(|error| format!("request id: {error}"))?;
        write_workbench_request(
            &mut self.writer,
            &WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
            }),
        )
        .map_err(|error| format!("workbench write: {error}"))?;
        let WorkbenchResponse::Snapshot(result) = read_workbench_response(&mut self.reader)
            .map_err(|error| format!("workbench read: {error}"))?
        else {
            return Err("snapshot request answered by another response kind".into());
        };
        if result.request_id != request_id {
            return Err("snapshot correlation mismatch".into());
        }
        let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
            return Err("snapshot unavailable".into());
        };
        Ok(snapshot)
    }

    fn attach(
        &mut self,
        site: &ObcsSiteId,
        binding: &Binding,
        deadline: Instant,
    ) -> Result<Option<ViewerId>, String> {
        match self.viewer(
            TerminalViewerAction::AttachBinding {
                site_id: site.clone(),
                worker_id: binding.worker.clone(),
                terminal_id: binding.terminal.clone(),
            },
            deadline,
        ) {
            Ok(TerminalViewerOutcome::Attached { attachment_id, .. }) => Ok(Some(attachment_id)),
            Ok(
                TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. },
            ) => Ok(None),
            Ok(other) => Err(format!("attach answered {other:?}")),
            Err(error) => Err(error),
        }
    }

    /// One viewer attach attempt for capacity probes. `ServerBusy` maps both a
    /// saturated carrier queue and a full worker viewer table, so the caller
    /// must distinguish sustained refusal from transient queue pressure; a
    /// typed `Rejected` here is a defect — table-full is mapped to
    /// `Unavailable(ServerBusy)` by the transport contract.
    fn attach_probe(
        &mut self,
        site: &ObcsSiteId,
        binding: &Binding,
        deadline: Instant,
    ) -> Result<AttachProbe, String> {
        match self.viewer(
            TerminalViewerAction::AttachBinding {
                site_id: site.clone(),
                worker_id: binding.worker.clone(),
                terminal_id: binding.terminal.clone(),
            },
            deadline,
        )? {
            TerminalViewerOutcome::Attached { attachment_id, .. } => {
                Ok(AttachProbe::Admitted(attachment_id))
            }
            TerminalViewerOutcome::Unavailable {
                reason: ViewerUnavailableReason::ServerBusy,
            } => Ok(AttachProbe::Refused),
            TerminalViewerOutcome::Unavailable { reason } => Err(format!(
                "probe attach unavailable with non-busy reason {reason:?}"
            )),
            TerminalViewerOutcome::Rejected { reason } => Err(format!(
                "probe attach received typed rejection {reason:?} where the \
                 transport contract maps table-full to Unavailable"
            )),
            other => Err(format!("probe attach unexpected outcome {other:?}")),
        }
    }

    /// Repeatedly attach until admitted, refusing only after the refusal has
    /// persisted for `persistence`. Returns the admitted viewer id or
    /// `AttachProbe::Refused` once a `ServerBusy` streak outlives the window.
    /// Each attempt gets a fresh exchange budget bounded by `deadline` — the
    /// phase budget is never reset per retry.
    fn attach_until_saturated(
        &mut self,
        site: &ObcsSiteId,
        binding: &Binding,
        deadline: Instant,
        persistence: Duration,
    ) -> Result<AttachProbe, String> {
        let mut first_refusal: Option<Instant> = None;
        loop {
            let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(deadline);
            match self.attach_probe(site, binding, per_op)? {
                AttachProbe::Admitted(id) => return Ok(AttachProbe::Admitted(id)),
                AttachProbe::Refused => {
                    let since = first_refusal.get_or_insert_with(Instant::now);
                    if since.elapsed() >= persistence || Instant::now() >= deadline {
                        return Ok(AttachProbe::Refused);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    /// Boundary evidence: the attach must keep refusing for `persistence`
    /// before it counts as saturation — one `ServerBusy` can be transient
    /// carrier pressure and never proves a full table. `Ok(None)` is the
    /// proven refusal; `Ok(Some)` means the boundary admitted.
    fn attach_expect_refused(
        &mut self,
        site: &ObcsSiteId,
        binding: &Binding,
        deadline: Instant,
        persistence: Duration,
    ) -> Result<Option<ViewerId>, String> {
        let mut first_refusal: Option<Instant> = None;
        loop {
            let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(deadline);
            match self.attach_probe(site, binding, per_op)? {
                AttachProbe::Admitted(id) => return Ok(Some(id)),
                AttachProbe::Refused => {
                    let since = first_refusal.get_or_insert_with(Instant::now);
                    if since.elapsed() >= persistence {
                        return Ok(None);
                    }
                    if Instant::now() >= deadline {
                        return Err("overflow refusal never persisted".into());
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    /// Detach proves release only on `Detached` — counters decrement only
    /// after this returns `Ok`. `ServerBusy` is the same transient carrier
    /// pressure every other request retries, so it is retried inside
    /// `deadline`; a `Rejected` or non-busy `Unavailable` reply is a typed
    /// failure that never proves the slot was freed.
    fn detach(&mut self, attachment: &ViewerId, deadline: Instant) -> Result<(), String> {
        loop {
            let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(deadline);
            match self.viewer(
                TerminalViewerAction::Detach {
                    attachment_id: attachment.clone(),
                },
                per_op,
            ) {
                Ok(TerminalViewerOutcome::Detached {}) => return Ok(()),
                Ok(TerminalViewerOutcome::Rejected { reason }) => {
                    return Err(format!("detach rejected {reason:?} — release not proven"));
                }
                Ok(TerminalViewerOutcome::Unavailable {
                    reason: ViewerUnavailableReason::ServerBusy,
                }) => {
                    if Instant::now() >= deadline {
                        return Err("detach stayed ServerBusy past its deadline".into());
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Ok(TerminalViewerOutcome::Unavailable { reason }) => {
                    return Err(format!(
                        "detach unavailable {reason:?} — release not proven"
                    ));
                }
                Ok(other) => return Err(format!("detach answered {other:?}")),
                Err(error) => return Err(error),
            }
        }
    }

    /// Acquire a controller grant on this attachment. `Some` means the grant
    /// was issued; `None` means the terminal already has another controller
    /// — legitimate pressure while another participant owns that terminal.
    /// `ServerBusy` is transient carrier pressure, retried inside `deadline`.
    fn acquire(
        &mut self,
        attachment: &ViewerId,
        deadline: Instant,
    ) -> Result<Option<ControllerId>, String> {
        loop {
            let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(deadline);
            match self.control(
                TerminalControlAction::Acquire {
                    attachment_id: attachment.clone(),
                },
                per_op,
            )? {
                TerminalControlOutcome::Acquired { controller_id } => {
                    return Ok(Some(controller_id));
                }
                TerminalControlOutcome::Rejected { .. } => return Ok(None),
                TerminalControlOutcome::Unavailable {
                    reason: ControlUnavailableReason::ServerBusy,
                } => {
                    if Instant::now() >= deadline {
                        return Err("acquire stayed ServerBusy past its deadline".into());
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                other => return Err(format!("acquire answered {other:?}")),
            }
        }
    }

    /// One input record on a held grant. Anything but `InputAccepted` is a
    /// defect: an acquired grant accepts input, never silently drops it.
    /// `ServerBusy` is transient carrier pressure, retried inside `deadline`.
    fn input(
        &mut self,
        attachment: &ViewerId,
        controller_id: &ControllerId,
        deadline: Instant,
    ) -> Result<(), String> {
        let bytes = ControlInput::new(b"true\r".to_vec())
            .map_err(|error| format!("control input: {error}"))?;
        loop {
            let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(deadline);
            match self.control(
                TerminalControlAction::Input {
                    attachment_id: attachment.clone(),
                    controller_id: controller_id.clone(),
                    sequence: NonZeroU64::MIN,
                    bytes: bytes.clone(),
                },
                per_op,
            )? {
                TerminalControlOutcome::InputAccepted {} => return Ok(()),
                TerminalControlOutcome::Unavailable {
                    reason: ControlUnavailableReason::ServerBusy,
                } => {
                    if Instant::now() >= deadline {
                        return Err("input stayed ServerBusy past its deadline".into());
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                other => return Err(format!("input answered {other:?}")),
            }
        }
    }
}

/// Presentation bindings under one Tab — one per bound terminal in that Tab.
fn tab_bindings(snapshot: &Snapshot, tab_id: &WorkbenchTabId) -> Result<Vec<Binding>, String> {
    fn visit(node: &PanelNode, rows: &mut Vec<Binding>) -> Result<(), String> {
        match node {
            PanelNode::Panel {
                primary_presentation,
                child_presentations,
                ..
            } => {
                for presentation in
                    std::iter::once(primary_presentation).chain(child_presentations.iter())
                {
                    if let Some(binding) = &presentation.binding {
                        rows.push(Binding {
                            worker: ObcsWorkerId::parse(binding.worker_id.as_str())
                                .map_err(|error| format!("worker id: {error}"))?,
                            terminal: ObcsTerminalId::parse(binding.terminal_id.as_str())
                                .map_err(|error| format!("terminal id: {error}"))?,
                        });
                    }
                }
            }
            PanelNode::Split { children, .. } => {
                for slot in children.iter() {
                    visit(&slot.node, rows)?;
                }
            }
        }
        Ok(())
    }
    let mut rows = Vec::new();
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            if &tab.tab_id == tab_id {
                visit(&tab.root, &mut rows)?;
            }
        }
    }
    Ok(rows)
}

fn site_id(snapshot: &Snapshot) -> Result<ObcsSiteId, String> {
    ObcsSiteId::parse(snapshot.site_id.as_str()).map_err(|error| format!("site id: {error}"))
}

/// Shared lane context: the same ambient references every participant lane
/// needs, passed as one struct instead of repeated argument lists.
struct LaneCtx<'a> {
    fixture: &'a Fixture,
    cohort: &'a NativeCohort,
    site: &'a ObcsSiteId,
    ready: &'a AtomicUsize,
    paused: &'a AtomicUsize,
    stop: &'a AtomicBool,
    recovery: &'a Recovery,
    counters: &'a Counters,
}

/// Automated-worker-like resident client: transient attach → poll → detach
/// cycles at a fixed cadence, one outstanding request at a time.
fn wait_like(slot: usize, index: usize, rows: &[Binding], ctx: &LaneCtx<'_>) -> Result<(), String> {
    let &LaneCtx {
        fixture,
        cohort,
        site,
        ready,
        paused,
        stop,
        recovery,
        counters,
    } = ctx;
    let mut peer = SessionPeer::connect(
        fixture,
        cohort,
        0x1000 + index as u32,
        Instant::now() + PROBE_TIMEOUT,
    )?;
    let mut acked = false;
    ready.fetch_add(1, Ordering::Relaxed);
    while !stop.load(Ordering::Relaxed) {
        if recovery.pause.load(Ordering::Relaxed) {
            // Quiescence acknowledgement: this participant holds no transient
            // attachment once it reaches this branch, so its ack means the
            // viewer table can no longer grow from it.
            if !acked {
                acked = true;
                paused.fetch_add(1, Ordering::Relaxed);
            }
            thread::sleep(Duration::from_millis(20));
            continue;
        }
        acked = false;
        counters.mark_begin(slot);
        let deadline = Instant::now() + EXCHANGE_TIMEOUT;
        match peer.attach(site, &rows[index % rows.len()], deadline)? {
            Some(attachment) => {
                bump(&counters.attached);
                bump(&counters.outstanding);
                // Only a served observation (Empty or Record) proves the
                // cycle was serviced; typed refusals are counted separately
                // and never satisfy the progress witness.
                let served = match peer.viewer(
                    TerminalViewerAction::Poll {
                        attachment_id: attachment.clone(),
                    },
                    deadline,
                )? {
                    TerminalViewerOutcome::Empty {} => {
                        bump(&counters.polled_empty);
                        true
                    }
                    TerminalViewerOutcome::Record(_) => {
                        bump(&counters.records);
                        true
                    }
                    TerminalViewerOutcome::Rejected { .. } => {
                        bump(&counters.rejected);
                        false
                    }
                    TerminalViewerOutcome::Unavailable { .. } => {
                        bump(&counters.unavailable);
                        false
                    }
                    other => return Err(format!("poll answered {other:?}")),
                };
                peer.detach(&attachment, deadline)?;
                counters.outstanding.fetch_sub(1, Ordering::Relaxed);
                bump(&counters.detached);
                bump(&counters.cycles);
                if served {
                    bump(&counters.progress[slot]);
                    recovery.record_success(slot);
                }
            }
            None => bump(&counters.unavailable),
        }
        counters.mark_end(slot);
        thread::sleep(HOLDER_CADENCE);
    }
    Ok(())
}

/// Synthetic TUI tenant: four standing attachments plus one controller grant,
/// round-robin polls — the observer lane's steady-state tenant. Three views
/// come from the shared Tab; the fourth is its dedicated controller terminal,
/// so the participant never races the provisioned TUI's per-leaf Acquire. It
/// never pauses: the probes rely on its attachments staying put.
fn tui_like(
    slot: usize,
    index: usize,
    rows: &[Binding],
    dedicated: &Binding,
    ctx: &LaneCtx<'_>,
) -> Result<(), String> {
    let &LaneCtx {
        fixture,
        cohort,
        site,
        ready,
        stop,
        recovery,
        counters,
        ..
    } = ctx;
    let mut peer = SessionPeer::connect(
        fixture,
        cohort,
        0x2000 + index as u32,
        Instant::now() + PROBE_TIMEOUT,
    )?;
    // Each setup attach tolerates transient carrier pressure inside the
    // overall phase deadline — only a refusal that persists past the
    // pressure window is a defect; the phase budget never resets per retry.
    let setup_deadline = Instant::now() + PROBE_TIMEOUT;
    let mut held = Vec::new();
    for binding in rows.iter().take(3) {
        match peer.attach_until_saturated(site, binding, setup_deadline, PRESSURE_RETRY)? {
            AttachProbe::Admitted(attachment) => {
                bump(&counters.attached);
                bump(&counters.outstanding);
                held.push(attachment);
            }
            AttachProbe::Refused => {
                return Err("tui-like standing attach refused persistently".into());
            }
        }
    }
    // The dedicated terminal's viewer is the fourth standing attachment.
    let dedicated_viewer =
        match peer.attach_until_saturated(site, dedicated, setup_deadline, PRESSURE_RETRY)? {
            AttachProbe::Admitted(attachment) => {
                bump(&counters.attached);
                bump(&counters.outstanding);
                attachment
            }
            AttachProbe::Refused => {
                return Err("dedicated tui-like attach refused persistently".into());
            }
        };
    held.push(dedicated_viewer.clone());
    let result = (|| {
        // One dedicated controller grant on the dedicated terminal — the
        // provisioned TUI never reconciles the second Tab, so the only
        // allowed contention is the typed ControllerBusy reason (or transient
        // ServerBusy pressure); every other typed reason or outcome fails
        // immediately instead of looping.
        let grant_deadline = Instant::now() + PROBE_TIMEOUT;
        loop {
            let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(grant_deadline);
            match peer.control(
                TerminalControlAction::Acquire {
                    attachment_id: dedicated_viewer.clone(),
                },
                per_op,
            )? {
                TerminalControlOutcome::Acquired { .. } => {
                    bump(&counters.acquired);
                    ready.fetch_add(1, Ordering::Relaxed);
                    break;
                }
                TerminalControlOutcome::Rejected {
                    reason: ControlRejectedReason::ControllerBusy,
                }
                | TerminalControlOutcome::Unavailable {
                    reason: ControlUnavailableReason::ServerBusy,
                } => {
                    if Instant::now() >= grant_deadline {
                        return Err("tui-like dedicated acquire stayed contended".into());
                    }
                    bump(&counters.rejected);
                    thread::sleep(Duration::from_millis(100));
                }
                other => {
                    return Err(format!(
                        "dedicated acquire answered an unexpected outcome {other:?}"
                    ));
                }
            }
        }
        while !stop.load(Ordering::Relaxed) {
            counters.mark_begin(slot);
            // A round only counts as progress when at least one poll was
            // actually served — typed refusals alone are not progress.
            let mut served = false;
            for attachment in &held {
                match peer.viewer(
                    TerminalViewerAction::Poll {
                        attachment_id: attachment.clone(),
                    },
                    Instant::now() + EXCHANGE_TIMEOUT,
                )? {
                    TerminalViewerOutcome::Empty {} => {
                        served = true;
                        bump(&counters.polled_empty);
                    }
                    TerminalViewerOutcome::Record(_) => {
                        served = true;
                        bump(&counters.records);
                    }
                    TerminalViewerOutcome::Rejected { .. } => bump(&counters.rejected),
                    TerminalViewerOutcome::Unavailable { .. } => bump(&counters.unavailable),
                    other => return Err(format!("poll answered {other:?}")),
                }
            }
            bump(&counters.cycles);
            if served {
                bump(&counters.progress[slot]);
                recovery.record_success(slot);
            }
            counters.mark_end(slot);
            thread::sleep(TUI_LIKE_CADENCE);
        }
        Ok(())
    })();
    // Teardown releases the standing attachments; when the service loop
    // already failed, detach is best-effort so the original participant
    // failure is never masked by a teardown error.
    for attachment in &held {
        let deadline = Instant::now() + EXCHANGE_TIMEOUT;
        match peer.detach(attachment, deadline) {
            Ok(()) => {
                counters.outstanding.fetch_sub(1, Ordering::Relaxed);
                bump(&counters.detached);
            }
            Err(error) if result.is_ok() => return Err(error),
            Err(error) => {
                eprintln!("tui-like teardown detach after failure: {error}");
            }
        }
    }
    result
}

/// Coordinator: serial interactive-class cycles — workbench snapshot, attach,
/// acquire, one input record, detach — the lane that must stay served while
/// observer load saturates. Its dedicated terminal is never claimed by the
/// provisioned TUI, so every cycle's Acquire is a real grant, and accepted
/// input is only counted after `InputAccepted` actually arrives.
fn coordinator(slot: usize, dedicated: &Binding, ctx: &LaneCtx<'_>) -> Result<(), String> {
    let &LaneCtx {
        fixture,
        cohort,
        site,
        ready,
        paused,
        stop,
        recovery,
        counters,
    } = ctx;
    let mut peer = SessionPeer::connect(fixture, cohort, 0x3000, Instant::now() + PROBE_TIMEOUT)?;
    let mut acked = false;
    ready.fetch_add(1, Ordering::Relaxed);
    while !stop.load(Ordering::Relaxed) {
        if recovery.pause.load(Ordering::Relaxed) {
            if !acked {
                acked = true;
                paused.fetch_add(1, Ordering::Relaxed);
            }
            thread::sleep(Duration::from_millis(20));
            continue;
        }
        acked = false;
        counters.mark_begin(slot);
        let deadline = Instant::now() + EXCHANGE_TIMEOUT;
        peer.snapshot(deadline)?;
        if let Some(attachment) = peer.attach(site, dedicated, deadline)? {
            bump(&counters.attached);
            bump(&counters.outstanding);
            // The dedicated terminal is exclusively this participant's, so a
            // refused grant is a defect — never silently accepted busy.
            match peer.acquire(&attachment, deadline)? {
                Some(controller_id) => {
                    bump(&counters.acquired);
                    peer.input(&attachment, &controller_id, deadline)?;
                    bump(&counters.input_accepted);
                }
                None => {
                    return Err("coordinator dedicated acquire refused — not a busy class".into());
                }
            }
            peer.detach(&attachment, deadline)?;
            counters.outstanding.fetch_sub(1, Ordering::Relaxed);
            bump(&counters.detached);
            bump(&counters.cycles);
            bump(&counters.progress[slot]);
            recovery.record_success(slot);
        } else {
            bump(&counters.unavailable);
        }
        counters.mark_end(slot);
        thread::sleep(Duration::from_millis(150));
    }
    Ok(())
}

/// Burst: real `terminal read` invocations — each a transient session plus a
/// transient attach — launched in parallel slots. The only legitimate busy is
/// a typed failure document pinned to an owned capacity surface:
/// `session-capacity` at the `admission` stage (the 32-session table) or
/// `service-unavailable` at the `viewer` stage (the 24-viewer table).
/// `outcome-unknown` is a defect; every other exit code or document is a
/// possible protocol failure and fails the witness.
fn burst(
    slot: usize,
    index: usize,
    terminal_ids: &[String],
    output_dir: &Path,
    ctx: &LaneCtx<'_>,
) -> Result<(), String> {
    let &LaneCtx {
        fixture,
        ready,
        paused,
        stop,
        recovery,
        counters,
        ..
    } = ctx;
    let terminal = &terminal_ids[index % terminal_ids.len()];
    let mut acked = false;
    let mut cycle = 0usize;
    ready.fetch_add(1, Ordering::Relaxed);
    while !stop.load(Ordering::Relaxed) {
        if recovery.pause.load(Ordering::Relaxed) {
            // Reaching this branch means the previous CLI child has already
            // exited — its transient session/viewer slots can only drain now.
            if !acked {
                acked = true;
                paused.fetch_add(1, Ordering::Relaxed);
            }
            thread::sleep(Duration::from_millis(20));
            continue;
        }
        acked = false;
        counters.mark_begin(slot);
        eprintln!(
            "burst[{index}] spawn terminal read at {}ms",
            counters.since_epoch()
        );
        cycle += 1;
        let (end, child_elapsed, document) = c1_cli_json(
            fixture,
            output_dir,
            &format!("burst-{index}-{cycle}"),
            stop,
            &["terminal", "read", terminal, "--format", "json"],
        )?;
        let kind = document["failure"]["kind"].as_str().unwrap_or_default();
        let stage = document["failure"]["details"]["stage"]
            .as_str()
            .unwrap_or_default();
        match end {
            CliChildEnd::Exited(status) => {
                eprintln!(
                    "burst[{index}] exit after {child_elapsed:?} code={:?} \
                     kind={kind:?} stage={stage:?}",
                    status.code()
                );
                match status.code() {
                    Some(0) => {
                        bump(&counters.records);
                        bump(&counters.cycles);
                        bump(&counters.progress[slot]);
                        recovery.record_success(slot);
                    }
                    Some(4) if kind == "session-capacity" && stage == "admission" => {
                        bump(&counters.unavailable)
                    }
                    Some(4) if kind == "service-unavailable" && stage == "viewer" => {
                        bump(&counters.unavailable)
                    }
                    Some(8) => bump(&counters.unknown),
                    other => {
                        return Err(format!(
                            "terminal read exit {other:?} kind={kind:?} \
                             stage={stage:?}: {document}"
                        ));
                    }
                }
            }
            // Internal stop-triggered teardown: never a served, refused, or
            // unknown cycle — just end the participant.
            CliChildEnd::Canceled => break,
            CliChildEnd::TimedOut => {
                return Err(format!(
                    "burst[{index}] child exceeded the {CLI_CHILD_WATCHDOG:?} watchdog"
                ));
            }
        }
        counters.mark_end(slot);
    }
    Ok(())
}

/// Provision four live terminals: a real TUI (provisioner and TUI1 in one)
/// plus three workbench splits on the standalone Tab, each leaf confirmed
/// live by its own rendered frame before the load map starts. The returned
/// peer is held by the caller for the whole run — it is one counted tenant.
fn provision(
    fixture: &Fixture,
    tui: &mut Client,
) -> Result<(WorkbenchPeer, ObcsSiteId, Vec<Binding>, Binding, Binding), String> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut peer = WorkbenchPeer::connect(fixture, deadline).map_err(|error| error.to_string())?;
    let snapshot = peer.snapshot(deadline).map_err(|error| error.to_string())?;
    let context_id = snapshot.navigation.standalone_terminal_context_id.clone();
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == context_id)
        .ok_or("standalone context missing")?;
    let tab = context.tabs.first().ok_or("standalone tab missing")?;
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        return Err("fresh standalone tab was already split".into());
    };
    let tab_id = tab.tab_id.clone();
    // Proven 2x2 layout (same shape as the multiview witness): leaf titles
    // keep their labels readable instead of truncating four narrow columns.
    let panel_a = panel_id.clone();
    let split = |peer: &mut WorkbenchPeer,
                 target: &WorkbenchPanelId,
                 direction: WorkbenchPanelSplitDirection,
                 label: &str|
     -> Result<WorkbenchPanelId, String> {
        match peer
            .prepare(
                WorkbenchAction::PanelSplit {
                    terminal_context_id: context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: target.clone(),
                    direction,
                    label: WorkbenchLabel::parse(label).map_err(|error| error.to_string())?,
                    geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)
                        .map_err(|error| error.to_string())?,
                    launch: None,
                },
                deadline,
            )
            .map_err(|error| error.to_string())?
        {
            WorkbenchCommandOutcome::PanelSplitCreated { new_panel_id, .. } => Ok(new_panel_id),
            outcome => Err(format!("split {label}: {outcome:?}")),
        }
    };
    let panel_b = split(
        &mut peer,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
        "W2",
    )?;
    native_multiview::wait_leaf_live(tui, "W2", Instant::now() + PROBE_TIMEOUT);
    split(
        &mut peer,
        &panel_b,
        WorkbenchPanelSplitDirection::Below,
        "W3",
    )?;
    native_multiview::wait_leaf_live(tui, "W3", Instant::now() + PROBE_TIMEOUT);
    split(
        &mut peer,
        &panel_a,
        WorkbenchPanelSplitDirection::Below,
        "W4",
    )?;
    native_multiview::wait_leaf_live(tui, "W4", Instant::now() + PROBE_TIMEOUT);

    // Dedicated controller terminals: a second Tab whose leaves stay outside
    // the provisioned TUI's reconciled set, so its controller grants are never
    // taken by the TUI's per-leaf Acquire. The TUI is pinned back to Tab 1 in
    // case the new Tab steals focus.
    let (tab2_id, panel_d1) = match peer
        .prepare(
            WorkbenchAction::TabCreate {
                terminal_context_id: context_id.clone(),
                label: WorkbenchLabel::parse("D").map_err(|error| error.to_string())?,
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)
                    .map_err(|error| error.to_string())?,
                launch: None,
            },
            deadline,
        )
        .map_err(|error| error.to_string())?
    {
        WorkbenchCommandOutcome::TabCreated {
            tab_id, panel_id, ..
        }
        | WorkbenchCommandOutcome::TabCreatePending {
            tab_id, panel_id, ..
        } => (tab_id, panel_id),
        outcome => return Err(format!("dedicated tab: {outcome:?}")),
    };
    tui.send(b"\x021");
    // A pending Tab's leaf is not splittable until it commits — retry the
    // split inside the same bounded window instead of racing the commit.
    let d2_deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match peer
            .prepare(
                WorkbenchAction::PanelSplit {
                    terminal_context_id: context_id.clone(),
                    tab_id: tab2_id.clone(),
                    panel_id: panel_d1.clone(),
                    direction: WorkbenchPanelSplitDirection::Below,
                    label: WorkbenchLabel::parse("D2").map_err(|error| error.to_string())?,
                    geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)
                        .map_err(|error| error.to_string())?,
                    launch: None,
                },
                deadline,
            )
            .map_err(|error| error.to_string())?
        {
            WorkbenchCommandOutcome::PanelSplitCreated { .. }
            | WorkbenchCommandOutcome::PanelSplitPending { .. } => break,
            WorkbenchCommandOutcome::Rejected { .. } if Instant::now() < d2_deadline => {
                thread::sleep(Duration::from_millis(100));
            }
            outcome => return Err(format!("split D2: {outcome:?}")),
        }
    }

    // Both dedicated leaves must be bound terminals before their tenants
    // attach — a missing binding means no live shell, not a slow one.
    let bind_deadline = Instant::now() + PROBE_TIMEOUT;
    let (site, rows, dedicated) = loop {
        let snapshot = peer.snapshot(deadline).map_err(|error| error.to_string())?;
        let dedicated = tab_bindings(&snapshot, &tab2_id)?;
        if dedicated.len() == 2 {
            let site = site_id(&snapshot)?;
            let rows = tab_bindings(&snapshot, &tab_id)?;
            break (site, rows, dedicated);
        }
        if Instant::now() >= bind_deadline {
            return Err("dedicated terminals never bound".into());
        }
        thread::sleep(Duration::from_millis(100));
    };
    if rows.len() != 4 {
        return Err(format!(
            "expected four bound terminals, found {}",
            rows.len()
        ));
    }
    let mut dedicated = dedicated.into_iter();
    let dedicated_tui = dedicated.next().expect("two dedicated bindings");
    let dedicated_coordinator = dedicated.next().expect("two dedicated bindings");
    Ok((peer, site, rows, dedicated_tui, dedicated_coordinator))
}

/// Exactly one managed cohort process of `name`, retrying a transient scan.
fn one_process(fixture: &Fixture, name: &str) -> ManagedProcess {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match fixture.process_handles(name) {
            Ok(mut handles) if handles.len() == 1 => return handles.remove(0),
            Ok(handles) => panic!(
                "{name}: expected one managed process, found {}",
                handles.len()
            ),
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            Err(error) => panic!("{name}: managed process scan: {error}"),
        }
    }
}

/// The named cohort process never exited during the witness.
fn assert_process_retained(name: &str, process: &ManagedProcess) {
    match process.exited() {
        Ok(false) => {}
        Ok(true) => panic!("{name} exited during the witness"),
        Err(error) => panic!("{name} exit check: {error}"),
    }
}

/// The authorized load map sustains typed service while every owned boundary
/// refuses overflow before effect — and releases every slot on teardown.
#[test]
fn c1_capacity_process_witness() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let profile = profile();
    let mut tui = Client::spawn(&fixture, "en_US.UTF-8");
    tui.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let server = one_process(&fixture, "orchbee-server");
    let worker = one_process(&fixture, "orchbee-worker");
    let host = one_process(&fixture, "orchbee-terminal-host");

    let (_provisioner, site, rows, dedicated_tui, dedicated_coordinator) =
        provision(&fixture, &mut tui).expect("provision terminals");
    let terminal_ids: Vec<String> = rows
        .iter()
        .map(|binding| binding.terminal.as_str().to_owned())
        .collect();
    // Held sessions for the whole run, counted exactly:
    //   1 real TUI client + 1 workbench provisioner peer
    // + `tui_like` synthetic tenants (1 each)
    // + `wait_like` synthetic tenants (1 each)
    // + 1 coordinator + 1 probe peer admitted below.
    // Transient burst CLI sessions are paused before any boundary fill.
    let live_sessions = 2 + profile.tui_like + profile.wait_like + 2;
    // The staged fixture cohort is private and immutable for the run — its
    // identity is loaded once and reused for every admission so the witness
    // does not re-hash ~170MiB of binaries per connect under load.
    let cohort = NativeCohort::load(&fixture.cohort, Instant::now() + PROBE_TIMEOUT)
        .expect("cohort identity");
    // Admit the probe before participants so session-cap arithmetic is exact
    // even when the map already sits at the boundary.
    let mut probe = SessionPeer::connect(&fixture, &cohort, 0x4000, Instant::now() + PROBE_TIMEOUT)
        .expect("probe peer");

    let stop = AtomicBool::new(false);
    let ready = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let paused = AtomicUsize::new(0);
    let participants = profile.wait_like + 1 + profile.tui_like + profile.burst_slots;
    // Transient producers whose quiescence must be acknowledged before any
    // exact fill: wait_like + coordinator + burst. tui_like is a standing
    // observer and never pauses.
    let transient = profile.wait_like + 1 + profile.burst_slots;
    let counters = Counters::with_participants(participants);
    let recovery = Recovery::new(participants);
    // Private per-invocation stream files for the owned CLI children.
    let child_output = fixture.base.join("c1-cli-children");
    fs::create_dir_all(&child_output).expect("child output directory");
    let socket = fixture.site().client_socket_path().to_owned();

    let ctx = LaneCtx {
        fixture: &fixture,
        cohort: &cohort,
        site: &site,
        ready: &ready,
        paused: &paused,
        stop: &stop,
        recovery: &recovery,
        counters: &counters,
    };
    thread::scope(|scope| {
        // Participant closures are `move`: rebind shared state as references
        // so each thread captures a Copy handle instead of the owned value.
        let fixture = &fixture;
        let cohort = &cohort;
        let site = &site;
        let rows = &rows;
        let dedicated_tui = &dedicated_tui;
        let dedicated_coordinator = &dedicated_coordinator;
        let terminal_ids = &terminal_ids;
        let child_output = &child_output;
        let ready = &ready;
        let failed = &failed;
        let paused = &paused;
        let stop = &stop;
        let recovery = &recovery;
        let counters = &counters;
        let ctx = &ctx;
        let mut joins = Vec::new();
        let mut slot = 0usize;
        for index in 0..profile.wait_like {
            let participant = slot;
            slot += 1;
            joins.push(scope.spawn(move || {
                wait_like(participant, index, rows, ctx).map_err(|error| {
                    eprintln!("wait_like {index}: {error}");
                    failed.fetch_add(1, Ordering::Relaxed);
                    error
                })
            }));
        }
        let participant = slot;
        slot += 1;
        joins.push(scope.spawn(move || {
            coordinator(participant, dedicated_coordinator, ctx).map_err(|error| {
                eprintln!("coordinator: {error}");
                failed.fetch_add(1, Ordering::Relaxed);
                error
            })
        }));
        for index in 0..profile.tui_like {
            let participant = slot;
            slot += 1;
            joins.push(scope.spawn(move || {
                tui_like(participant, index, rows, dedicated_tui, ctx).map_err(|error| {
                    eprintln!("tui_like {index}: {error}");
                    failed.fetch_add(1, Ordering::Relaxed);
                    error
                })
            }));
        }
        for index in 0..profile.burst_slots {
            let participant = slot;
            slot += 1;
            joins.push(scope.spawn(move || {
                burst(participant, index, terminal_ids, child_output, ctx).map_err(|error| {
                    eprintln!("burst {index}: {error}");
                    failed.fetch_add(1, Ordering::Relaxed);
                    error
                })
            }));
        }

        let witness = (|| -> Result<(), String> {
            // Admission itself is serial server work: the warm window only
            // starts once every promised participant is actually in service,
            // so connect latency can never be misread as stalled progress.
            let ready_deadline = Instant::now() + PROBE_TIMEOUT * 2;
            while ready.load(Ordering::Relaxed) < participants {
                if failed.load(Ordering::Relaxed) > 0 {
                    return Err(format!(
                        "{} participant(s) failed during admission",
                        failed.load(Ordering::Relaxed)
                    ));
                }
                if Instant::now() >= ready_deadline {
                    return Err(format!(
                        "participants never all entered service: {}/{}",
                        ready.load(Ordering::Relaxed),
                        participants
                    ));
                }
                thread::sleep(Duration::from_millis(50));
            }
            let warm_deadline = Instant::now() + WARM_WINDOW;
            while Instant::now() < warm_deadline {
                thread::sleep(Duration::from_millis(50));
            }
            // End-ledger first half: every promised participant — not only the
            // aggregate — made progress inside the warm window.
            for (participant, progressed) in counters.progress.iter().enumerate() {
                if progressed.load(Ordering::Relaxed) == 0 {
                    return Err(format!("participant {participant} made no warm progress"));
                }
            }

            // Freeze transient traffic, then require a real quiescence
            // acknowledgement from every transient producer — a participant
            // that observed pause=false but has not finished its in-flight
            // cycle can still grow the table, so the outstanding counter
            // alone is not proof. Burst participants ack only after their
            // in-flight CLI child has exited.
            recovery.pause.store(true, Ordering::Relaxed);
            let quiesce_deadline = Instant::now() + PROBE_TIMEOUT;
            while paused.load(Ordering::Relaxed) < transient {
                if failed.load(Ordering::Relaxed) > 0 {
                    return Err("a participant failed while quiescing".into());
                }
                if Instant::now() >= quiesce_deadline {
                    return Err(format!(
                        "transient producers never all quiesced: {}/{transient}",
                        paused.load(Ordering::Relaxed)
                    ));
                }
                thread::sleep(Duration::from_millis(25));
            }
            // After quiescence the table must sit exactly at the standing
            // baseline — each synthetic TUI holds three shared viewers plus
            // its dedicated one.
            let standing = profile.tui_like * 4;
            let drain_deadline = Instant::now() + PROBE_TIMEOUT;
            while counters.outstanding.load(Ordering::Relaxed) != standing {
                if Instant::now() >= drain_deadline {
                    return Err(format!(
                        "attachments settled at {}, expected standing {standing}",
                        counters.outstanding.load(Ordering::Relaxed)
                    ));
                }
                thread::sleep(Duration::from_millis(25));
            }

            // Boundary conclusions are only valid while every resident is
            // healthy: a failed participant or a dead managed process
            // invalidates the witness outright.
            if failed.load(Ordering::Relaxed) > 0 {
                return Err("a participant failed before the boundary probes".into());
            }
            assert_process_retained("orchbee-server", &server);
            assert_process_retained("orchbee-worker", &worker);
            assert_process_retained("orchbee-terminal-host", &host);

            // Viewer cap: the authorized table is 24. Two four-view TUIs
            // already stand at 8; the remaining 16 are placed as
            // (session, binding, attachment) tuples — four distinct Client
            // Sessions each holding the four shared row targets. A duplicate
            // (session, target) attach is AlreadyAttached before it reaches
            // the Worker and one session hard-caps at eight, so legitimate
            // ownership requires distinct sessions — and a refusal here is
            // bounded pressure or a defect, never inferred capacity.
            let deadline = Instant::now() + PROBE_TIMEOUT;
            if standing + TUI_STANDING_ATTACHMENTS + 16 <= profile.viewers {
                let mut fill_peers: Vec<SessionPeer> = Vec::new();
                for index in 0..3 {
                    fill_peers.push(SessionPeer::connect(
                        fixture,
                        cohort,
                        0x4100 + index as u32,
                        Instant::now() + PROBE_TIMEOUT,
                    )?);
                }
                let mut probe_viewers: Vec<ViewerId> = Vec::new();
                let mut temp_viewers: Vec<Vec<ViewerId>> = vec![Vec::new(); fill_peers.len()];
                let fill_rows = |peer: &mut SessionPeer,
                                 owned: &mut Vec<ViewerId>,
                                 placed_base: usize,
                                 count: usize|
                 -> Result<(), String> {
                    for binding in rows.iter().take(count) {
                        match peer.attach_until_saturated(
                            site,
                            binding,
                            deadline,
                            PRESSURE_RETRY,
                        )? {
                            AttachProbe::Admitted(attachment) => {
                                bump(&counters.attached);
                                bump(&counters.outstanding);
                                owned.push(attachment);
                            }
                            AttachProbe::Refused => {
                                return Err(format!(
                                    "viewer fill refused at {} + {standing} known live — \
                                 pressure or defect, not capacity",
                                    placed_base + owned.len()
                                ));
                            }
                        }
                    }
                    Ok(())
                };
                fill_rows(&mut probe, &mut probe_viewers, 0, 4)?;
                let mut placed = probe_viewers.len();
                for (index, peer) in fill_peers.iter_mut().enumerate() {
                    let mut owned = Vec::new();
                    fill_rows(peer, &mut owned, placed, 4)?;
                    placed += owned.len();
                    temp_viewers[index] = owned;
                }
                if placed + standing + TUI_STANDING_ATTACHMENTS != profile.viewers {
                    return Err(format!(
                        "viewer fill placed {placed} + {standing} + \
                     {TUI_STANDING_ATTACHMENTS}, expected {}",
                        profile.viewers
                    ));
                }

                // Boundary evidence: exactly 24 known live attachments, then one
                // distinct valid request — the probe holds four of its session's
                // eight slots and a fifth distinct target — must receive a
                // refusal that persists, so transient carrier pressure can
                // never impersonate a full table.
                if let Some(attachment) =
                    probe.attach_expect_refused(site, dedicated_tui, deadline, PRESSURE_RETRY)?
                {
                    let _ = probe.detach(&attachment, deadline);
                    return Err("viewer table admitted past the authorized 24".into());
                }

                // Interactive lane under a full observer table: one send must be
                // accepted — the temporary input attachment is a separate
                // admission, and its effect must reach the shell.
                let marker = fixture.base.join("c1-send-marker");
                let payload = TerminalSendPayload::new_text(
                    SendText::new(format!("printf W > {}", marker.display()))
                        .map_err(|error| format!("send text: {error}"))?,
                    true,
                )
                .map_err(|error| format!("send payload: {error}"))?;
                match probe.send(site, &rows[0], payload, deadline)? {
                    TerminalSendOutcome::Accepted { .. } => bump(&counters.send_accepted),
                    other => return Err(format!("send under full viewer table: {other:?}")),
                }
                let marker_deadline = Instant::now() + OBSERVATION_TIMEOUT;
                while !marker.exists() {
                    if Instant::now() >= marker_deadline {
                        return Err("accepted send never reached the shell".into());
                    }
                    thread::sleep(Duration::from_millis(25));
                }

                // Release witness: detaching one known slot must readmit a
                // distinct valid attach.
                let released = probe_viewers.pop().expect("probe holds attachments");
                probe.detach(&released, deadline)?;
                counters.outstanding.fetch_sub(1, Ordering::Relaxed);
                let AttachProbe::Admitted(readmitted) =
                    probe.attach_until_saturated(site, &rows[3], deadline, PRESSURE_RETRY)?
                else {
                    return Err("released viewer slot did not readmit".into());
                };
                bump(&counters.outstanding);
                probe_viewers.push(readmitted);

                // Abrupt-drop witness (still quiesced): one separate leaver
                // session holds 1 and the other owners hold 15 = 4+4+4+3 across
                // four sessions — with standing 8 that is 24 known live. Dropping
                // the leaver frees exactly one slot.
                // Give up one known slot so the leaver's attach is the last
                // legitimate admission to 24.
                let spare = temp_viewers[2]
                    .pop()
                    .expect("third fill peer holds viewers");
                fill_peers[2].detach(&spare, deadline)?;
                counters.outstanding.fetch_sub(1, Ordering::Relaxed);
                let mut leaver =
                    SessionPeer::connect(fixture, cohort, 0x7000, Instant::now() + PROBE_TIMEOUT)?;
                let AttachProbe::Admitted(_abandoned) =
                    leaver.attach_until_saturated(site, &rows[1], deadline, PRESSURE_RETRY)?
                else {
                    return Err("abrupt witness leaver could not attach".into());
                };
                bump(&counters.attached);
                bump(&counters.outstanding);
                // 24 known live again: a distinct valid overflow must refuse.
                if let Some(attachment) =
                    probe.attach_expect_refused(site, dedicated_tui, deadline, PRESSURE_RETRY)?
                {
                    let _ = probe.detach(&attachment, deadline);
                    return Err("abrupt-fill table admitted past 24".into());
                }
                // Session-drop release: dropping the leaver frees its one
                // attachment on the Server's own schedule — no Detach
                // acknowledgement is claimed, but the ledger knows the slot
                // is no longer held.
                drop(leaver);
                counters.outstanding.fetch_sub(1, Ordering::Relaxed);
                // The Server notices the disconnect on its own schedule:
                // exactly one fresh admission may reclaim the freed slot.
                // Each attempt gets a fresh budget bounded by the reclaim
                // window — the window itself is never extended per retry.
                let reattach_deadline = Instant::now() + RELEASE_WINDOW;
                let reclaimed = loop {
                    let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(reattach_deadline);
                    match probe.attach_probe(site, dedicated_tui, per_op)? {
                        AttachProbe::Admitted(attachment) => break attachment,
                        AttachProbe::Refused => {
                            if Instant::now() >= reattach_deadline {
                                return Err("abandoned viewer slot was never reclaimed".into());
                            }
                            thread::sleep(Duration::from_millis(150));
                        }
                    }
                };
                bump(&counters.outstanding);
                probe_viewers.push(reclaimed);
                // Exactly one slot was freed: a second distinct overflow
                // must refuse again.
                if let Some(attachment) = probe.attach_expect_refused(
                    site,
                    dedicated_coordinator,
                    deadline,
                    PRESSURE_RETRY,
                )? {
                    let _ = probe.detach(&attachment, deadline);
                    return Err("a second slot appeared after one reclaim".into());
                }

                // Settle every probe-held attachment, then drop the four
                // temporary sessions. Their session slots reap asynchronously;
                // the boundary fills below retry a not-yet-reaped slot rather
                // than assume a synchronous teardown.
                for attachment in &probe_viewers {
                    probe.detach(attachment, deadline)?;
                    counters.outstanding.fetch_sub(1, Ordering::Relaxed);
                }
                // Session-drop release: the fill peers' remaining attachments
                // free on disconnect, not on Detach — settle the known count.
                let dropped: usize = temp_viewers.iter().map(Vec::len).sum();
                drop(fill_peers);
                drop(temp_viewers);
                counters.outstanding.fetch_sub(dropped, Ordering::Relaxed);
            } else {
                // Base profile: the two standing TUIs alone already fill the
                // viewer table, so no fill is possible — boundary evidence is
                // one distinct valid overflow refused persistently.
                if let Some(attachment) =
                    probe.attach_expect_refused(site, dedicated_tui, deadline, PRESSURE_RETRY)?
                {
                    let _ = probe.detach(&attachment, deadline);
                    return Err("base viewer table admitted past its cap".into());
                }
            }

            // Session/connection conclusions likewise require healthy
            // residents after the viewer probes.
            if failed.load(Ordering::Relaxed) > 0 {
                return Err("a participant failed during the viewer probes".into());
            }
            assert_process_retained("orchbee-server", &server);
            assert_process_retained("orchbee-worker", &worker);
            assert_process_retained("orchbee-terminal-host", &host);

            // Session cap: every tenant session is a held object, so the
            // fill arithmetic is exact — the next admission must be a typed
            // CapacityExceeded refusal, then one release must readmit. A
            // fill peer's not-yet-reaped slot reads identically to a full
            // table, so an early CapacityExceeded is retried — but only
            // inside one aggregate fill budget that also caps how long the
            // loop may keep succeeding; each handshake gets its own bounded
            // budget inside that window.
            let mut fills = Vec::new();
            let fill_deadline = Instant::now() + PROBE_TIMEOUT;
            while fills.len() < profile.sessions - live_sessions {
                if Instant::now() >= fill_deadline {
                    return Err(format!(
                        "session fill never completed inside the aggregate budget: \
                         {} live of {}",
                        live_sessions + fills.len(),
                        profile.sessions
                    ));
                }
                let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(fill_deadline);
                match ClientSessionConnection::connect_before(&socket, cohort.identity(), per_op) {
                    Ok(connection) => fills.push(connection),
                    Err(ConnectClientError::Rejected(RejectionReason::CapacityExceeded)) => {
                        thread::sleep(Duration::from_millis(100))
                    }
                    Err(error) => {
                        return Err(format!(
                            "session fill refused at {} live: {error}",
                            live_sessions + fills.len()
                        ));
                    }
                }
            }
            // Fresh bounded probe: one admission past the cap must be the
            // typed refusal — the fill budget is not reused after it served.
            match ClientSessionConnection::connect_before(
                &socket,
                cohort.identity(),
                Instant::now() + EXCHANGE_TIMEOUT,
            ) {
                Err(ConnectClientError::Rejected(RejectionReason::CapacityExceeded)) => {}
                Err(error) => return Err(format!("session overflow failed untyped: {error}")),
                Ok(_) => return Err("session admitted past the cap".into()),
            }
            if fills.pop().is_some() {
                let release_deadline = Instant::now() + RELEASE_WINDOW;
                loop {
                    let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(release_deadline);
                    match SessionPeer::connect(fixture, cohort, 0x5000, per_op) {
                        Ok(_) => break,
                        Err(_) if Instant::now() < release_deadline => {
                            thread::sleep(Duration::from_millis(100))
                        }
                        Err(error) => {
                            return Err(format!("released session slot did not readmit: {error}"));
                        }
                    }
                }
            }
            drop(fills);
            // The just-dropped fills reap asynchronously; a short settle
            // keeps the connection-table arithmetic from racing teardown.
            thread::sleep(Duration::from_millis(500));

            // Connection cap: raw half-open connections occupy slots until
            // the handshake timeout reaps them. A full table refuses the
            // next admission by dropping the accepted socket before any
            // handshake — a labeled transport-boundary event, never a typed
            // business busy and never an invented protocol refusal.
            let mut observed_refusal = false;
            for _round in 0..CONNECTION_PROBE_ROUNDS {
                let mut raws = Vec::new();
                for _ in live_sessions..profile.connections {
                    match UnixStream::connect(&socket) {
                        Ok(raw) => raws.push(raw),
                        Err(_) => break,
                    }
                }
                match ClientSessionConnection::connect_before(
                    &socket,
                    cohort.identity(),
                    Instant::now() + EXCHANGE_TIMEOUT,
                ) {
                    Err(ConnectClientError::Io(error)) => {
                        eprintln!("connection-slot drop surfaced at transport boundary: {error}");
                        observed_refusal = true;
                    }
                    Err(error) => {
                        return Err(format!(
                            "connection-cap refusal surfaced an unexpected class: {error}"
                        ));
                    }
                    Ok(_) => {}
                }
                drop(raws);
                if observed_refusal {
                    break;
                }
                thread::sleep(Duration::from_millis(400));
            }
            if !observed_refusal {
                return Err("connection cap never refused a probe".into());
            }
            let recover_deadline = Instant::now() + RELEASE_WINDOW + Duration::from_secs(3);
            loop {
                let per_op = (Instant::now() + EXCHANGE_TIMEOUT).min(recover_deadline);
                match SessionPeer::connect(fixture, cohort, 0x6000, per_op) {
                    Ok(_) => break,
                    Err(_) if Instant::now() < recover_deadline => {
                        thread::sleep(Duration::from_millis(200))
                    }
                    Err(error) => return Err(format!("connection slots never reaped: {error}")),
                }
            }

            // Resume full traffic under the acceptance-44 oracle: one armed
            // start shared by all participants, published together with
            // pause=false inside the observation lock. In-process slots
            // (resumed producers plus the standing tui_like tenant, whose
            // after-arm success is continuity evidence) owe a first qualified
            // success by start+3s; the real CLI burst slots owe one exit-0
            // served cycle by start+12s. Pre-arm observations are ineligible
            // and a cutoff stamp can never be overwritten.
            let progress_before: Vec<usize> = counters
                .progress
                .iter()
                .map(|progress| progress.load(Ordering::Relaxed))
                .collect();
            eprintln!("pre-unpause progress snapshot: {progress_before:?}");
            let recovery_start = recovery.arm();
            let in_process = profile.wait_like + 1 + profile.tui_like;
            let in_process_cut = recovery_start + RECOVERY_WINDOW;
            let burst_cut = recovery_start + BURST_RECOVERY_WINDOW;

            // Observe the 3s cut: record all fifteen diagnostics, then fix
            // the in-process verdicts — a slot missing here stays failed even
            // if a success lands afterwards.
            thread::sleep(in_process_cut.saturating_duration_since(Instant::now()));
            {
                let observation = recovery.observation.lock().unwrap();
                for participant in 0..participants {
                    let stamp = observation
                        .first(participant)
                        .map(|instant| format!("+{:?}", instant.duration_since(recovery_start)))
                        .unwrap_or_else(|| "none".to_owned());
                    eprintln!(
                        "recovery cut: participant {participant} first={stamp} {}",
                        counters.phase_line(participant)
                    );
                }
                let missed: Vec<usize> = (0..in_process)
                    .filter(|slot| !observation.qualified(*slot, in_process_cut))
                    .collect();
                drop(observation);
                if !missed.is_empty() {
                    eprintln!(
                        "recovery cut counters: cycles={} attached={} detached={} \
                         records={} rejected={} unavailable={} send={} \
                         acquired={} input={}",
                        counters.cycles.load(Ordering::Relaxed),
                        counters.attached.load(Ordering::Relaxed),
                        counters.detached.load(Ordering::Relaxed),
                        counters.records.load(Ordering::Relaxed),
                        counters.rejected.load(Ordering::Relaxed),
                        counters.unavailable.load(Ordering::Relaxed),
                        counters.send_accepted.load(Ordering::Relaxed),
                        counters.acquired.load(Ordering::Relaxed),
                        counters.input_accepted.load(Ordering::Relaxed),
                    );
                    return Err(format!(
                        "in-process participants missed the 3s recovery cut: {missed:?}"
                    ));
                }
            }

            // Burst slots: stop early once every required first success has
            // been recorded, otherwise the verdict is read at the 12s
            // maximum — a stamp at-or-before the boundary qualifies, a later
            // or missing stamp is the honest failure evidence.
            loop {
                let done = {
                    let observation = recovery.observation.lock().unwrap();
                    (in_process..participants).all(|slot| observation.first(slot).is_some())
                };
                if done || Instant::now() >= burst_cut || failed.load(Ordering::Relaxed) > 0 {
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
            if failed.load(Ordering::Relaxed) > 0 {
                return Err("a participant failed during the recovery window".into());
            }
            let mut missed = Vec::new();
            {
                let observation = recovery.observation.lock().unwrap();
                for slot in in_process..participants {
                    if !observation.qualified(slot, burst_cut) {
                        let stamp = observation
                            .first(slot)
                            .map(|instant| format!("+{:?}", instant.duration_since(recovery_start)))
                            .unwrap_or_else(|| "missing".to_owned());
                        missed.push(format!(
                            "participant {slot} first={stamp} {}",
                            counters.phase_line(slot)
                        ));
                    }
                }
            }
            if !missed.is_empty() {
                return Err(format!(
                    "burst participants missed the 12s recovery boundary: {}",
                    missed.join("; ")
                ));
            }
            Ok(())
        })();

        // Every join is reached and every owned child reaped before any
        // witness error surfaces — a participant failure's own cause is
        // preserved alongside the ledger error.
        stop.store(true, Ordering::Relaxed);
        let mut problems: Vec<String> = Vec::new();
        for join in joins {
            match join.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => problems.push(format!("participant failed: {error}")),
                Err(_) => problems.push("participant panicked".to_owned()),
            }
        }
        if let Err(error) = &witness {
            problems.push(format!("witness: {error}"));
        }
        assert!(problems.is_empty(), "{}", problems.join("; "));
    });

    // Post-load reality: every table is free, every process retained, and a
    // fresh query plus a fresh attach both admit. The status child is owned
    // the same way — its own never-set stop flag, since the participant stop
    // has already been raised.
    let post_stop = AtomicBool::new(false);
    let (end, _elapsed, document) = c1_cli_json(
        &fixture,
        &child_output,
        "post-load-status",
        &post_stop,
        &["status", "--format", "json"],
    )
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    let CliChildEnd::Exited(status) = end else {
        return Err(format!("post-load status child ended abnormally: {document}").into());
    };
    assert_eq!(status.code(), Some(0), "post-load status: {document}");
    assert!(
        probe
            .attach(&site, &rows[0], Instant::now() + EXCHANGE_TIMEOUT)
            .map_err(|error| -> Box<dyn Error> { error.into() })?
            .is_some(),
        "post-load attach did not admit"
    );
    assert_process_retained("orchbee-server", &server);
    assert_process_retained("orchbee-worker", &worker);
    assert_process_retained("orchbee-terminal-host", &host);

    let cycles = counters.cycles.load(Ordering::Relaxed);
    assert!(cycles > 0, "load map recorded no cycles");
    assert_eq!(
        counters.unknown.load(Ordering::Relaxed),
        0,
        "outcome-unknown under capacity pressure"
    );
    assert!(
        counters.acquired.load(Ordering::Relaxed) > 0,
        "no controller grant was ever accepted"
    );
    assert!(
        counters.input_accepted.load(Ordering::Relaxed) > 0,
        "no controller input was ever accepted"
    );
    eprintln!(
        "c1 witness counters: cycles={cycles} attached={} detached={} records={} \
         rejected={} unavailable={} send={} acquired={} input={}",
        counters.attached.load(Ordering::Relaxed),
        counters.detached.load(Ordering::Relaxed),
        counters.records.load(Ordering::Relaxed),
        counters.rejected.load(Ordering::Relaxed),
        counters.unavailable.load(Ordering::Relaxed),
        counters.send_accepted.load(Ordering::Relaxed),
        counters.acquired.load(Ordering::Relaxed),
        counters.input_accepted.load(Ordering::Relaxed),
    );

    tui.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// Bounded timing probe for the run-12 cause packet — never part of the
/// suite. Measures what one real `orchbee terminal read` cycle costs against
/// the same staged cohort and fixture shape the witness uses: sequential,
/// four-parallel (one burst slot's unit of work), and four-parallel under a
/// fourteen-session polling herd (participant 11's post-resume environment).
/// No capacity verdicts — durations only.
#[test]
#[ignore = "run-12 bounded timing probe — invoked explicitly"]
fn c1_burst_cli_timing_probe() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let mut tui = Client::spawn(&fixture, "en_US.UTF-8");
    tui.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let (_provisioner, site, rows, _dedicated_tui, _dedicated_coordinator) =
        provision(&fixture, &mut tui).expect("provision terminals");
    let cohort = NativeCohort::load(&fixture.cohort, Instant::now() + PROBE_TIMEOUT)
        .expect("cohort identity");
    let terminal = rows[0].terminal.as_str().to_owned();

    // Phase A: the cohort-hash term every CLI process pays on startup.
    let hash_start = Instant::now();
    let _ = NativeCohort::load(&fixture.cohort, Instant::now() + PROBE_TIMEOUT)
        .expect("cohort identity");
    eprintln!("probe: NativeCohort::load {:?}", hash_start.elapsed());

    // Phase B: sequential baseline — one CLI cycle at a time.
    for round in 0..3 {
        let start = Instant::now();
        let (output, document) = run_json(
            &fixture,
            &["terminal", "read", &terminal, "--format", "json"],
        );
        eprintln!(
            "probe seq[{round}]: {:?} exit={:?} kind={:?}",
            start.elapsed(),
            output.status.code(),
            document["failure"]["kind"].as_str()
        );
    }

    // Phase C: four parallel CLI cycles — the burst-slot unit of work.
    for round in 0..2 {
        thread::scope(|scope| {
            let mut joins = Vec::new();
            for index in 0..4 {
                let fixture = &fixture;
                let terminal = &terminal;
                joins.push(scope.spawn(move || {
                    let start = Instant::now();
                    let (output, document) =
                        run_json(fixture, &["terminal", "read", terminal, "--format", "json"]);
                    eprintln!(
                        "probe par[{index}] round {round}: {:?} exit={:?} kind={:?}",
                        start.elapsed(),
                        output.status.code(),
                        document["failure"]["kind"].as_str()
                    );
                }));
            }
            for join in joins {
                join.join().expect("probe thread");
            }
        });
    }

    // Phase D: the same four parallel cycles under a fourteen-session
    // polling herd — nine pollers plus five idle session holders model the
    // post-resume environment where participant 11's child exceeded the 3s
    // recovery cut.
    let herd_stop = AtomicBool::new(false);
    thread::scope(|scope| {
        let fixture = &fixture;
        let cohort = &cohort;
        let site = &site;
        let rows = &rows;
        let herd_stop = &herd_stop;
        let mut herd = Vec::new();
        for index in 0..14u32 {
            herd.push(scope.spawn(move || -> Result<(), String> {
                let mut peer = SessionPeer::connect(
                    fixture,
                    cohort,
                    0x8000 + index,
                    Instant::now() + PROBE_TIMEOUT,
                )?;
                while !herd_stop.load(Ordering::Relaxed) {
                    if index < 9 {
                        let deadline = Instant::now() + EXCHANGE_TIMEOUT;
                        if let Some(attachment) =
                            peer.attach(site, &rows[index as usize % rows.len()], deadline)?
                        {
                            let _ = peer.viewer(
                                TerminalViewerAction::Poll {
                                    attachment_id: attachment.clone(),
                                },
                                deadline,
                            );
                            let _ = peer.detach(&attachment, deadline);
                        }
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Ok(())
            }));
        }
        thread::sleep(Duration::from_millis(500));
        for index in 0..4 {
            let terminal = &terminal;
            scope.spawn(move || {
                let start = Instant::now();
                let (output, document) =
                    run_json(fixture, &["terminal", "read", terminal, "--format", "json"]);
                eprintln!(
                    "probe herd[{index}]: {:?} exit={:?} kind={:?}",
                    start.elapsed(),
                    output.status.code(),
                    document["failure"]["kind"].as_str()
                );
            });
        }
        thread::sleep(Duration::from_secs(12));
        herd_stop.store(true, Ordering::Relaxed);
        for join in herd {
            match join.join() {
                Ok(Ok(_)) | Ok(Err(_)) => {}
                Err(_) => panic!("herd thread panicked"),
            }
        }
    });
    tui.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// Acceptance-44 oracle evidence with injected clocks — the production
/// witness only ever samples real `Instant`s inside the guard; these
/// boundaries are deterministic. Slot 10 models the standing tenant.
#[test]
fn recovery_observation_oracle_boundaries() {
    let start = Instant::now();
    let in_process_cut = start + RECOVERY_WINDOW;
    let burst_cut = start + BURST_RECOVERY_WINDOW;
    let mut observation = RecoveryObservation::new(15);

    // A standing completion stamped before arming is dropped: nothing was
    // armed, so the record is ignored outright.
    observation.record_at(10, start);
    assert!(observation.first(10).is_none());

    // Arming clears the slots and establishes the common start; a stamp
    // recorded while armed then re-cleared by a second arm is also gone.
    observation.arm_at(start);
    observation.record_at(10, start);
    observation.arm_at(start);
    assert!(observation.first(10).is_none());

    // A first success exactly at the boundary qualifies; one nanosecond
    // later does not.
    observation.arm_at(start);
    observation.record_at(0, in_process_cut);
    assert!(observation.qualified(0, in_process_cut));
    observation.record_at(1, in_process_cut + Duration::from_nanos(1));
    assert!(!observation.qualified(1, in_process_cut));

    // The same stamp qualifies against the later burst boundary — the
    // classes share one armed start with different deadlines.
    observation.record_at(11, in_process_cut + Duration::from_nanos(1));
    assert!(observation.qualified(11, burst_cut));
    observation.record_at(12, burst_cut + Duration::from_nanos(1));
    assert!(!observation.qualified(12, burst_cut));

    // A later completion never overwrites the retained first stamp.
    observation.record_at(0, in_process_cut + Duration::from_secs(1));
    assert_eq!(observation.first(0), Some(in_process_cut));

    // Refusals never enter the oracle — nothing recorded means no stamp —
    // and a missing slot fails its verdict.
    assert!(observation.first(2).is_none());
    assert!(!observation.qualified(2, in_process_cut));
    assert!(!observation.qualified(13, burst_cut));
}

/// Acceptance-44 child-lifetime evidence: a controlled held child is ended
/// by the stop signal (distinct `Canceled`), by its fixed watchdog
/// (`TimedOut`), or exits normally — every path reaps it. No second
/// capacity workload.
#[test]
fn owned_child_stop_watchdog_and_exit() -> TestResult {
    let dir = std::env::temp_dir().join(format!("obc-owned-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    let out = dir.join("child.out");
    let err = dir.join("child.err");

    // Normal exit: a short-lived producer reports Exited and its streams
    // landed in the private files, collected within bound.
    let stop = AtomicBool::new(false);
    let end = run_owned_child(
        Command::new("sh")
            .arg("-c")
            .arg("echo out-bytes; echo err-bytes >&2"),
        &out,
        &err,
        &stop,
        Duration::from_secs(30),
    )
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    let CliChildEnd::Exited(status) = end else {
        return Err("producer child did not exit normally".into());
    };
    assert!(status.success());
    assert_eq!(collect_bounded(&out, 1024).unwrap(), b"out-bytes\n");

    // Stop ends a held child early with the distinct cancellation end.
    let stop = AtomicBool::new(false);
    thread::scope(|scope| {
        let stop_ref = &stop;
        scope.spawn(move || {
            thread::sleep(Duration::from_millis(150));
            stop_ref.store(true, Ordering::Relaxed);
        });
        let end = run_owned_child(
            Command::new("sleep").arg("30"),
            &out,
            &err,
            &stop,
            Duration::from_secs(60),
        )
        .expect("stop-cancelled child");
        assert!(
            matches!(end, CliChildEnd::Canceled),
            "held child did not end as canceled"
        );
    });

    // The fixed watchdog ends a held child that outlives it.
    let end = run_owned_child(
        Command::new("sleep").arg("30"),
        &out,
        &err,
        &AtomicBool::new(false),
        Duration::from_millis(300),
    )
    .map_err(|error| -> Box<dyn Error> { error.into() })?;
    assert!(
        matches!(end, CliChildEnd::TimedOut),
        "held child did not hit the watchdog"
    );

    // An over-bound stream is diagnosed, never truncated into success.
    fs::write(&out, vec![b'x'; 2048])?;
    assert!(collect_bounded(&out, 1024).is_err());
    fs::remove_dir_all(&dir)?;
    Ok(())
}
