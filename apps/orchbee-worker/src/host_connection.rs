//! Owns the admitted same-user Host broker connection, negotiated limits, and
//! connection-scoped viewer, controller, title, and event state. It offers the `worker-host`
//! connection and validates the OBTP Hello without owning the Host child, product Workbench
//! membership, or Agent interpretation.

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    cell::RefCell,
    error::Error,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use orchbee_connection_handshake::{CANDIDATE_DEADLINE, ProcessEntry};

use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ClientId, ConnectionControl, CorrelationId, Envelope, Hello,
    MAXIMUM_INPUT_BYTES, MAXIMUM_ROW_WINDOW_BYTES, MAXIMUM_TERMINAL_FRAME_BYTES,
    MAXIMUM_TRANSPORT_RECORD_BYTES, MessageRole, PayloadEncoding, TRANSPORT_ENVELOPE_HEADER_BYTES,
    TerminalEffectEvent, decode_connection_control, decode_terminal_effect_event,
    decode_transport_envelope, encode_connection_control, encode_transport_envelope,
};

use crate::agent_drain::AgentDrain;

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod admission;

mod events;
pub(crate) use events::{drain_events, validate_event};

mod controllers;
pub(crate) use controllers::ControlGrant;
mod exchange;
mod host_colors;
mod offer;
#[cfg(windows)]
pub(crate) use offer::{HostStopping, source_commit};
mod pacing;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod process_query;
mod target_cleanup;
mod titles;
mod viewers;

#[cfg(test)]
mod tests;
#[cfg(windows)]
pub(crate) use exchange::exchange_within;
pub(crate) use exchange::{
    HostExchange, PeerFailure, UnexpectedAnswer, exchange, exchange_between_requests,
    response_envelope,
};
pub(crate) use host_colors::HostColors;
#[cfg(test)]
pub(crate) use offer::answer_test_offer;
pub(crate) use pacing::{HostFailure, NotStarted, Pacing};
pub(crate) use target_cleanup::PreparedTargetCleanup;
use viewers::HostViewers;

/// The message kind an intact Host record names in its fixed header, read without decoding the
/// rest; zero for a record too short to hold one.
pub(crate) fn record_kind(record: &[u8]) -> u16 {
    record
        .get(20..22)
        .and_then(|kind| <[u8; 2]>::try_from(kind).ok())
        .map_or(0, u16::from_le_bytes)
}

/// Records an intact Host event dropped because its envelope or payload breaks its contract.
fn contract(site: crate::diagnostic_events::Site, kind: u16) {
    crate::diagnostic_events::host_unexpected(
        site,
        kind,
        crate::diagnostic_events::Anomaly::Contract,
    );
}

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);
/// The program exits kept for the Worker's program owner: twice what one Host lists.
const MAXIMUM_PENDING_PROGRAM_EXITS: usize = 128;
const CLIENT_ID_BYTES: [u8; 16] = *b"orchbee-worker-1";
const HELLO_CORRELATION_BYTES: [u8; 16] = *b"worker-host-hi-1";

pub(crate) struct HostBrokerConnection {
    /// The endpoint this connection dialed, where the Host also answers control requests.
    endpoint: PathBuf,
    host_epoch: orchbee_terminal_protocol::HostEpoch,
    negotiated_limits: NegotiatedHostLimits,
    /// The Host's answer listed `prompt-gated-input`.
    prompt_gated_input: bool,
    /// The Host's answer listed `screen-gated-send`.
    screen_gated_send: bool,
    /// The Host's answer listed `keeper` and `recovery`.
    #[cfg(windows)]
    keeper: KeeperTerms,
    /// The Keeper events the Host sent since the Worker last took them.
    #[cfg(windows)]
    keeper_events: std::cell::Cell<KeeperEvents>,
    /// The Host's answer listed `background-programs`.
    background_programs: bool,
    /// The `ProgramExited` events the Host sent since the Worker last took them.
    program_exits: RefCell<Vec<orchbee_terminal_protocol::ProgramExited>>,
    stream: UnixStream,
    /// The Host record every reader of `stream` continues.
    inbound: RefCell<events::PartialRecord>,
    pub(crate) viewers: RefCell<HostViewers>,
    pub(crate) controllers: RefCell<controllers::HostControllers>,
    retired_targets: RefCell<target_cleanup::RetiredTargets>,
    /// The title projection every Host connection of this Worker shares, so one change sequence
    /// covers all of them.
    titles: Rc<RefCell<titles::Titles>>,
    /// The background programs this Host runs for the Worker, as its program answers showed: each
    /// it started or listed, until it disposed of it.
    programs: RefCell<std::collections::BTreeSet<orchbee_terminal_protocol::ProgramId>>,
    pub(crate) host_colors: RefCell<HostColors>,
    /// Shared handle to the admitted Server connection's Agent Drain queue.
    /// The broker only appends; the connection loop owns the watermark and
    /// outlives this Host attachment, so dropping the broker never resets it.
    drain: Rc<RefCell<AgentDrain>>,
    /// Exchange bounds, the current request's deadline and site, and the first Host failure.
    pub(crate) pacing: Pacing,
}

/// A failed bounded Host dial, split where the Host answered: `Connect` never reached it,
/// `ServerProcess` (Windows) reached an endpoint another process serves, `Busy` reached a Host that kept
/// another Worker session past its retry deadline, and `Handshake` reached it and failed the
/// `worker-host` offer or the protocol `Hello`.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[derive(Debug)]
pub(crate) enum DialFailure {
    Connect(io::Error),
    #[cfg(windows)]
    ServerProcess(io::Error),
    Busy(Box<dyn Error>),
    Handshake(Box<dyn Error>),
}

/// The bounds of one Host dial: on Windows the process that must serve the endpoint, when one is
/// known, the deadline past which a busy Host is no longer dialed again, and the deadline of the
/// whole dial.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct DialBounds {
    #[cfg(windows)]
    pub(crate) server_process: Option<std::num::NonZeroU32>,
    pub(crate) busy_until: Instant,
    pub(crate) deadline: Instant,
}

/// Whether the Host's answer listed `keeper`, `recovery`, and `handover`.
#[cfg(windows)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct KeeperTerms {
    keeper: bool,
    recovery: bool,
    handover: bool,
}

/// The Keeper events a Host sent: its Keeper ended (`KeeperLost`), missed a deadline
/// (`KeeperUnresponsive`), or a terminal runs unheld (`TerminalUnheld`).
#[cfg(windows)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct KeeperEvents {
    pub(crate) lost: bool,
    pub(crate) unresponsive: bool,
    pub(crate) unheld: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NegotiatedHostLimits {
    pub(crate) frame_bytes: u32,
    pub(crate) input_bytes: u32,
    pub(crate) row_window_bytes: u32,
}

impl HostBrokerConnection {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) fn connect_before(
        endpoint: &Path,
        deadline: std::time::Instant,
        drain: Rc<RefCell<AgentDrain>>,
    ) -> Result<Self, DialFailure> {
        Self::dial(
            endpoint,
            DialBounds {
                #[cfg(windows)]
                server_process: None,
                busy_until: deadline,
                deadline,
            },
            drain,
        )
    }

    /// Dials `endpoint` within `bounds`. On Windows a known server process must serve the pipe
    /// before any offer is written; on macOS and Linux the endpoint's peer user is checked as for
    /// every dial, and a server process is never trusted.
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) fn dial(
        endpoint: &Path,
        bounds: DialBounds,
        drain: Rc<RefCell<AgentDrain>>,
    ) -> Result<Self, DialFailure> {
        let deadline = bounds.deadline;
        let offer = offer::local().map_err(DialFailure::Handshake)?;
        let (mut transport, answer) = loop {
            let mut transport =
                admission::Admission::connect(endpoint, deadline).map_err(DialFailure::Connect)?;
            #[cfg(windows)]
            if let Some(expected) = bounds.server_process {
                let served = transport
                    .stream
                    .server_process_id()
                    .map_err(DialFailure::ServerProcess)?;
                if served != expected.get() {
                    return Err(DialFailure::ServerProcess(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "another process serves the Terminal Host endpoint",
                    )));
                }
            }
            transport.set_deadline(deadline.min(Instant::now() + CANDIDATE_DEADLINE));
            if let offer::Outcome::Answered(answer) =
                offer::exchange(&mut transport, &offer).map_err(DialFailure::Handshake)?
            {
                transport.set_deadline(deadline);
                break (transport, answer);
            }
            if Instant::now() + offer::RETRY_INTERVAL >= bounds.busy_until.min(deadline) {
                return Err(DialFailure::Busy(offer::still_busy()));
            }
            thread::sleep(offer::RETRY_INTERVAL);
        };
        let (host_epoch, negotiated_limits) = (|| {
            let expected = requested_hello()?;
            let correlation_id = CorrelationId::from_bytes(HELLO_CORRELATION_BYTES)?;
            write_hello(&mut transport, &expected, correlation_id)?;
            let accepted = validate_hello_accepted(&mut transport, &expected, correlation_id)?;
            transport.remaining()?;
            Ok(accepted)
        })()
        .map_err(DialFailure::Handshake)?;
        Ok(Self {
            endpoint: endpoint.to_owned(),
            host_epoch,
            negotiated_limits,
            prompt_gated_input: answer.prompt_gated_input,
            screen_gated_send: answer.screen_gated_send,
            #[cfg(windows)]
            keeper: answer.keeper,
            #[cfg(windows)]
            keeper_events: std::cell::Cell::default(),
            background_programs: answer.background_programs,
            program_exits: RefCell::default(),
            stream: transport.stream,
            inbound: RefCell::default(),
            viewers: RefCell::new(HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            titles: Rc::default(),
            programs: RefCell::default(),
            host_colors: RefCell::new(HostColors::default()),
            drain,
            pacing: Pacing::default(),
        })
    }

    pub(crate) fn connect(
        endpoint: &Path,
        drain: Rc<RefCell<AgentDrain>>,
    ) -> Result<Self, Box<dyn Error>> {
        let offer = offer::local()?;
        // A Host still ending the previous Worker session asks this one to dial again.
        let busy_deadline = Instant::now() + CANDIDATE_DEADLINE;
        let (mut stream, answer) = loop {
            let mut stream = UnixStream::connect(endpoint)?;
            require_same_effective_user(&stream)?;
            stream.set_read_timeout(Some(CANDIDATE_DEADLINE))?;
            stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;
            if let offer::Outcome::Answered(answer) = offer::exchange(&mut stream, &offer)? {
                break (stream, answer);
            }
            if Instant::now() + offer::RETRY_INTERVAL >= busy_deadline {
                return Err(offer::still_busy());
            }
            thread::sleep(offer::RETRY_INTERVAL);
        };
        stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;

        let expected = requested_hello()?;
        let correlation_id = CorrelationId::from_bytes(HELLO_CORRELATION_BYTES)?;
        write_hello(&mut stream, &expected, correlation_id)?;
        let (host_epoch, negotiated_limits) =
            validate_hello_accepted(&mut stream, &expected, correlation_id)?;

        unless_shut(stream.set_read_timeout(None))?;
        unless_shut(stream.set_write_timeout(None))?;
        Ok(Self {
            endpoint: endpoint.to_owned(),
            host_epoch,
            negotiated_limits,
            prompt_gated_input: answer.prompt_gated_input,
            screen_gated_send: answer.screen_gated_send,
            #[cfg(windows)]
            keeper: answer.keeper,
            #[cfg(windows)]
            keeper_events: std::cell::Cell::default(),
            background_programs: answer.background_programs,
            program_exits: RefCell::default(),
            stream,
            inbound: RefCell::default(),
            viewers: RefCell::new(HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            titles: Rc::default(),
            programs: RefCell::default(),
            host_colors: RefCell::new(HostColors::default()),
            drain,
            pacing: Pacing::default(),
        })
    }

    pub(crate) const fn host_epoch(&self) -> orchbee_terminal_protocol::HostEpoch {
        self.host_epoch
    }

    /// The Host's own process entry, from one `query` control request on a new connection to the
    /// endpoint this connection dialed; `None` when the Host has not answered by `deadline`.
    pub(crate) fn query_process(&self, deadline: Instant) -> Option<ProcessEntry> {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        return process_query::query(&self.endpoint, deadline).ok();
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        return None;
    }

    /// The Host's `dispatch-progress` and terminal count, from one `query` control request as
    /// [`Self::query_process`] sends it; `None` when the Host has not answered by `deadline` or its
    /// answer carries no progress.
    #[cfg(windows)]
    pub(crate) fn query_progress(&self, deadline: Instant) -> Option<(u64, u32)> {
        let (entry, progress) =
            process_query::query_with_progress(&self.endpoint, deadline).ok()?;
        Some((progress?, entry.terminals))
    }

    pub(crate) const fn negotiated_limits(&self) -> NegotiatedHostLimits {
        self.negotiated_limits
    }

    /// Whether this Host types an idle-prompt line only at a confirmed fresh prompt.
    pub(crate) const fn prompt_gated_input(&self) -> bool {
        self.prompt_gated_input
    }

    /// Whether this Host types a screen-gated key only while the screen is still the frame its
    /// sender read.
    pub(crate) const fn screen_gated_send(&self) -> bool {
        self.screen_gated_send
    }

    /// Whether this Host holds its running terminals in a Keeper the Worker attaches.
    #[cfg(windows)]
    pub(crate) const fn keeper(&self) -> bool {
        self.keeper.keeper
    }

    /// Whether this Host recovers a program the Keeper kept after its Host ended.
    #[cfg(windows)]
    pub(crate) const fn recovery(&self) -> bool {
        self.keeper.recovery
    }

    /// Whether this Host exports and imports terminals that move between Hosts.
    #[cfg(windows)]
    pub(crate) const fn handover(&self) -> bool {
        self.keeper.handover
    }

    /// Whether the Host reported its Keeper lost or unresponsive since the Keeper's care last took
    /// its events, which this call leaves for that care.
    #[cfg(windows)]
    pub(crate) fn keeper_troubled(&self) -> bool {
        let events = self.keeper_events.get();
        events.lost || events.unresponsive
    }

    /// The Keeper events the Host sent since the last call, which this call clears.
    #[cfg(windows)]
    pub(crate) fn take_keeper_events(&self) -> KeeperEvents {
        self.keeper_events.take()
    }

    /// Whether this Host runs the Worker's background programs.
    pub(crate) const fn background_programs(&self) -> bool {
        self.background_programs
    }

    /// The program exits the Host reported since the last call, which this call clears.
    pub(crate) fn take_program_exits(&self) -> Vec<orchbee_terminal_protocol::ProgramExited> {
        self.program_exits.take()
    }

    /// Notes which programs one of this Host's program answers shows it running or no longer
    /// running.
    pub(crate) fn note_program_answer(&self, answer: &orchbee_terminal_protocol::ProgramAnswer) {
        use orchbee_terminal_protocol::ProgramAnswer;
        let mut programs = self.programs.borrow_mut();
        match answer {
            ProgramAnswer::Started { program_id, .. } => {
                programs.insert(*program_id);
            }
            ProgramAnswer::Disposed(program_id) => {
                programs.remove(program_id);
            }
            ProgramAnswer::Page {
                programs: listed, ..
            } => programs.extend(listed.iter().map(|summary| summary.program_id)),
            _ => {}
        }
    }

    /// Whether this Host runs `program` for the Worker, as far as its answers showed.
    pub(crate) fn runs_program(&self, program: orchbee_terminal_protocol::ProgramId) -> bool {
        self.programs.borrow().contains(&program)
    }

    /// Whether this Host runs any program for the Worker, as far as its answers showed.
    #[cfg(windows)]
    pub(crate) fn runs_programs(&self) -> bool {
        !self.programs.borrow().is_empty()
    }

    /// Joins this connection's title projection to the one `primary` shares, so the Worker keeps one
    /// change sequence across its Hosts; the titles this connection read so far become changes there.
    #[cfg(windows)]
    pub(crate) fn share_titles(&mut self, primary: &Self) {
        if Rc::ptr_eq(&self.titles, &primary.titles) {
            return;
        }
        let own = std::mem::replace(&mut self.titles, Rc::clone(&primary.titles));
        primary.titles.borrow_mut().absorb(own.take());
    }

    /// Sends this Host one `drain` control request on a new connection to its endpoint, so it takes
    /// no new terminal and ends once it holds none; `false` when it has not answered by `deadline`.
    #[cfg(windows)]
    pub(crate) fn drain_host(&self, deadline: Instant) -> bool {
        process_query::drain(&self.endpoint, deadline).is_ok()
    }

    /// Routes one intact Host event to connection state. An event this Worker cannot use never
    /// ends the connection: one whose envelope or payload breaks its contract is dropped, a break in
    /// one attachment's flow closes only that attachment, and each is recorded. Only a record whose
    /// framing is lost, which [`PartialRecord`](events::PartialRecord) refuses before routing, ends
    /// the connection.
    pub(crate) fn receive_event(&self, record: Vec<u8>) {
        // A record the retired-target check cannot read is routed on and judged below.
        if self
            .retired_targets
            .borrow()
            .accept_event(&record, self.host_epoch)
            .unwrap_or(false)
        {
            return;
        }
        let Ok(envelope) = decode_transport_envelope(&record) else {
            return contract(self.pacing.site(), record_kind(&record));
        };
        // Envelope validation precedes every consumer below, including the
        // Agent Drain intercept and the viewer borrow.
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(self.host_epoch)
            || envelope.optional_message
        {
            return contract(self.pacing.site(), envelope.kind);
        }
        if envelope.kind == 23 {
            return self.receive_lease(&envelope);
        }
        // `KeeperLost`, `KeeperUnresponsive`, and `TerminalUnheld` come only to a session that
        // negotiated `keeper`; the Worker acts on them between Server requests.
        #[cfg(windows)]
        if (92..=94).contains(&envelope.kind) {
            let mut events = self.keeper_events.get();
            match envelope.kind {
                92 => events.lost = true,
                93 => events.unresponsive = true,
                _ => events.unheld = true,
            }
            return self.keeper_events.set(events);
        }
        // `ProgramExited` comes only to a session that negotiated `background-programs`; the
        // Worker's program owner takes the exits when it next serves a plugin-job request.
        if envelope.kind == orchbee_terminal_protocol::PROGRAM_EXITED_KIND {
            let Ok(exit) = orchbee_terminal_protocol::decode_program_exited(
                envelope.kind,
                envelope.role,
                envelope.payload,
            ) else {
                return contract(self.pacing.site(), envelope.kind);
            };
            let mut exits = self.program_exits.borrow_mut();
            // The Host lists at most 64 programs and reports each once to a session.
            if exits.len() < MAXIMUM_PENDING_PROGRAM_EXITS {
                exits.push(exit);
            }
            return;
        }
        if envelope.payload_encoding == PayloadEncoding::ControlJson
            && let Ok(TerminalEffectEvent::ApplicationSignal(signal)) =
                decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload)
        {
            // A quarantined terminal belongs to no binding: its signal reaches no Agent.
            if self.quarantined(&signal.identity) {
                return;
            }
            // The Agent Drain producer intercepts application signals before
            // the viewer borrow so terminals with no attached viewer still
            // reach the queue. A signal it cannot take is dropped and recorded.
            let identity = (signal.identity.host_epoch == self.host_epoch)
                .then(|| crate::terminal_host_launch_adapter::capability_identity(signal.identity))
                .and_then(Result::ok);
            let appended = identity.is_some_and(|identity| {
                self.drain
                    .borrow_mut()
                    .append(identity, signal.payload)
                    .is_ok()
            });
            if !appended {
                contract(crate::diagnostic_events::Site::Drain, envelope.kind);
            }
            return;
        }
        if envelope.kind == 57 {
            let observed = events::validate_event(&record, self.host_epoch).is_ok()
                && match decode_terminal_effect_event(
                    envelope.kind,
                    envelope.role,
                    envelope.payload,
                ) {
                    // A quarantined terminal's title is never named.
                    Ok(TerminalEffectEvent::MetadataChanged(event))
                        if self.quarantined(&event.identity) =>
                    {
                        true
                    }
                    Ok(TerminalEffectEvent::MetadataChanged(event)) => {
                        self.titles.borrow_mut().observe(event).is_ok()
                    }
                    _ => false,
                };
            if !observed {
                contract(crate::diagnostic_events::Site::Titles, envelope.kind);
            }
            return;
        }
        self.viewers.borrow_mut().receive(record, self.host_epoch);
    }

    fn quarantined(&self, identity: &orchbee_terminal_protocol::TerminalIdentity) -> bool {
        orchbee_capability_node_protocol::HostTerminalId::from_bytes(
            *identity.terminal_id.as_bytes(),
        )
        .is_ok_and(|terminal| self.quarantined_terminal(terminal))
    }

    /// A controller's resize-lease event. One for a controller this Worker does not hold is
    /// dropped; one that contradicts a held controller's grant closes that controller's viewer,
    /// whose cleanup releases the controller with it.
    fn receive_lease(&self, envelope: &orchbee_terminal_protocol::DecodedEnvelope<'_>) {
        use crate::diagnostic_events::{Anomaly, Site, host_unexpected};
        let event = (envelope.payload_encoding == PayloadEncoding::ControlJson)
            .then(|| {
                orchbee_terminal_protocol::decode_attachment_control(
                    envelope.kind,
                    envelope.role,
                    envelope.payload,
                )
            })
            .and_then(Result::ok);
        let Some(orchbee_terminal_protocol::AttachmentControl::ResizeLeaseChanged(event)) = event
        else {
            return contract(Site::Control, envelope.kind);
        };
        let controllers = self.controllers.borrow();
        let Some(viewer) = controllers.viewer_of(event.attachment_id) else {
            // Released controllers were excused by the retired-target check above; a lease
            // for any other attachment this Worker does not hold is dropped and recorded.
            return host_unexpected(Site::Control, envelope.kind, Anomaly::UnknownAttachment);
        };
        if !controllers.validate_lease_event(event) {
            self.viewers.borrow_mut().close(viewer);
            host_unexpected(Site::Control, envelope.kind, Anomaly::Flow);
        }
    }

    pub(crate) fn terminal_title(
        &self,
        identity: orchbee_capability_node_protocol::HostTerminalIdentity,
    ) -> orchbee_capability_node_protocol::TerminalTitleObservation {
        self.titles.borrow().read(identity)
    }

    /// Answers one Server change read after `after` from the retained title projection, within
    /// the protocol's page bounds. It neither drains Host events nor waits.
    pub(crate) fn title_changes(
        &self,
        after: u64,
    ) -> orchbee_capability_node_protocol::TerminalTitleChangesOutcome {
        self.titles.borrow_mut().changes(after)
    }

    /// The title note due between requests, at most one until a later change read is answered.
    pub(crate) fn take_title_note(
        &self,
    ) -> Option<orchbee_capability_node_protocol::TerminalTitlesChanged> {
        self.titles.borrow_mut().take_note()
    }

    pub(crate) const fn stream(&self) -> &UnixStream {
        &self.stream
    }

    /// When the Host record it has begun must be complete: past this, the Host has stopped inside
    /// a record for longer than an exchange may wait.
    pub(crate) fn partial_deadline(&self) -> Option<Instant> {
        self.inbound
            .borrow()
            .began()
            .map(|began| began + self.pacing.exchange_bound())
    }

    /// Records that the Host stopped inside a record past its bound, and returns that error.
    pub(crate) fn stalled(&self) -> io::Error {
        let waited = self
            .inbound
            .borrow()
            .began()
            .map_or(Duration::ZERO, |began| began.elapsed());
        let failure = HostFailure {
            class: PeerFailure::Host,
            deadline: true,
            waited,
            budget: self.pacing.exchange_bound(),
        };
        self.pacing.note_failure(failure);
        failure.into_io()
    }
}

fn requested_hello() -> Result<Hello, Box<dyn Error>> {
    Ok(Hello {
        client_id: ClientId::from_bytes(CLIENT_ID_BYTES)?,
        frame_bytes: u32::try_from(MAXIMUM_TERMINAL_FRAME_BYTES)?,
        input_bytes: u32::try_from(MAXIMUM_INPUT_BYTES)?,
        protocol: CURRENT_PROTOCOL_VERSION,
        row_window_bytes: u32::try_from(MAXIMUM_ROW_WINDOW_BYTES)?,
        supported_optional_features: Vec::new(),
    })
}

fn write_hello(
    stream: &mut impl Write,
    hello: &Hello,
    correlation_id: CorrelationId,
) -> Result<(), Box<dyn Error>> {
    let control = encode_connection_control(&ConnectionControl::Hello(hello.clone()))?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation_id),
        host_epoch: None,
        kind: control.kind,
        optional_message: false,
        payload: &control.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: control.role,
    })?;
    stream.write_all(&record)?;
    Ok(())
}

fn validate_hello_accepted(
    stream: &mut impl Read,
    requested: &Hello,
    correlation_id: CorrelationId,
) -> Result<(orchbee_terminal_protocol::HostEpoch, NegotiatedHostLimits), Box<dyn Error>> {
    let record = read_transport_record(stream)?;
    let envelope = decode_transport_envelope(&record).map_err(io::Error::other)?;
    if envelope.role != MessageRole::Response
        || envelope.correlation_id != Some(correlation_id)
        || envelope.host_epoch.is_none()
        || envelope.optional_message
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
    {
        return Err(invalid_handshake());
    }

    let ConnectionControl::HelloAccepted(accepted) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err(invalid_handshake());
    };
    if envelope.host_epoch != Some(accepted.host_epoch)
        || accepted.protocol != CURRENT_PROTOCOL_VERSION
        || accepted.frame_bytes != requested.frame_bytes
        || accepted.input_bytes != requested.input_bytes
        || accepted.row_window_bytes != requested.row_window_bytes
        || !accepted.supported_optional_features.is_empty()
    {
        return Err(invalid_handshake());
    }
    Ok((
        accepted.host_epoch,
        NegotiatedHostLimits {
            frame_bytes: accepted.frame_bytes,
            input_bytes: accepted.input_bytes,
            row_window_bytes: accepted.row_window_bytes,
        },
    ))
}

fn read_transport_record(stream: &mut impl Read) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
    stream.read_exact(&mut header)?;
    let total_length = usize::try_from(u32::from_le_bytes([
        header[8], header[9], header[10], header[11],
    ]))?;
    if !(TRANSPORT_ENVELOPE_HEADER_BYTES..=MAXIMUM_TRANSPORT_RECORD_BYTES).contains(&total_length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Terminal Host record length is outside the protocol boundary",
        )
        .into());
    }

    let mut record = vec![0_u8; total_length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    stream.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    Ok(record)
}

fn invalid_handshake() -> Box<dyn Error> {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Terminal Host rejected the exact Worker broker handshake",
    )
    .into()
}

/// Accepts a cleared handshake timeout that a shut connection refuses. macOS refuses every
/// socket option with `EINVAL` once a connection is shut in both directions, so a Host that closes
/// right after its Hello would otherwise fail an admission that has completed. A shut connection
/// never waits, so the timeout no longer matters, and the first exchange finds the closure as it
/// does on Linux.
fn unless_shut(cleared: io::Result<()>) -> io::Result<()> {
    match cleared {
        Err(error) if cfg!(target_os = "macos") && error.kind() == io::ErrorKind::InvalidInput => {
            Ok(())
        }
        cleared => cleared,
    }
}
