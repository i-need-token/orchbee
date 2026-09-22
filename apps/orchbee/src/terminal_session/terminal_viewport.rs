//! Deterministic per-pane retained-history viewport core.
//!
//! One `TerminalViewport` exists per pane incarnation while the pane is
//! non-Live; its absence means `Live`. The core owns mode transitions, facts
//! tracking, the installed row window, the single read flight, the single
//! acknowledgement flight, the committed anchor, navigation coalescing,
//! bounded committed deltas, facts deadlines, and recovery. It never touches
//! the mailbox directly: effects are returned to `ui.rs`, and commands are
//! staged in `pending_commands` for the owner to admit in order.
//!
//! Contract: `docs/native-terminal-viewport.md` and
//! `formal/tla/TerminalClientViewport.tla`.

use std::{collections::VecDeque, num::NonZeroU64, time::Duration, time::Instant};

use orchbee_client_server_protocol::{ViewerRowsRejectedReason, ViewportRowOrigin};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, HistoryAnchorStatus, HistoryChanged, HistoryControl,
    HostEpoch, MessageRole, PayloadEncoding, TerminalTextRowWindow, decode_history_control,
    decode_transport_envelope,
};

use super::state::SessionNotice;

/// `HistoryChanged` (kind 53) — the wire kind is owned by
/// `crates/terminal-protocol`'s message contract; the literal mirrors
/// `MESSAGE_CONTRACTS` so the intake can route before decoding.
const HISTORY_CHANGED_KIND: u16 = 53;

/// Decode and route-validate a `HistoryChanged` record for `attachment`/`epoch`.
/// `Ok(None)` means the record is not a kind-53 event at all; `Err` is a
/// protocol violation the caller retires.
pub(super) fn decode_history_changed(
    record: &[u8],
    attachment: &AttachmentId,
    epoch: &HostEpoch,
) -> Result<Option<HistoryChanged>, ()> {
    let envelope = decode_transport_envelope(record).map_err(|_| ())?;
    if envelope.kind != HISTORY_CHANGED_KIND {
        return Ok(None);
    }
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Event
        || envelope.correlation_id.is_some()
        || envelope.host_epoch != Some(*epoch)
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
    {
        return Err(());
    }
    match decode_history_control(envelope.kind, envelope.role, envelope.payload) {
        Ok(HistoryControl::HistoryChanged(changed)) if changed.attachment_id == *attachment => {
            Ok(Some(changed))
        }
        _ => Err(()),
    }
}

/// Committed reads move at most this many rows per step, matching the wire
/// `Committed{delta}` domain.
const COMMITTED_DELTA_BOUND: i64 = 4096;
/// One bounded budget covers each facts-wait episode: entry facts, a stale
/// retry inside the episode, and post-resize facts.
const FACTS_DEADLINE: Duration = Duration::from_secs(3);
/// The wire `maximum_bytes` budget for a row window.
const READ_MAXIMUM_BYTES: u32 = 8 * 1024 * 1024;
const MAXIMUM_READ_ROWS: u16 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewportMode {
    Entering,
    Viewing,
    Recovering,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AckState {
    None,
    Pending,
    Flight,
    Accepted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingNav {
    None,
    /// Remaining signed line delta; positive scrolls toward older rows.
    Delta(i64),
    /// Absolute jump to the oldest retained row (`Tail{u32::MAX}`).
    Oldest,
}

/// What the in-flight read carries, so its intent is consumed only when the
/// window actually installs — a rejection keeps the whole intent for the
/// retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InflightNav {
    None,
    EntryTail,
    Delta(i64),
    Oldest,
    Refresh,
}

/// One logical viewport command the worker executes against the stable S1
/// protocol seam. `Enter` is staged by `enter_history` itself (its admission
/// gates installing the core); the other steps ride `pending_commands`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewportStep {
    Enter,
    Read {
        origin: ViewportRowOrigin,
        row_count: u16,
        revision: u64,
        generation: NonZeroU64,
        request_sequence: NonZeroU64,
        maximum_bytes: u32,
    },
    Ack {
        request_sequence: NonZeroU64,
        generation: NonZeroU64,
        revision: u64,
    },
    Recover,
}

/// Typed host rejection for an in-flight read; `Host` is the opaque
/// `Rejected{HostRejected}` form that also covers pending-pin blockages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewportReadRejection {
    Typed(ViewerRowsRejectedReason),
    Host,
}

/// Local navigation intent admitted by `nav`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewportNav {
    /// Signed lines: positive toward older rows, negative toward the tail.
    Lines(i64),
    Oldest,
    /// `End`/`Esc`: recover and return to live.
    Exit,
}

/// What the renderer may paint for this pane at `columns` right now. Every
/// access re-validates generation and columns so stale data can never paint.
#[derive(Clone, Copy, Debug)]
pub(super) enum ViewportSurface<'a> {
    /// A current, gate-passing window: paint its plain-text rows.
    Window(&'a TerminalTextRowWindow),
    /// Entering with no window yet: the last verified live frame may show
    /// beneath the history badge.
    Entering,
    /// Viewing without a current window: clear and show the badge only.
    AwaitingWindow,
    /// Recovering: clear and show the recovering indicator.
    Recovering,
}

/// Side effects a core transition asks the owner to perform. Commands stay
/// inside `pending_commands`; `live` drops the core entirely.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ViewportEffect {
    pub notice: Option<SessionNotice>,
    /// Drop the core: the pane returns to Live.
    pub live: bool,
    /// A wire-level violation (undecodable or foreign-route window): the
    /// owner retires the carrier as a protocol fault.
    pub protocol_error: bool,
}

pub(super) struct TerminalViewport {
    mode: ViewportMode,
    attachment: AttachmentId,
    epoch: HostEpoch,
    columns: u16,
    facts: Option<HistoryChanged>,
    window: Option<TerminalTextRowWindow>,
    ack: AckState,
    committed: bool,
    read_flight: Option<NonZeroU64>,
    /// The facts generation the in-flight (or last-emitted) read was issued
    /// under — evidence for whether a mismatched window's pin was released
    /// by a generation transition the client already observed.
    read_generation: Option<NonZeroU64>,
    next_read_sequence: u64,
    pending_nav: PendingNav,
    inflight_nav: InflightNav,
    refresh: bool,
    needs_facts: bool,
    facts_deadline: Option<Instant>,
    retry_used: bool,
    halve_used: bool,
    /// Recovery demands applied; the first post-recovery frame ends it.
    recover_applied: bool,
    pending_commands: VecDeque<ViewportStep>,
    entry_rows_before: u32,
    /// The pane's live row count: what `surface`/`resize` compare against —
    /// `read_rows` can halve under `LimitExceeded` while the geometry stays.
    row_count: u16,
    read_rows: u16,
}

impl TerminalViewport {
    /// A newly admitted Enter: the caller has already validated the pane's
    /// live frame facts and enqueued `ViewportStep::Enter` for `key`.
    /// `first_read_sequence` continues the owning projection's per-incarnation
    /// read watermark — the host's `last_row_request_sequence` outlives a
    /// single history episode, so a re-entered episode must resume above the
    /// last issued value, never restart at 1.
    pub(super) fn entering(
        attachment: AttachmentId,
        epoch: HostEpoch,
        columns: u16,
        row_count: u16,
        entry_rows_before: u32,
        first_read_sequence: u64,
        now: Instant,
    ) -> Self {
        Self {
            mode: ViewportMode::Entering,
            attachment,
            epoch,
            columns,
            facts: None,
            window: None,
            ack: AckState::None,
            committed: false,
            read_flight: None,
            read_generation: None,
            next_read_sequence: first_read_sequence,
            pending_nav: PendingNav::None,
            inflight_nav: InflightNav::None,
            refresh: false,
            needs_facts: true,
            facts_deadline: Some(now + FACTS_DEADLINE),
            retry_used: false,
            halve_used: false,
            recover_applied: false,
            pending_commands: VecDeque::new(),
            entry_rows_before,
            row_count,
            read_rows: row_count.clamp(1, MAXIMUM_READ_ROWS),
        }
    }

    /// The geometry the core was last resized to — `(columns, row_count)`.
    pub(super) const fn geometry(&self) -> (u16, u16) {
        (self.columns, self.row_count)
    }

    #[cfg(test)]
    pub(super) const fn mode(&self) -> ViewportMode {
        self.mode
    }

    pub(super) const fn recovering(&self) -> bool {
        matches!(self.mode, ViewportMode::Recovering)
    }

    /// The next unadmitted command the owner should try to enqueue.
    pub(super) fn peek_command(&self) -> Option<ViewportStep> {
        self.pending_commands.front().copied()
    }

    /// The owner admitted the head command onto the wire queue.
    pub(super) fn command_admitted(&mut self) {
        match self.pending_commands.pop_front() {
            Some(ViewportStep::Read {
                request_sequence, ..
            }) => {
                self.read_flight = Some(request_sequence);
            }
            Some(ViewportStep::Ack { .. }) => {
                self.ack = AckState::Flight;
            }
            Some(ViewportStep::Recover) => {
                self.mode = ViewportMode::Recovering;
                self.pending_nav = PendingNav::None;
                self.inflight_nav = InflightNav::None;
                self.read_flight = None;
                self.read_generation = None;
                self.refresh = false;
                self.facts_deadline = None;
            }
            Some(ViewportStep::Enter) | None => {}
        }
        self.drive_emissions();
    }

    /// Whether recovery is already decided — Recovering mode, or a Recover
    /// step still staged for admission. From this point on, late facts,
    /// windows, and read/ack outcomes are absorbed without installing a
    /// window or minting further wire commands.
    fn recovery_pending(&self) -> bool {
        self.recovering()
            || self
                .pending_commands
                .iter()
                .any(|step| matches!(step, ViewportStep::Recover))
    }

    /// A validated `HistoryChanged` event for this attachment. The caller has
    /// already enforced route and strict-sequence checks; every such event is
    /// acknowledged once through the causal lane regardless of adoption.
    pub(super) fn facts(&mut self, changed: HistoryChanged) -> ViewportEffect {
        self.facts = Some(changed);
        self.needs_facts = false;
        let mut effect = ViewportEffect::default();
        if self.recovery_pending() {
            // Recovery is already decided: store the facts for the record
            // but never let a late event re-arm emissions or mint a second
            // Recover.
            return effect;
        }
        if self.committed && changed.anchor_status != HistoryAnchorStatus::Valid {
            // The committed anchor is gone or unverifiable: recover; never
            // fake the position from stale absolute offsets.
            self.clear_position();
            self.queue_recover();
            effect.notice = Some(SessionNotice::HistoryRecovering);
        } else if matches!(self.mode, ViewportMode::Entering | ViewportMode::Viewing) {
            if self.committed {
                self.refresh = true;
            }
            self.drive_emissions();
        }
        effect
    }

    /// Local navigation while non-Live. Absolute intents own the next read;
    /// a delta landing on an absolute intent is dropped. A call that finds
    /// no pending and no in-flight intent starts a fresh intent — the
    /// retry/halving budget is per intent, so only then is it re-armed;
    /// merges and replacements inside an ongoing intent share its budget.
    pub(super) fn nav(&mut self, nav: ViewportNav) -> ViewportEffect {
        if self.recovery_pending() {
            // Recovery decided: further navigation — Exit included — is
            // absorbed without minting a second Recover.
            return ViewportEffect::default();
        }
        if !matches!(nav, ViewportNav::Exit)
            && matches!(self.pending_nav, PendingNav::None)
            && matches!(self.inflight_nav, InflightNav::None)
        {
            self.retry_used = false;
            self.halve_used = false;
        }
        match nav {
            ViewportNav::Exit => self.queue_recover(),
            ViewportNav::Oldest => {
                self.pending_nav = PendingNav::Oldest;
                self.drive_emissions();
            }
            ViewportNav::Lines(lines) => {
                if matches!(self.pending_nav, PendingNav::Oldest) {
                    return ViewportEffect::default();
                }
                let merged = match self.pending_nav {
                    PendingNav::Delta(current) => current.saturating_add(lines),
                    _ => lines,
                };
                self.pending_nav = if merged == 0 {
                    PendingNav::None
                } else {
                    PendingNav::Delta(merged)
                };
                self.drive_emissions();
            }
        }
        ViewportEffect::default()
    }

    /// An ordinary typed byte while non-Live: recovery is queued first; the
    /// owner forwards the byte only after `recovering()` reports the command
    /// was admitted. A Recover already staged counts as decided — typing
    /// again must not mint a second one.
    pub(super) fn typed(&mut self) {
        if !self.recovery_pending() {
            self.queue_recover();
        }
    }

    /// `ViewportEnterOutcome`: an applied entry keeps waiting for facts; a
    /// rejected (already reset) entry drops the core back to Live.
    pub(super) fn enter_outcome(&mut self, applied: bool) -> ViewportEffect {
        if applied {
            ViewportEffect::default()
        } else {
            ViewportEffect {
                notice: Some(SessionNotice::HistoryUnavailable),
                live: true,
                protocol_error: false,
            }
        }
    }

    /// A served, decoded OBRW window. The remaining install gates run in
    /// order; anything that cannot prove which pending it settled enters
    /// recovery. (Decode failures never reach here: the caller retires them
    /// as protocol faults.)
    pub(super) fn window(&mut self, window: TerminalTextRowWindow, now: Instant) -> ViewportEffect {
        if self.recovery_pending() {
            // A demand transition — applied or already staged — cleared the
            // host pin this window came from; discarding it is provably
            // safe, and it must not install or re-arm anything.
            return ViewportEffect::default();
        }
        if window.attachment_id != self.attachment || window.host_epoch != self.epoch {
            return ViewportEffect {
                protocol_error: true,
                ..ViewportEffect::default()
            };
        }
        if self.read_flight != Some(window.sequence) {
            // Cannot prove which pending it settled: recovery, never adoption.
            self.queue_recover();
            return ViewportEffect {
                notice: Some(SessionNotice::HistoryRecovering),
                ..ViewportEffect::default()
            };
        }
        let facts_generation = self.facts.map(|facts| facts.generation);
        if Some(window.generation) != facts_generation
            || window.columns != self.columns
            || self.needs_facts
        {
            self.read_flight = None;
            self.inflight_nav = InflightNav::None;
            // Did the held facts advance past the generation this read was
            // issued under? A kind-53 generation transition that landed
            // while the read was in flight is the host's pin release for
            // exactly this pending request — the freshest facts already sit
            // in hand, so the next read proceeds under them instead of
            // waiting for an event the host will never re-emit. Facts that
            // did not move since issue mean the host still owes the event
            // describing the window's generation: wait, bounded.
            let released = !self.needs_facts
                && self
                    .read_generation
                    .is_some_and(|issued| facts_generation != Some(issued));
            self.read_generation = None;
            if released {
                self.drive_emissions();
            } else {
                self.needs_facts = true;
                self.arm_deadline(now);
            }
            return ViewportEffect::default();
        }
        // All gates passed: install. Installing does not commit the anchor —
        // only an accepted RowsApplied does.
        let (request_sequence, generation, revision) =
            (window.sequence, window.generation, window.revision);
        self.window = Some(window);
        self.read_flight = None;
        self.read_generation = None;
        self.ack = AckState::Pending;
        self.pending_commands.push_back(ViewportStep::Ack {
            request_sequence,
            generation,
            revision,
        });
        self.consume_inflight_nav();
        ViewportEffect::default()
    }

    /// A read outcome for the in-flight sequence.
    pub(super) fn read_outcome(
        &mut self,
        request_sequence: NonZeroU64,
        rejection: ViewportReadRejection,
        now: Instant,
    ) -> ViewportEffect {
        if self.recovery_pending() || self.read_flight != Some(request_sequence) {
            return ViewportEffect::default();
        }
        self.read_flight = None;
        self.read_generation = None;
        self.inflight_nav = InflightNav::None;
        let mut effect = ViewportEffect::default();
        let recover = match rejection {
            // An opaque host rejection — pending blockage or a state race —
            // recovers rather than probing with another read.
            ViewportReadRejection::Host => true,
            ViewportReadRejection::Typed(
                ViewerRowsRejectedReason::StaleGeneration | ViewerRowsRejectedReason::StaleRevision,
            ) => {
                if self.retry_used {
                    true
                } else {
                    self.retry_used = true;
                    self.needs_facts = true;
                    self.arm_deadline(now);
                    false
                }
            }
            ViewportReadRejection::Typed(ViewerRowsRejectedReason::LimitExceeded) => {
                if self.halve_used || self.read_rows <= 1 {
                    true
                } else {
                    self.halve_used = true;
                    self.read_rows = (self.read_rows / 2).max(1);
                    self.refresh = true;
                    false
                }
            }
            ViewportReadRejection::Typed(_) => true,
        };
        if recover {
            self.queue_recover();
            effect.notice = Some(SessionNotice::HistoryRecovering);
        } else {
            self.drive_emissions();
        }
        effect
    }

    /// The acknowledgement outcome for the installed window. Only an exact
    /// accepted match promotes the pending anchor to committed.
    pub(super) fn ack_outcome(
        &mut self,
        request_sequence: NonZeroU64,
        accepted: bool,
    ) -> ViewportEffect {
        let matches = self.ack == AckState::Flight
            && self
                .window
                .as_ref()
                .is_some_and(|window| window.sequence == request_sequence);
        if self.recovery_pending() || !matches {
            return ViewportEffect::default();
        }
        if accepted {
            self.ack = AckState::Accepted;
            self.committed = true;
            self.facts_deadline = None;
            if self.mode == ViewportMode::Entering {
                self.mode = ViewportMode::Viewing;
            }
            self.drive_emissions();
            return ViewportEffect::default();
        }
        // Rejection loses the anchor: recovery with a notice.
        self.ack = AckState::None;
        self.clear_position();
        self.queue_recover();
        ViewportEffect {
            notice: Some(SessionNotice::HistoryRecovering),
            ..ViewportEffect::default()
        }
    }

    /// `ViewportRecovered`: recovery demands were applied; the next frame
    /// that arrives afterwards ends Recovering.
    pub(super) fn recovered(&mut self) {
        self.recover_applied = true;
    }

    /// A projection frame settled for this pane. Frames that were already in
    /// flight before the recovery demands cannot end Recovering early.
    pub(super) fn frame_applied(&mut self) -> ViewportEffect {
        if self.recovering() && self.recover_applied {
            ViewportEffect {
                live: true,
                ..ViewportEffect::default()
            }
        } else {
            ViewportEffect::default()
        }
    }

    /// Pane geometry changed: old-column windows can never paint again and
    /// fresh facts are awaited before the next read. The committed anchor is
    /// tracked by the host across resizes, so it survives. A repeated report
    /// of unchanged geometry is a no-op — it must not reset a wait episode.
    pub(super) fn resize(&mut self, columns: u16, row_count: u16, now: Instant) -> ViewportEffect {
        if self.columns == columns && self.row_count == row_count {
            return ViewportEffect::default();
        }
        self.columns = columns;
        self.row_count = row_count;
        self.read_rows = row_count.clamp(1, MAXIMUM_READ_ROWS);
        self.window = None;
        // A pending or in-flight acknowledgement resolves nothing under the
        // new geometry: the served window is gone either way, and a late
        // outcome for it can no longer match an installed window. Unadmitted
        // steps minted under the old geometry are dropped the same way —
        // they would only reach the wire with a stale generation.
        if matches!(self.ack, AckState::Pending | AckState::Flight) {
            self.ack = AckState::None;
        }
        self.pending_commands
            .retain(|step| matches!(step, ViewportStep::Recover));
        // A resize ends the current intent: whatever read the old geometry
        // staged is gone, and the fresh wait episode carries its own
        // retry/halving budget rather than inheriting spent flags.
        self.read_generation = None;
        self.retry_used = false;
        self.halve_used = false;
        self.needs_facts = true;
        self.arm_deadline(now);
        if self.committed {
            self.refresh = true;
        }
        self.drive_emissions();
        ViewportEffect::default()
    }

    /// Facts deadline: one bounded budget per wait episode; expiry recovers.
    pub(super) fn tick(&mut self, now: Instant) -> ViewportEffect {
        if self.facts_deadline.is_some_and(|deadline| now >= deadline)
            && matches!(self.mode, ViewportMode::Entering | ViewportMode::Viewing)
        {
            self.facts_deadline = None;
            self.queue_recover();
            return ViewportEffect {
                notice: Some(SessionNotice::HistoryRecovering),
                ..ViewportEffect::default()
            };
        }
        ViewportEffect::default()
    }

    /// What the renderer may paint right now. Stale-generation or
    /// stale-column data is never handed out: the window re-validates
    /// against the tracked geometry and facts on every call.
    pub(super) fn surface(&self) -> ViewportSurface<'_> {
        match self.mode {
            ViewportMode::Recovering => ViewportSurface::Recovering,
            ViewportMode::Entering => {
                if self.current_window().is_some() {
                    ViewportSurface::Window(self.window.as_ref().unwrap())
                } else {
                    ViewportSurface::Entering
                }
            }
            ViewportMode::Viewing => {
                if self.current_window().is_some() {
                    ViewportSurface::Window(self.window.as_ref().unwrap())
                } else {
                    ViewportSurface::AwaitingWindow
                }
            }
        }
    }

    /// The visible position for the indicator: `start_row` and `total_rows`
    /// of the currently painted window — only while that window still
    /// passes the live generation/columns validation the painter relies on.
    pub(super) fn position(&self) -> Option<(u64, u64)> {
        if self.recovering() {
            return None;
        }
        let window = self.current_window()?;
        Some((window.start_row, window.total_rows))
    }

    fn current_window(&self) -> Option<&TerminalTextRowWindow> {
        let window = self.window.as_ref()?;
        if self.needs_facts || window.columns != self.columns {
            return None;
        }
        if Some(window.generation) != self.facts.map(|facts| facts.generation) {
            return None;
        }
        Some(window)
    }

    fn arm_deadline(&mut self, now: Instant) {
        // The budget covers the whole wait episode; a retry inside it must
        // not reset it — but a fresh post-commit wait gets a fresh budget.
        if self.facts_deadline.is_none() {
            self.facts_deadline = Some(now + FACTS_DEADLINE);
        }
    }

    fn clear_position(&mut self) {
        self.window = None;
        self.committed = false;
        self.ack = AckState::None;
    }

    /// Absolute recovery intent: unadmitted non-recover commands are dropped
    /// (never on the wire — sequence gaps are legal), and exactly one Recover
    /// is queued.
    fn queue_recover(&mut self) {
        while let Some(front) = self.pending_commands.front() {
            if matches!(front, ViewportStep::Recover) {
                break;
            }
            self.pending_commands.pop_front();
        }
        if !self
            .pending_commands
            .iter()
            .any(|step| matches!(step, ViewportStep::Recover))
        {
            self.pending_commands.push_back(ViewportStep::Recover);
        }
        self.pending_nav = PendingNav::None;
        self.refresh = false;
        self.facts_deadline = None;
    }

    /// Stage the next wire command when all gates allow it. At most one
    /// unadmitted command is staged at a time so the admission order the
    /// owner applies always matches the core's intent.
    fn drive_emissions(&mut self) {
        if !self.pending_commands.is_empty()
            || !matches!(self.mode, ViewportMode::Entering | ViewportMode::Viewing)
        {
            return;
        }
        if self.ack == AckState::Pending {
            if let Some(window) = self.window.as_ref() {
                self.pending_commands.push_back(ViewportStep::Ack {
                    request_sequence: window.sequence,
                    generation: window.generation,
                    revision: window.revision,
                });
            }
            return;
        }
        let Some(step) = self.next_read_step() else {
            return;
        };
        self.pending_commands.push_back(step);
    }

    fn next_read_step(&mut self) -> Option<ViewportStep> {
        if self.read_flight.is_some()
            || !matches!(self.ack, AckState::None | AckState::Accepted)
            || self.needs_facts
        {
            return None;
        }
        let facts = self.facts?;
        let request_sequence = NonZeroU64::new(self.next_read_sequence)?;
        let (origin, row_count) = match self.mode {
            ViewportMode::Entering => {
                let entry = match self.pending_nav {
                    PendingNav::Oldest => {
                        self.inflight_nav = InflightNav::Oldest;
                        self.pending_nav = PendingNav::None;
                        u32::MAX
                    }
                    PendingNav::Delta(delta) => {
                        self.pending_nav = PendingNav::None;
                        self.entry_rows_before = (self.entry_rows_before as i64)
                            .saturating_add(delta)
                            .clamp(0, u32::MAX as i64)
                            as u32;
                        if self.entry_rows_before == 0 {
                            self.queue_recover();
                            return None;
                        }
                        self.inflight_nav = InflightNav::EntryTail;
                        self.entry_rows_before
                    }
                    PendingNav::None => {
                        self.inflight_nav = InflightNav::EntryTail;
                        self.entry_rows_before
                    }
                };
                (
                    ViewportRowOrigin::Tail { rows_before: entry },
                    self.read_rows,
                )
            }
            ViewportMode::Viewing => match self.pending_nav {
                PendingNav::Delta(delta) => {
                    // Reaching the live tail exits instead of emitting a read
                    // the host would only clamp.
                    if delta < 0
                        && self
                            .rows_before_estimate()
                            .is_some_and(|before| before + delta <= 0)
                    {
                        self.queue_recover();
                        return None;
                    }
                    let chunk = delta.clamp(-COMMITTED_DELTA_BOUND, COMMITTED_DELTA_BOUND);
                    self.inflight_nav = InflightNav::Delta(chunk);
                    // The wire delta lives in rows_before space: positive
                    // scrolls toward older rows, matching the pending intent.
                    (
                        ViewportRowOrigin::Committed {
                            delta: chunk as i32,
                        },
                        self.read_rows,
                    )
                }
                PendingNav::Oldest => {
                    self.inflight_nav = InflightNav::Oldest;
                    (
                        ViewportRowOrigin::Tail {
                            rows_before: u32::MAX,
                        },
                        self.read_rows,
                    )
                }
                PendingNav::None if self.refresh => {
                    self.inflight_nav = InflightNav::Refresh;
                    self.refresh = false;
                    (ViewportRowOrigin::Committed { delta: 0 }, self.read_rows)
                }
                PendingNav::None => return None,
            },
            ViewportMode::Recovering => return None,
        };
        let Some(next_sequence) = self.next_read_sequence.checked_add(1) else {
            // The sequence space ran out: no further read can ever be
            // correlated, so the position is unrecoverable by construction.
            self.queue_recover();
            return None;
        };
        self.next_read_sequence = next_sequence;
        self.read_generation = Some(facts.generation);
        Some(ViewportStep::Read {
            origin,
            row_count,
            revision: facts.revision,
            generation: facts.generation,
            request_sequence,
            maximum_bytes: READ_MAXIMUM_BYTES,
        })
    }

    /// Rows remaining below the installed window — the position estimate a
    /// downward scroll is checked against before it exits to live.
    fn rows_before_estimate(&self) -> Option<i64> {
        let window = self.window.as_ref()?;
        let below = window
            .total_rows
            .saturating_sub(window.start_row.saturating_add(window.rows.len() as u64));
        Some(i64::try_from(below.min(i64::MAX as u64)).unwrap_or(i64::MAX))
    }

    /// The in-flight read's intent is consumed only when its window installs.
    fn consume_inflight_nav(&mut self) {
        match self.inflight_nav {
            InflightNav::Delta(chunk) => {
                if let PendingNav::Delta(remaining) = self.pending_nav {
                    self.pending_nav = if remaining == chunk {
                        PendingNav::None
                    } else {
                        PendingNav::Delta(remaining.saturating_sub(chunk))
                    };
                }
            }
            InflightNav::Oldest => {
                // The installed jump satisfies the pending absolute intent;
                // a delta that accumulated during the flight survives.
                if matches!(self.pending_nav, PendingNav::Oldest) {
                    self.pending_nav = PendingNav::None;
                }
            }
            InflightNav::EntryTail | InflightNav::Refresh => {
                // An entry/refresh leaves a delta pending untouched.
            }
            InflightNav::None => {}
        }
        self.inflight_nav = InflightNav::None;
    }
}

#[cfg(test)]
mod tests;
