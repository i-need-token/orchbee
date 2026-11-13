//! One Machine as the hub carries it: a state machine over its phase, its retry history, its one
//! carrier and the `connect` requests waiting on it, with the time and each carrier event as
//! arguments. It returns its effects for the hub to apply; it reads no clock and starts nothing.
//! `docs/machines.md#carrying-a-machine` owns the rules.

mod failure;

pub(super) use failure::{End, Failure, classify, refusal};

use super::lifetime::Moment;
use failure::reconnect_cause;
use orchbee_connection_handshake::ProductVersion;
use orchbee_machine_protocol::{
    Id, MAXIMUM_COUNT, MachineId, SessionName, Time, Token,
    hub::{
        ConnectFailure, ConnectOutcome, ConnectResult, Event, MachineEvent, MachineState,
        OfflineCause, ReconnectCause, Remote, Stage, UnsupportedCause,
    },
};
use orchbee_workbench_client::application::machine_retry::Backoff;
use std::time::{Duration, Instant};

/// An attempt that has not reached `online` by then is ended.
pub(super) const ATTEMPT: Duration = Duration::from_secs(60);
/// The link sends `ping` this long after the last one.
pub(super) const PING_INTERVAL: Duration = Duration::from_secs(30);
/// A `ping` unanswered this long makes the Machine `unresponsive`.
pub(super) const PONG_WAIT: Duration = Duration::from_secs(15);

/// How the hub can reach a catalog record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Disabled,
    /// A connection kind this build does not know.
    Unsupported,
    Ssh(Route),
}

/// Where a carrier goes: the OpenSSH destination and the remote Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Route {
    pub(super) target: String,
    pub(super) session: SessionName,
}

/// A `connect` request waiting for its reply: its connection and its request id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Waiter {
    pub(super) peer: u64,
    pub(super) request: Id,
}

/// What the hub does for a transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Effect {
    /// Start the carrier `carrier` toward `route`.
    Start { carrier: u64, route: Route },
    /// End the carrier `carrier` and everything it started.
    End { carrier: u64 },
    /// Open `ensure-session` for `session` on the carrier's link.
    EnsureSession { carrier: u64, session: SessionName },
    /// Send `ping` on the carrier's link.
    Ping { carrier: u64 },
    /// Reply `result` to every waiter.
    Reply {
        waiters: Vec<Waiter>,
        result: ConnectResult,
    },
}

/// What the hub knows around a transition.
pub(super) struct Context<'a> {
    pub(super) now: Moment,
    /// A `presence` connection is attached.
    pub(super) presence: bool,
    /// The hub is closing: nothing starts and nothing is scheduled.
    pub(super) closing: bool,
    /// The last carrier number given out in the hub's life.
    pub(super) carriers: &'a mut u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Phase {
    Idle,
    Waiting {
        at: Instant,
        cause: Option<ReconnectCause>,
    },
    Attempting(Attempt),
    Online(Link),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Attempt {
    carrier: u64,
    deadline: Instant,
    /// The helper's product version once it answered the offer.
    answered: Option<ProductVersion>,
    /// Reports `reconnecting` with `cause` instead of `connecting`.
    retry: bool,
    cause: Option<ReconnectCause>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Link {
    carrier: u64,
    version: ProductVersion,
    ping_at: Instant,
    /// When the outstanding `ping` makes the Machine `unresponsive`.
    pong_by: Option<Instant>,
}

#[derive(Debug)]
pub(super) struct Machine {
    id: MachineId,
    kind: Kind,
    phase: Phase,
    state: MachineState,
    last_attempt: Option<Time>,
    version: Option<ProductVersion>,
    platform: Option<Token>,
    architecture: Option<Token>,
    backoff: Backoff,
    /// `online` since the last `disconnect`, permanent failure or change: the hub keeps it.
    proven: bool,
    waiters: Vec<Waiter>,
}

impl Machine {
    pub(super) fn new(id: MachineId, kind: Kind) -> Self {
        let state = initial(&kind);
        Self {
            id,
            kind,
            phase: Phase::Idle,
            state,
            last_attempt: None,
            version: None,
            platform: None,
            architecture: None,
            backoff: Backoff::new(),
            proven: false,
            waiters: Vec::new(),
        }
    }

    pub(super) const fn id(&self) -> &MachineId {
        &self.id
    }

    pub(super) const fn state(&self) -> &MachineState {
        &self.state
    }

    /// The record's kind, taken from a Machine read only to be reconciled into another.
    pub(super) fn into_kind(self) -> Kind {
        self.kind
    }

    /// The `machine` event that reports this Machine.
    pub(super) fn event(&self) -> Event {
        Event::Machine(MachineEvent {
            machine: self.id.clone(),
            state: self.state.clone(),
            last_attempt: self.last_attempt,
            remote: Remote::new(
                self.version.clone(),
                self.platform.clone(),
                self.architecture.clone(),
            )
            .ok(),
            draft: false,
        })
    }

    /// The carrier the Machine has, if any.
    pub(super) const fn carrier(&self) -> Option<u64> {
        match &self.phase {
            Phase::Attempting(attempt) => Some(attempt.carrier),
            Phase::Online(link) => Some(link.carrier),
            Phase::Idle | Phase::Waiting { .. } => None,
        }
    }

    /// The carrier and the Session a channel reaches, only while the Machine reads `online`.
    pub(super) fn online_link(&self) -> Option<(u64, &SessionName)> {
        match (&self.kind, &self.phase) {
            (Kind::Ssh(route), Phase::Online(link)) if self.state == MachineState::Online => {
                Some((link.carrier, &route.session))
            }
            _ => None,
        }
    }

    /// When the Machine's next timed transition is due.
    pub(super) fn next_wake(&self) -> Option<Instant> {
        match &self.phase {
            Phase::Idle => None,
            Phase::Waiting { at, .. } => Some(*at),
            Phase::Attempting(attempt) => Some(attempt.deadline),
            Phase::Online(link) => match link.pong_by {
                Some(_) if self.state == MachineState::Unresponsive => None,
                Some(by) => Some(by),
                None => Some(link.ping_at),
            },
        }
    }

    /// A `connect` request: replied at once when the answer is known, otherwise it waits for the
    /// running attempt or starts one.
    pub(super) fn connect(&mut self, waiter: Waiter, cx: &mut Context<'_>) -> Vec<Effect> {
        let known = match (&self.kind, &self.phase) {
            (Kind::Disabled, _) => Some(not_online(ConnectFailure::DisabledMachine)),
            (Kind::Unsupported, _) => Some(not_online(ConnectFailure::UnsupportedMachine)),
            _ if cx.closing => Some(not_online(ConnectFailure::Stopping)),
            (Kind::Ssh(_), Phase::Online(link)) if self.state == MachineState::Online => {
                Some(ConnectOutcome::Online {
                    remote_version: link.version.clone(),
                })
            }
            _ => None,
        };
        if let Some(outcome) = known {
            return vec![self.reply(vec![waiter], outcome)];
        }
        self.waiters.push(waiter);
        match &self.phase {
            Phase::Idle => self.start(false, None, cx),
            Phase::Waiting { cause, .. } => self.start(true, *cause, cx),
            Phase::Attempting(_) | Phase::Online(_) => Vec::new(),
        }
    }

    /// A `presence` connection attached: an idle Machine is attempted silently.
    pub(super) fn attach(&mut self, cx: &mut Context<'_>) -> Vec<Effect> {
        if matches!(self.phase, Phase::Idle) {
            self.start(false, None, cx)
        } else {
            Vec::new()
        }
    }

    /// A reload added the record, or changed it into this Machine: while a `presence` connection
    /// is attached it is attempted as if that connection had just attached.
    pub(super) fn arrived(&mut self, cx: &mut Context<'_>) -> Vec<Effect> {
        if cx.presence {
            self.attach(cx)
        } else {
            Vec::new()
        }
    }

    /// The timed transitions due at `cx.now`.
    pub(super) fn tick(&mut self, cx: &mut Context<'_>) -> Vec<Effect> {
        if cx.closing {
            return Vec::new();
        }
        let now = cx.now.at;
        match &mut self.phase {
            Phase::Waiting { at, cause } if *at <= now => {
                let cause = *cause;
                self.start(true, cause, cx)
            }
            Phase::Attempting(attempt) if attempt.deadline <= now => {
                let cause = if attempt.answered.is_some() {
                    OfflineCause::RemoteNotReady
                } else {
                    OfflineCause::HostTimeout
                };
                let carrier = attempt.carrier;
                self.fail(carrier, Failure::Offline(cause), cx)
            }
            Phase::Online(link) => match link.pong_by {
                Some(by) if by <= now && self.state == MachineState::Online => {
                    self.state = MachineState::Unresponsive;
                    Vec::new()
                }
                None if link.ping_at <= now => {
                    link.ping_at = now + PING_INTERVAL;
                    link.pong_by = Some(now + PONG_WAIT);
                    vec![Effect::Ping {
                        carrier: link.carrier,
                    }]
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// The helper answered the carrier's offer.
    pub(super) fn answered(&mut self, carrier: u64, version: ProductVersion) {
        if let Phase::Attempting(attempt) = &mut self.phase
            && attempt.carrier == carrier
            && attempt.answered.is_none()
        {
            attempt.answered = Some(version.clone());
            self.version = Some(version);
            self.state = attempting(attempt.retry, attempt.cause, Some(Stage::Starting));
        }
    }

    /// The helper's `facts`: a supported platform opens `ensure-session`.
    pub(super) fn facts(
        &mut self,
        carrier: u64,
        platform: Token,
        architecture: Token,
        cx: &mut Context<'_>,
    ) -> Vec<Effect> {
        let Phase::Attempting(attempt) = &self.phase else {
            return Vec::new();
        };
        let Kind::Ssh(route) = &self.kind else {
            return Vec::new();
        };
        if attempt.carrier != carrier || attempt.answered.is_none() {
            return Vec::new();
        }
        let supported =
            platform.as_str() == "windows" && matches!(architecture.as_str(), "x86_64" | "aarch64");
        let session = route.session.clone();
        self.platform = Some(platform);
        self.architecture = Some(architecture);
        if supported {
            vec![Effect::EnsureSession { carrier, session }]
        } else {
            self.fail(
                carrier,
                Failure::Unsupported(UnsupportedCause::RemotePlatform),
                cx,
            )
        }
    }

    /// `ensure-session` closed `ready`, or closed otherwise or was reset.
    pub(super) fn session(
        &mut self,
        carrier: u64,
        ready: bool,
        cx: &mut Context<'_>,
    ) -> Vec<Effect> {
        let Phase::Attempting(Attempt {
            carrier: current,
            answered: Some(version),
            ..
        }) = &self.phase
        else {
            return Vec::new();
        };
        if *current != carrier {
            return Vec::new();
        }
        if !ready {
            return self.fail(carrier, Failure::Offline(OfflineCause::RemoteNotReady), cx);
        }
        let version = version.clone();
        self.phase = Phase::Online(Link {
            carrier,
            version: version.clone(),
            ping_at: cx.now.at + PING_INTERVAL,
            pong_by: None,
        });
        self.backoff.online(cx.now.at);
        self.proven = true;
        self.state = MachineState::Online;
        self.last_attempt = Some(time(cx.now.wall));
        self.reply_all(ConnectOutcome::Online {
            remote_version: version,
        })
        .into_iter()
        .collect()
    }

    /// The helper answered the outstanding `ping`.
    pub(super) fn pong(&mut self, carrier: u64) -> Vec<Effect> {
        let Phase::Online(link) = &mut self.phase else {
            return Vec::new();
        };
        if link.carrier != carrier {
            return Vec::new();
        }
        link.pong_by = None;
        let version = link.version.clone();
        if self.state != MachineState::Unresponsive {
            return Vec::new();
        }
        self.state = MachineState::Online;
        self.reply_all(ConnectOutcome::Online {
            remote_version: version,
        })
        .into_iter()
        .collect()
    }

    /// The attempt failed with `failure`: a refused offer, a wrong first frame, or a platform the
    /// hub does not support.
    pub(super) fn failed(
        &mut self,
        carrier: u64,
        failure: Failure,
        cx: &mut Context<'_>,
    ) -> Vec<Effect> {
        match &self.phase {
            Phase::Attempting(attempt) if attempt.carrier == carrier => {
                self.fail(carrier, failure, cx)
            }
            _ => Vec::new(),
        }
    }

    /// The carrier's output ended before the helper answered: it is ended, and its exit status
    /// then tells what happened.
    pub(super) fn closed(&mut self, carrier: u64, cx: &mut Context<'_>) -> Vec<Effect> {
        match &self.phase {
            Phase::Attempting(attempt) if attempt.carrier == carrier => {
                if attempt.answered.is_some() {
                    self.fail(carrier, Failure::Offline(OfflineCause::RemoteProtocol), cx)
                } else {
                    vec![Effect::End { carrier }]
                }
            }
            _ => self.broken(carrier, cx),
        }
    }

    /// The link broke after the answer: a violation by either end, `bye`, or the end of output.
    pub(super) fn broken(&mut self, carrier: u64, cx: &mut Context<'_>) -> Vec<Effect> {
        match &self.phase {
            Phase::Attempting(attempt) if attempt.carrier == carrier => {
                self.fail(carrier, Failure::Offline(OfflineCause::RemoteProtocol), cx)
            }
            Phase::Online(link) if link.carrier == carrier => self.lost(carrier, cx),
            _ => Vec::new(),
        }
    }

    /// The carrier ended and was reaped.
    pub(super) fn ended(&mut self, carrier: u64, end: &End, cx: &mut Context<'_>) -> Vec<Effect> {
        match &self.phase {
            Phase::Attempting(attempt) if attempt.carrier == carrier => {
                let failure = if attempt.answered.is_some() {
                    Failure::Offline(OfflineCause::RemoteProtocol)
                } else {
                    classify(end)
                };
                self.fail(carrier, failure, cx)
            }
            Phase::Online(link) if link.carrier == carrier => self.lost(carrier, cx),
            _ => Vec::new(),
        }
    }

    /// `disconnect`: the carrier ends, waiters are `cancelled`, the history is forgotten, and an
    /// SSH Machine reads `offline` without a cause until something asks for it again.
    pub(super) fn disconnect(&mut self) -> Vec<Effect> {
        let mut effects: Vec<Effect> = self
            .carrier()
            .map(|carrier| Effect::End { carrier })
            .into_iter()
            .collect();
        self.phase = Phase::Idle;
        self.backoff.reset();
        self.proven = false;
        self.state = initial(&self.kind);
        effects.extend(self.reply_all(not_online(ConnectFailure::Cancelled)));
        effects
    }

    /// A reread catalog holds the Machine as `kind`. A change ends the carrier and the history;
    /// waiters are `cancelled`, the outcome a `disconnect` also gives, or told the record is
    /// disabled or unsupported now. The changed Machine then [arrives](Self::arrived).
    pub(super) fn reconcile(&mut self, kind: Kind, cx: &mut Context<'_>) -> Vec<Effect> {
        if kind == self.kind {
            return Vec::new();
        }
        let mut effects: Vec<Effect> = self
            .carrier()
            .map(|carrier| Effect::End { carrier })
            .into_iter()
            .collect();
        let outcome = match kind {
            Kind::Disabled => ConnectFailure::DisabledMachine,
            Kind::Unsupported => ConnectFailure::UnsupportedMachine,
            Kind::Ssh(_) => ConnectFailure::Cancelled,
        };
        let waiters = std::mem::take(&mut self.waiters);
        *self = Self::new(self.id.clone(), kind);
        if !waiters.is_empty() {
            effects.push(self.reply(waiters, not_online(outcome)));
        }
        effects.extend(self.arrived(cx));
        effects
    }

    /// The catalog no longer holds the Machine: the carrier ends and waiters read `not-found`.
    pub(super) fn remove(&mut self) -> Vec<Effect> {
        let mut effects: Vec<Effect> = self
            .carrier()
            .map(|carrier| Effect::End { carrier })
            .into_iter()
            .collect();
        let waiters = std::mem::take(&mut self.waiters);
        if !waiters.is_empty() {
            effects.push(Effect::Reply {
                waiters,
                result: ConnectResult {
                    outcome: not_online(ConnectFailure::NotFound),
                    state: None,
                },
            });
        }
        effects
    }

    /// The hub entered `closing`: every waiter reads `stopping`; carriers end at its end.
    pub(super) fn closing(&mut self) -> Vec<Effect> {
        self.reply_all(not_online(ConnectFailure::Stopping))
            .into_iter()
            .collect()
    }

    /// The connection `peer` closed: its requests wait no longer.
    pub(super) fn forget(&mut self, peer: u64) {
        self.waiters.retain(|waiter| waiter.peer != peer);
    }

    fn start(
        &mut self,
        retry: bool,
        cause: Option<ReconnectCause>,
        cx: &mut Context<'_>,
    ) -> Vec<Effect> {
        let Kind::Ssh(route) = &self.kind else {
            return Vec::new();
        };
        if cx.closing {
            return Vec::new();
        }
        *cx.carriers += 1;
        let carrier = *cx.carriers;
        let route = route.clone();
        self.phase = Phase::Attempting(Attempt {
            carrier,
            deadline: cx.now.at + ATTEMPT,
            answered: None,
            retry,
            cause,
        });
        self.state = attempting(retry, cause, None);
        vec![Effect::Start { carrier, route }]
    }

    /// Ends the attempt of `carrier` with `failure`.
    fn fail(&mut self, carrier: u64, failure: Failure, cx: &Context<'_>) -> Vec<Effect> {
        let mut effects = vec![Effect::End { carrier }];
        self.last_attempt = Some(time(cx.now.wall));
        let delay = self.backoff.fail(cx.now.at);
        match failure.transient() {
            Some(cause) if !cx.closing && (self.proven || cx.presence) => {
                let cause = Some(reconnect_cause(cause));
                self.phase = Phase::Waiting {
                    at: cx.now.at + delay,
                    cause,
                };
                self.state = MachineState::Reconnecting {
                    cause,
                    stage: None,
                    next_attempt: Some(later(cx.now.wall, delay)),
                };
            }
            transient => {
                if transient.is_none() {
                    self.proven = false;
                }
                self.phase = Phase::Idle;
                self.state = failure.state();
            }
        }
        effects.extend(self.reply_all(not_online(failure.outcome())));
        effects
    }

    /// The link of `carrier` was lost after `online`.
    fn lost(&mut self, carrier: u64, cx: &mut Context<'_>) -> Vec<Effect> {
        let mut effects = vec![Effect::End { carrier }];
        let delay = self.backoff.fail(cx.now.at);
        let cause = Some(ReconnectCause::LinkLost);
        if cx.closing {
            self.phase = Phase::Idle;
            self.state = initial(&self.kind);
        } else if self.waiters.is_empty() {
            self.phase = Phase::Waiting {
                at: cx.now.at + delay,
                cause,
            };
            self.state = MachineState::Reconnecting {
                cause,
                stage: None,
                next_attempt: Some(later(cx.now.wall, delay)),
            };
        } else {
            effects.extend(self.start(true, cause, cx));
        }
        effects
    }

    /// Replies `outcome` to `waiters`, with the state the Machine reads now.
    fn reply(&self, waiters: Vec<Waiter>, outcome: ConnectOutcome) -> Effect {
        Effect::Reply {
            waiters,
            result: ConnectResult {
                outcome,
                state: Some(self.state.name()),
            },
        }
    }

    /// Replies `outcome` to every waiter, if any waits.
    fn reply_all(&mut self, outcome: ConnectOutcome) -> Option<Effect> {
        let waiters = std::mem::take(&mut self.waiters);
        (!waiters.is_empty()).then(|| self.reply(waiters, outcome))
    }
}

/// The state a record reads before any attempt.
fn initial(kind: &Kind) -> MachineState {
    match kind {
        Kind::Disabled => MachineState::Disabled,
        Kind::Unsupported => MachineState::Unsupported {
            cause: Some(UnsupportedCause::ConnectionKind),
        },
        Kind::Ssh(_) => MachineState::Offline {
            cause: None,
            next_attempt: None,
        },
    }
}

/// The state of a running attempt.
const fn attempting(
    retry: bool,
    cause: Option<ReconnectCause>,
    stage: Option<Stage>,
) -> MachineState {
    if retry {
        MachineState::Reconnecting {
            cause,
            stage,
            next_attempt: None,
        }
    } else {
        MachineState::Connecting { stage }
    }
}

const fn not_online(failure: ConnectFailure) -> ConnectOutcome {
    ConnectOutcome::NotOnline(failure)
}

/// The wall-clock time `wall`, in milliseconds since the Unix epoch.
fn time(wall: u64) -> Time {
    Time::new(wall.min(MAXIMUM_COUNT)).expect("bounded by the count range")
}

/// The wall-clock time `delay` after `wall`.
fn later(wall: u64, delay: Duration) -> Time {
    time(wall.saturating_add(u64::try_from(delay.as_millis()).unwrap_or(u64::MAX)))
}

#[cfg(test)]
mod tests;
