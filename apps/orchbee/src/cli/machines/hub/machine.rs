//! One Machine as the hub carries it: a state machine over its phase, its retry history, its one
//! carrier, its login episode, its mute and the `connect` requests waiting on it, with the time,
//! each carrier event, each question and each client's act as arguments. It returns its effects for the hub to apply; it reads no
//! clock and starts nothing. `docs/machines.md#carrying-a-machine` and
//! `docs/machines.md#login-episode` own the rules.

mod episode;
mod failure;
mod remembered;

pub(super) use failure::{End, Failure, classify, refusal};

use super::lifetime::Moment;
use crate::cli::machines::askpass::question::attributed;
use episode::{Episode, Holder};
use failure::reconnect_cause;
use orchbee_connection_handshake::ProductVersion;
use orchbee_machine_protocol::{
    Count, Id, MAXIMUM_COUNT, MachineId, Prompt, SessionName, Time, Token,
    hub::{
        AnswerEndpoint, ClaimOutcome, Claimant, ConnectFailure, ConnectOutcome, ConnectResult,
        ConnectWaiting, DismissOutcome, Event, LoginEnd, LoginEvent, LoginFailedCause, LoginPhase,
        MachineEvent, MachineState, NeedsLoginCause, OfflineCause, QuestionKind, ReconnectCause,
        Remote, Stage, Trigger, UnsupportedCause, WaitReason,
    },
};
use orchbee_workbench_client::application::machine_credentials::SecretKind;
use orchbee_workbench_client::application::machine_retry::Backoff;
use remembered::{Holds, Remembered, Use};
use std::time::{Duration, Instant};

/// An attempt that has not reached `online` by then is ended.
pub(super) const ATTEMPT: Duration = Duration::from_secs(60);
/// The link sends `ping` this long after the last one.
pub(super) const PING_INTERVAL: Duration = Duration::from_secs(30);
/// A `ping` unanswered this long makes the Machine `unresponsive`.
pub(super) const PONG_WAIT: Duration = Duration::from_secs(15);
/// A question nobody answers ends the attempt after this, unless a development or test build
/// shortens it.
pub(super) const QUESTION: Duration = Duration::from_secs(120);

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

/// A `connect` request waiting for its reply: its connection, its request id, whether it can be
/// asked a question, and, for a `command` request while a login episode runs, when it stops
/// waiting for a person.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Waiter {
    pub(super) peer: u64,
    pub(super) request: Id,
    pub(super) answers: Answers,
    pub(super) until: Option<Instant>,
}

impl Waiter {
    /// A request of `peer` that has not started waiting for a person.
    pub(super) const fn new(peer: u64, request: Id, answers: Answers) -> Self {
        Self {
            peer,
            request,
            answers,
            until: None,
        }
    }

    /// A `command` request, which an episode bounds and counts.
    const fn command(&self) -> bool {
        !matches!(self.answers, Answers::User)
    }

    /// Whether `holder` claimed the episode by this very request.
    fn holds(&self, holder: Option<Holder>) -> bool {
        holder
            .is_some_and(|holder| holder.peer == self.peer && holder.request == Some(self.request))
    }
}

/// Whether a waiting request can be asked a question.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Answers {
    /// A `command` without `answers`.
    Never,
    /// A `command` with `answers`: asked as `user` while no connection that `asks` is attached or
    /// the Machine is muted.
    Command,
    /// A `user` request.
    User,
}

/// A question `ssh` asked through a carrier's askpass helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Raised {
    pub(super) kind: QuestionKind,
    pub(super) prompt: Prompt,
    pub(super) answer: AnswerEndpoint,
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
    /// Send `event` to the connection `peer` alone.
    Notify { peer: u64, event: Event },
    /// Close the connection of the carrier's askpass helper: its question is refused or closed.
    Close { carrier: u64 },
    /// Tell the carrier's askpass helper to read the Machine's remembered answer of `kind` from
    /// the credential store itself; the hub never reads it.
    Recall {
        carrier: u64,
        machine: MachineId,
        kind: SecretKind,
    },
    /// Delete the Machine's remembered answer of `kind`: `ssh` refused it.
    Forget {
        machine: MachineId,
        kind: SecretKind,
    },
}

/// What the hub knows around a transition.
pub(super) struct Context<'a> {
    pub(super) now: Moment,
    /// A `presence` connection is attached.
    pub(super) presence: bool,
    /// The hub is closing: nothing starts and nothing is scheduled.
    pub(super) closing: bool,
    /// The connections that `asks`, in ascending order.
    pub(super) askers: &'a [u64],
    /// How long a question put to a claimant stays open unanswered, and how long a `command`
    /// request waits for a person.
    pub(super) question: Duration,
    /// The numbers given out in the hub's life.
    pub(super) serials: &'a mut Serials,
}

impl Context<'_> {
    /// Whether a connection that `asks` is attached.
    const fn asking(&self) -> bool {
        !self.askers.is_empty()
    }
}

/// The last carrier number and the last login id given out in the hub's life.
#[derive(Debug, Default)]
pub(super) struct Serials {
    pub(super) carriers: u64,
    pub(super) logins: u32,
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
    /// When the attempt ends; it moves later by the time a question stayed open.
    deadline: Instant,
    /// The helper's product version once it answered the offer.
    answered: Option<ProductVersion>,
    /// Reports `reconnecting` with `cause` instead of `connecting`.
    retry: bool,
    cause: Option<ReconnectCause>,
    /// The login episode once `ssh` asked a person.
    episode: Option<Episode>,
    /// What the attempt did with the Machine's remembered answers; boxed, as it holds a parked
    /// question and a prompt.
    remembered: Box<Remembered>,
}

impl Attempt {
    /// When the open question goes unanswered, if one is open and put to a claimant.
    fn question_due(&self) -> Option<Instant> {
        self.episode.as_ref().and_then(Episode::due)
    }

    /// Whether a helper waits for an answer: the attempt's clock stops meanwhile.
    fn question_open(&self) -> bool {
        self.episode.as_ref().is_some_and(Episode::question_open)
    }
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
    /// The remembered answers no attempt recalls now; kept through a change of the record.
    holds: Holds,
    /// A `command` episode was dismissed: none is offered until a `user` request or until no
    /// connection asks.
    muted: bool,
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
            holds: Holds::default(),
            muted: false,
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

    /// The running episode, if any.
    const fn episode(&self) -> Option<&Episode> {
        match &self.phase {
            Phase::Attempting(Attempt {
                episode: Some(episode),
                ..
            }) => Some(episode),
            _ => None,
        }
    }

    /// The running episode's login id, if one runs.
    pub(super) fn login(&self) -> Option<Id> {
        self.episode().map(Episode::login)
    }

    /// When the Machine's next timed transition is due.
    pub(super) fn next_wake(&self) -> Option<Instant> {
        let phase = match &self.phase {
            Phase::Idle => None,
            Phase::Waiting { at, .. } => Some(*at),
            // A held question stops the attempt's clock and has no bound of its own: only the
            // waiters' bounds run.
            Phase::Attempting(attempt) => match attempt.question_due() {
                Some(due) => Some(due),
                None if attempt.question_open() => None,
                None => Some(attempt.deadline),
            },
            Phase::Online(link) => match link.pong_by {
                Some(_) if self.state == MachineState::Unresponsive => None,
                Some(by) => Some(by),
                None => Some(link.ping_at),
            },
        };
        let waiting = self.waiters.iter().filter_map(|waiter| waiter.until).min();
        phase.into_iter().chain(waiting).min()
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
        // A person asked: the mute lifts, and an answer held back from automatic attempts may be
        // tried once more.
        if waiter.answers == Answers::User {
            self.unmute();
            self.holds.release();
        }
        match &self.phase {
            Phase::Idle => {
                self.waiters.push(waiter);
                self.start(false, None, cx)
            }
            Phase::Waiting { cause, .. } => {
                let cause = *cause;
                self.waiters.push(waiter);
                self.start(true, cause, cx)
            }
            Phase::Attempting(_) => self.join(waiter, cx),
            Phase::Online(_) => {
                self.waiters.push(waiter);
                Vec::new()
            }
        }
    }

    /// `waiter` joins the running attempt. While an episode runs it starts waiting for a person: a
    /// `user` request claims an offered episode, and the audience reads the new count.
    fn join(&mut self, mut waiter: Waiter, cx: &Context<'_>) -> Vec<Effect> {
        let Some(episode) = self.episode() else {
            self.waiters.push(waiter);
            return Vec::new();
        };
        let claim = waiter.answers == Answers::User && episode.claimant().is_none();
        if waiter.command() {
            waiter.until = Some(cx.now.at + cx.question);
        }
        let mut effects = vec![waiting(&waiter)];
        let holder = Holder {
            peer: waiter.peer,
            request: Some(waiter.request),
        };
        self.waiters.push(waiter);
        if claim {
            effects.extend(self.claimed_by(holder, cx));
        } else {
            effects.extend(self.announce());
        }
        effects
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
        // A `command` request that waited its bound for a person fails alone; an offered episode
        // it leaves without a waiter ends.
        let expired: Vec<Waiter> = self
            .waiters
            .extract_if(.., |waiter| waiter.until.is_some_and(|until| until <= now))
            .collect();
        let mut effects = Vec::new();
        if !expired.is_empty() {
            effects.push(self.reply(expired, not_online(ConnectFailure::AuthRequired)));
            effects.extend(self.review(None, cx));
        }
        effects.extend(self.timed(cx));
        effects
    }

    /// The timed transitions of the phase due at `cx.now`.
    fn timed(&mut self, cx: &mut Context<'_>) -> Vec<Effect> {
        let now = cx.now.at;
        match &mut self.phase {
            Phase::Waiting { at, cause } if *at <= now => {
                let cause = *cause;
                self.start(true, cause, cx)
            }
            // While a question is open only its own bound runs, and only once a claimant was
            // asked.
            Phase::Attempting(attempt) if attempt.question_due().is_some_and(|due| due <= now) => {
                let carrier = attempt.carrier;
                self.conclude(
                    carrier,
                    Failure::NeedsLogin,
                    LoginEnd::Unavailable,
                    Some(ConnectFailure::Timeout),
                    cx,
                )
            }
            Phase::Attempting(attempt) if !attempt.question_open() && attempt.deadline <= now => {
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

    /// The helper answered the carrier's offer: `ssh` authenticated, so an episode ends
    /// `succeeded`.
    pub(super) fn answered(&mut self, carrier: u64, version: ProductVersion) -> Vec<Effect> {
        let Phase::Attempting(attempt) = &mut self.phase else {
            return Vec::new();
        };
        if attempt.carrier != carrier || attempt.answered.is_some() {
            return Vec::new();
        }
        attempt.answered = Some(version.clone());
        if attempt
            .episode
            .as_ref()
            .is_some_and(Episode::answered_secret)
        {
            self.holds.logged_in();
        }
        self.version = Some(version);
        self.state = attempting(attempt.retry, attempt.cause, Some(Stage::Starting));
        self.close_episode(LoginEnd::Succeeded)
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
        self.holds.release();
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

    /// The carrier ended and was reaped. A refusal after the claimant delivered a secret is a
    /// refusal of those answers; one after the remembered password is a refusal of that password.
    /// A remembered passphrase is never refused by the end: its key may be the one refused.
    pub(super) fn ended(&mut self, carrier: u64, end: &End, cx: &mut Context<'_>) -> Vec<Effect> {
        match &self.phase {
            Phase::Attempting(attempt) if attempt.carrier == carrier => {
                let failure = match classify(end) {
                    _ if attempt.answered.is_some() => {
                        Failure::Offline(OfflineCause::RemoteProtocol)
                    }
                    Failure::NeedsLogin | Failure::LoginFailed(_)
                        if attempt
                            .episode
                            .as_ref()
                            .is_some_and(Episode::answered_secret) =>
                    {
                        Failure::LoginFailed(LoginFailedCause::PasswordRejected)
                    }
                    // `ssh` gave up after the remembered password, which it refused.
                    Failure::NeedsLogin | Failure::LoginFailed(_)
                        if attempt.remembered.get(SecretKind::Password) == Use::Given =>
                    {
                        Failure::RememberedRejected
                    }
                    failure => failure,
                };
                let forgotten = (failure == Failure::RememberedRejected).then(|| Effect::Forget {
                    machine: self.id.clone(),
                    kind: SecretKind::Password,
                });
                let mut effects = self.fail(carrier, failure, cx);
                effects.extend(forgotten);
                effects
            }
            Phase::Online(link) if link.carrier == carrier => self.lost(carrier, cx),
            _ => Vec::new(),
        }
    }

    /// `ssh` asked `raised` through the askpass helper of `carrier`, which the hub admitted as the
    /// carrier's only open helper. The kind is the one the prompt reads as for this Machine's
    /// target, so another host's password or new host is `other`. While `recall` says the local
    /// `ssh` is known to be 8.5 or later, the attempt's first question of a kind that may be
    /// remembered, and that no hold keeps back, is held while the helper reads the remembered
    /// answer, before any person is asked or offered the login. A password question after that
    /// answer went to `ssh`, or a passphrase question in the same words, means `ssh` refused it,
    /// which is then deleted and never tried again in this attempt. Otherwise the question is
    /// [posed](Self::pose). A helper that comes while the attempt cannot ask is
    /// [refused](Self::refuse).
    pub(super) fn question(
        &mut self,
        carrier: u64,
        raised: Raised,
        recall: bool,
        cx: &mut Context<'_>,
    ) -> Vec<Effect> {
        let Raised {
            kind,
            prompt,
            answer,
        } = raised;
        let kind = match &self.kind {
            Kind::Ssh(route) => attributed(kind, prompt.as_str(), &route.target),
            Kind::Disabled | Kind::Unsupported => QuestionKind::Other,
        };
        let raised = Raised {
            kind,
            prompt,
            answer,
        };
        let Phase::Attempting(attempt) = &mut self.phase else {
            return self.refuse(carrier, cx);
        };
        if attempt.carrier != carrier || attempt.answered.is_some() {
            return self.refuse(carrier, cx);
        }
        if let Some(kind) = SecretKind::of(&raised.kind) {
            if attempt.remembered.refuses(kind, &raised.prompt) {
                attempt.remembered.set(kind, Use::Refused);
                let mut effects = vec![Effect::Forget {
                    machine: self.id.clone(),
                    kind,
                }];
                effects.extend(self.pose(carrier, raised, true, cx));
                return effects;
            }
            if recall
                && self.holds.recallable(kind)
                && attempt.remembered.get(kind) == Use::Untried
                && attempt.remembered.parked.is_none()
            {
                attempt.remembered.set(kind, Use::Reading);
                attempt.remembered.parked = Some(raised);
                return vec![Effect::Recall {
                    carrier,
                    machine: self.id.clone(),
                    kind,
                }];
            }
        }
        self.pose(carrier, raised, false, cx)
    }

    /// The carrier's helper read the remembered answer its held question asked for and gives it to
    /// `ssh`; the hub learns only that.
    pub(super) fn recalled(&mut self, carrier: u64) -> Vec<Effect> {
        if let Phase::Attempting(attempt) = &mut self.phase
            && attempt.carrier == carrier
            && let Some(raised) = attempt.remembered.parked.take()
            && let Some(kind) = SecretKind::of(&raised.kind)
        {
            attempt.remembered.given(&raised, kind);
        }
        Vec::new()
    }

    /// The deletion of the remembered answer of `kind`, which `ssh` refused, failed or could not
    /// start: no attempt recalls that kind again until a person logs in themselves.
    pub(super) fn undeleted(&mut self, kind: SecretKind) -> Vec<Effect> {
        self.holds.undeleted(kind);
        Vec::new()
    }

    /// Nothing usable is remembered for the held question: it is posed as any other.
    pub(super) fn unrecalled(&mut self, carrier: u64, cx: &mut Context<'_>) -> Vec<Effect> {
        let Phase::Attempting(attempt) = &mut self.phase else {
            return Vec::new();
        };
        if attempt.carrier != carrier {
            return Vec::new();
        }
        let Some(raised) = attempt.remembered.parked.take() else {
            return Vec::new();
        };
        if let Some(kind) = SecretKind::of(&raised.kind) {
            attempt.remembered.set(kind, Use::Spent);
        }
        self.pose(carrier, raised, false, cx)
    }

    /// Puts `raised` to a person. With a claimant the question goes to it, the first one starting
    /// the episode, marked when `ssh` refused the remembered answer to it; without one the attempt
    /// ends in the state the question names and then the helper is closed.
    fn pose(
        &mut self,
        carrier: u64,
        raised: Raised,
        rejected: bool,
        cx: &mut Context<'_>,
    ) -> Vec<Effect> {
        let Phase::Attempting(attempt) = &self.phase else {
            return self.refuse(carrier, cx);
        };
        if attempt.episode.is_some() {
            return self.ask(carrier, raised, rejected, cx);
        }
        let commands = self.waiters.iter().any(Waiter::command);
        let offer = commands && cx.asking() && !self.muted;
        // A person's request claims; otherwise an interface that asks is offered the login, and a
        // `command` that answers itself claims only when no interface can.
        let claimant = self
            .waiters
            .iter()
            .find(|waiter| waiter.answers == Answers::User)
            .or_else(|| {
                self.waiters
                    .iter()
                    .find(|waiter| waiter.answers == Answers::Command && !offer)
            })
            .map(|waiter| Holder {
                peer: waiter.peer,
                request: Some(waiter.request),
            });
        let login = (claimant.is_some() || offer)
            .then(|| {
                let next = cx.serials.logins.checked_add(1)?;
                cx.serials.logins = next;
                Id::new(next)
            })
            .flatten();
        let Some(login) = login else {
            let failure = match raised.kind {
                QuestionKind::HostKey { host_key } => Failure::Untrusted(host_key),
                _ if rejected => Failure::RememberedRejected,
                _ => Failure::NeedsLogin,
            };
            let mut effects = self.fail(carrier, failure, cx);
            // The carrier ends first, then its helper is refused.
            effects.insert(1.min(effects.len()), Effect::Close { carrier });
            return effects;
        };
        let episode = match claimant {
            Some(holder) => Episode::claimed(login, holder),
            None => Episode::offered(login, cx.askers),
        };
        // Every request starts waiting for a person; a `command` one for at most its bound.
        let until = cx.now.at + cx.question;
        let mut effects = Vec::new();
        for waiter in &mut self.waiters {
            if waiter.command() && !waiter.holds(claimant) {
                waiter.until = Some(until);
            }
            effects.push(waiting(waiter));
        }
        if let Phase::Attempting(attempt) = &mut self.phase {
            attempt.episode = Some(episode);
            self.state = attempting(attempt.retry, attempt.cause, Some(Stage::Authenticating));
        }
        effects.extend(self.announce());
        effects.extend(self.ask(carrier, raised, rejected, cx));
        effects
    }

    /// Opens `raised` as the running episode's next question, marked when `ssh` refused the
    /// remembered answer to it, and puts it to the claimant, or holds it while the episode is
    /// offered; a helper that comes while one is open is refused.
    fn ask(
        &mut self,
        carrier: u64,
        raised: Raised,
        rejected: bool,
        cx: &mut Context<'_>,
    ) -> Vec<Effect> {
        let Phase::Attempting(Attempt {
            episode: Some(episode),
            ..
        }) = &mut self.phase
        else {
            return self.refuse(carrier, cx);
        };
        if episode.open(raised, rejected, cx.now.at).is_none() {
            return self.refuse(carrier, cx);
        }
        put(episode, cx).into_iter().collect()
    }

    /// The askpass helper of `carrier` took the answer to the open question: the question closes
    /// and the attempt's clock runs again. A person answered, so an answer held back from
    /// automatic attempts may be tried once more.
    pub(super) fn taken(&mut self, carrier: u64, cx: &Context<'_>) -> Vec<Effect> {
        if let Phase::Attempting(attempt) = &mut self.phase
            && attempt.carrier == carrier
            && let Some(episode) = &mut attempt.episode
            && let Some(since) = episode.take()
        {
            attempt.deadline += cx.now.at.saturating_duration_since(since);
            self.holds.release();
        }
        Vec::new()
    }

    /// The askpass helper of `carrier` left while its question was open: the attempt ends as an
    /// unanswered question does, so `ssh` never goes on with an empty answer.
    pub(super) fn unanswered(&mut self, carrier: u64, cx: &Context<'_>) -> Vec<Effect> {
        match &self.phase {
            // The helper left while it read the store: `ssh` would go on with an empty answer.
            Phase::Attempting(attempt)
                if attempt.carrier == carrier && attempt.remembered.parked.is_some() =>
            {
                self.fail(carrier, Failure::NeedsLogin, cx)
            }
            Phase::Attempting(attempt) if attempt.carrier == carrier && attempt.question_open() => {
                self.conclude(
                    carrier,
                    Failure::NeedsLogin,
                    LoginEnd::Unavailable,
                    Some(ConnectFailure::Timeout),
                    cx,
                )
            }
            _ => Vec::new(),
        }
    }

    /// A helper of `carrier` that cannot be asked: one that could not build its question, sent a
    /// record the hub does not admit, came while another was open, or came when the carrier's
    /// attempt cannot ask. `ssh` would send the empty answer of a helper that exits, so the carrier
    /// ends first, its attempt `auth-required` or its link lost, and then the helper is closed.
    pub(super) fn refuse(&mut self, carrier: u64, cx: &mut Context<'_>) -> Vec<Effect> {
        let mut effects = match &self.phase {
            Phase::Attempting(attempt) if attempt.carrier == carrier => {
                self.fail(carrier, Failure::NeedsLogin, cx)
            }
            Phase::Online(link) if link.carrier == carrier => self.lost(carrier, cx),
            _ => vec![Effect::End { carrier }],
        };
        effects.push(Effect::Close { carrier });
        effects
    }

    /// `claim` of the episode `login` from the connection `peer`: a connection that asks claims an
    /// offered episode, which then goes to it with its held question.
    pub(super) fn claim(
        &mut self,
        peer: u64,
        login: Id,
        cx: &Context<'_>,
    ) -> (ClaimOutcome, Vec<Effect>) {
        let claimant = match self.episode() {
            Some(episode) if episode.login() == login => episode.claimant(),
            _ => return (ClaimOutcome::Ended, Vec::new()),
        };
        match claimant {
            Some(holder) if holder.peer == peer => (ClaimOutcome::Claimed, Vec::new()),
            None if cx.askers.contains(&peer) => {
                let holder = Holder {
                    peer,
                    request: None,
                };
                (ClaimOutcome::Claimed, self.claimed_by(holder, cx))
            }
            Some(_) | None => (ClaimOutcome::Taken, Vec::new()),
        }
    }

    /// `dismiss` of the episode `login` from the connection `peer`: the claimant's, or any asking
    /// connection's while the episode is offered, ends the attempt `auth-required`, and a `command`
    /// episode's mutes the Machine.
    pub(super) fn dismiss(
        &mut self,
        peer: u64,
        login: Id,
        cx: &Context<'_>,
    ) -> (DismissOutcome, Vec<Effect>) {
        let (carrier, allowed, trigger) = match (&self.phase, self.episode()) {
            (Phase::Attempting(attempt), Some(episode)) if episode.login() == login => (
                attempt.carrier,
                match episode.claimant() {
                    Some(holder) => holder.peer == peer,
                    None => cx.askers.contains(&peer),
                },
                episode.trigger(),
            ),
            _ => return (DismissOutcome::Ended, Vec::new()),
        };
        if !allowed {
            return (DismissOutcome::NotClaimant, Vec::new());
        }
        if trigger == Trigger::Command {
            self.muted = true;
        }
        let effects = self.conclude(carrier, Failure::NeedsLogin, LoginEnd::Dismissed, None, cx);
        (DismissOutcome::Dismissed, effects)
    }

    /// `disconnect`: the carrier ends, waiters are `cancelled`, the history is forgotten, and an
    /// SSH Machine reads `offline` without a cause until something asks for it again, or
    /// `needs-login` with cause `disconnected` when it ended a login episode.
    pub(super) fn disconnect(&mut self) -> Vec<Effect> {
        self.hold_handed();
        let login = self.episode().is_some();
        let mut effects: Vec<Effect> = self
            .carrier()
            .map(|carrier| Effect::End { carrier })
            .into_iter()
            .collect();
        effects.extend(self.close_episode(LoginEnd::Unavailable));
        self.phase = Phase::Idle;
        self.backoff.reset();
        self.proven = false;
        self.state = if login {
            MachineState::NeedsLogin {
                cause: Some(NeedsLoginCause::Disconnected),
            }
        } else {
            initial(&self.kind)
        };
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
        self.hold_handed();
        let mut effects: Vec<Effect> = self
            .carrier()
            .map(|carrier| Effect::End { carrier })
            .into_iter()
            .collect();
        effects.extend(self.close_episode(LoginEnd::Unavailable));
        let outcome = match kind {
            Kind::Disabled => ConnectFailure::DisabledMachine,
            Kind::Unsupported => ConnectFailure::UnsupportedMachine,
            Kind::Ssh(_) => ConnectFailure::Cancelled,
        };
        let waiters = std::mem::take(&mut self.waiters);
        let holds = self.holds;
        *self = Self::new(self.id.clone(), kind);
        // The store's items are the Machine id's, whatever the record now says.
        self.holds = holds;
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
        effects.extend(self.close_episode(LoginEnd::Unavailable));
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

    /// The hub entered `closing`: every waiter reads `stopping`; a carrier in an episode ends now,
    /// so `ssh` never goes on with an empty answer, and the others at closing's end.
    pub(super) fn closing(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let Some(carrier) = self.episode().and(self.carrier()) {
            effects.push(Effect::End { carrier });
        }
        effects.extend(self.close_episode(LoginEnd::Unavailable));
        effects.extend(self.reply_all(not_online(ConnectFailure::Stopping)));
        effects
    }

    /// The connection `peer` closed: its requests wait no longer, an episode it shows no longer
    /// goes to it, and one it claimed is offered again or ends `abandoned` with the attempt.
    pub(super) fn forget(&mut self, peer: u64, cx: &Context<'_>) -> Vec<Effect> {
        let before = self.waiters.len();
        self.waiters.retain(|waiter| waiter.peer != peer);
        let left = self.waiters.len() != before;
        if !cx.asking() {
            self.unmute();
        }
        let mut effects = self.review(Some((peer, true)), cx);
        if left && effects.is_empty() {
            effects = self.announce();
        }
        effects
    }

    /// The connection `peer` stopped asking: a `command` episode no longer goes to it and one it
    /// claimed is offered again or ends `abandoned`; with nobody asking any more the mute lifts.
    pub(super) fn stopped_asking(&mut self, peer: u64, cx: &Context<'_>) -> Vec<Effect> {
        if !cx.asking() {
            self.unmute();
        }
        self.review(Some((peer, false)), cx)
    }

    /// The connection `peer` started asking: a running `command` episode goes to it as it stands.
    pub(super) fn asking(&mut self, peer: u64) -> Vec<Effect> {
        let Phase::Attempting(Attempt {
            episode: Some(episode),
            ..
        }) = &mut self.phase
        else {
            return Vec::new();
        };
        if episode.trigger() != Trigger::Command || !episode.join(peer) {
            return Vec::new();
        }
        let episode = episode.clone();
        vec![self.phase_event(&episode, peer)]
    }

    /// Settles the running episode after the connection `peer` left, `closed` telling whether its
    /// connection closed or it stopped asking, or, without `peer`, after waiters left by
    /// themselves. A claimant that left gives a `command` episode back to the connections that ask
    /// while a waiter remains, and otherwise ends it `abandoned`; an offered episode with no
    /// waiter or nobody to ask ends `abandoned` too. Otherwise the audience reads the episode as
    /// it stands now.
    fn review(&mut self, peer: Option<(u64, bool)>, cx: &Context<'_>) -> Vec<Effect> {
        let waiting = self.waiters.iter().any(Waiter::command);
        let Phase::Attempting(Attempt {
            carrier,
            episode: Some(episode),
            ..
        }) = &mut self.phase
        else {
            return Vec::new();
        };
        let carrier = *carrier;
        let command = episode.trigger() == Trigger::Command;
        // The connection that left still reads an end it caused.
        let offered = episode.claimant().is_none();
        if let Some((peer, closed)) = peer {
            // A `user` episode is its claimant's whether it asks or not.
            if !command && !closed {
                return Vec::new();
            }
            if episode.claimant().is_some_and(|holder| holder.peer == peer) {
                if !(command && waiting && cx.asking()) {
                    return self.abandon(carrier, cx);
                }
                episode.leave(peer);
                episode.release();
                return self.announce();
            }
            if offered && !(waiting && cx.asking()) {
                return self.abandon(carrier, cx);
            }
            episode.leave(peer);
            return Vec::new();
        }
        if offered && !waiting {
            return self.abandon(carrier, cx);
        }
        self.announce()
    }

    /// Ends the attempt of `carrier` and its episode `abandoned`: nobody is left to answer.
    fn abandon(&mut self, carrier: u64, cx: &Context<'_>) -> Vec<Effect> {
        self.conclude(carrier, Failure::NeedsLogin, LoginEnd::Abandoned, None, cx)
    }

    /// `holder` claims the offered episode: the audience reads who holds it and the held question
    /// goes to the claimant, its bound starting now.
    fn claimed_by(&mut self, holder: Holder, cx: &Context<'_>) -> Vec<Effect> {
        let Phase::Attempting(Attempt {
            episode: Some(episode),
            ..
        }) = &mut self.phase
        else {
            return Vec::new();
        };
        episode.claim(holder);
        let question = put(episode, cx);
        let mut effects = self.announce();
        effects.extend(question);
        effects
    }

    /// Lifts the mute; a Machine that read `login-dismissed` reads `auth-required` again.
    fn unmute(&mut self) {
        self.muted = false;
        if self.state
            == (MachineState::NeedsLogin {
                cause: Some(NeedsLoginCause::LoginDismissed),
            })
        {
            self.state = MachineState::NeedsLogin {
                cause: Some(NeedsLoginCause::AuthRequired),
            };
        }
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
        cx.serials.carriers += 1;
        let carrier = cx.serials.carriers;
        let route = route.clone();
        self.phase = Phase::Attempting(Attempt {
            carrier,
            deadline: cx.now.at + ATTEMPT,
            answered: None,
            retry,
            cause,
            episode: None,
            remembered: Box::default(),
        });
        self.state = attempting(retry, cause, None);
        vec![Effect::Start { carrier, route }]
    }

    /// Ends the attempt of `carrier` with `failure`; an episode ends `failed` when `ssh` refused
    /// the claimant's answers and `unavailable` otherwise.
    fn fail(&mut self, carrier: u64, failure: Failure, cx: &Context<'_>) -> Vec<Effect> {
        let end = match failure {
            Failure::LoginFailed(LoginFailedCause::PasswordRejected) => LoginEnd::Failed,
            _ => LoginEnd::Unavailable,
        };
        self.conclude(carrier, failure, end, None, cx)
    }

    /// Ends the attempt of `carrier` with `failure` and its episode with `end`, and replies
    /// `outcome`, or the failure's own outcome, to every waiter. The carrier ends before an open
    /// helper is closed, so `ssh` never reads the empty answer a refused helper leaves.
    fn conclude(
        &mut self,
        carrier: u64,
        failure: Failure,
        end: LoginEnd,
        outcome: Option<ConnectFailure>,
        cx: &Context<'_>,
    ) -> Vec<Effect> {
        self.hold_handed();
        let mut effects = vec![Effect::End { carrier }];
        effects.extend(self.close_episode(end));
        self.last_attempt = Some(time(cx.now.wall));
        let delay = self.backoff.fail(cx.now.at);
        let outcome = outcome.unwrap_or_else(|| failure.outcome());
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
                if self.muted && matches!(self.state, MachineState::NeedsLogin { .. }) {
                    self.state = MachineState::NeedsLogin {
                        cause: Some(NeedsLoginCause::LoginDismissed),
                    };
                }
            }
        }
        effects.extend(self.reply_all(not_online(outcome)));
        effects
    }

    /// The running attempt ends before the link handshake: each kind of remembered answer it
    /// handed to `ssh` is held back from later attempts, so an end that is no refusal, such as a
    /// server that drops the connection after too many tries, never sends it again by itself.
    fn hold_handed(&mut self) {
        let Phase::Attempting(attempt) = &self.phase else {
            return;
        };
        if attempt.answered.is_some() {
            return;
        }
        for kind in attempt.remembered.handed() {
            self.holds.withhold(kind);
        }
    }

    /// Ends the running episode, if any, with `end`: an open question's helper is closed, the
    /// audience receives the ended `login`, and the waiters no longer wait for a person.
    fn close_episode(&mut self, end: LoginEnd) -> Vec<Effect> {
        let Phase::Attempting(attempt) = &mut self.phase else {
            return Vec::new();
        };
        let carrier = attempt.carrier;
        let mut effects = Vec::new();
        // A question held while its helper reads the store closes with the attempt.
        if attempt.remembered.parked.take().is_some() {
            effects.push(Effect::Close { carrier });
        }
        let Some(episode) = attempt.episode.take() else {
            return effects;
        };
        if episode.question_open() {
            effects.push(Effect::Close { carrier });
        }
        for waiter in &mut self.waiters {
            waiter.until = None;
        }
        effects.extend(
            episode
                .audience()
                .iter()
                .map(|peer| self.login_event(&episode, *peer, LoginPhase::Ended { end })),
        );
        effects
    }

    /// The running episode's `login` event, as it stands, to every connection it goes to.
    fn announce(&self) -> Vec<Effect> {
        let Some(episode) = self.episode() else {
            return Vec::new();
        };
        episode
            .audience()
            .iter()
            .map(|peer| self.phase_event(episode, *peer))
            .collect()
    }

    /// The `login` event of `episode` as it stands, for the connection `peer`.
    fn phase_event(&self, episode: &Episode, peer: u64) -> Effect {
        let phase = match episode.claimant() {
            None => LoginPhase::Offered,
            Some(holder) => LoginPhase::Claimed {
                claim: if holder.peer == peer {
                    Claimant::You
                } else {
                    Claimant::Other
                },
            },
        };
        self.login_event(episode, peer, phase)
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

    /// The `login` event of `episode` in `phase` for the connection `peer`; `waiters` counts the
    /// `command` requests waiting, the claimant's own excluded.
    fn login_event(&self, episode: &Episode, peer: u64, phase: LoginPhase) -> Effect {
        let claimant = episode.claimant();
        let waiters = self
            .waiters
            .iter()
            .filter(|waiter| waiter.command() && !waiter.holds(claimant))
            .count();
        Effect::Notify {
            peer,
            event: Event::Login(LoginEvent {
                login: episode.login(),
                machine: self.id.clone(),
                trigger: episode.trigger(),
                phase,
                waiters: count(waiters),
            }),
        }
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

/// `connect-waiting` for `waiter`: its request now waits for a person.
const fn waiting(waiter: &Waiter) -> Effect {
    Effect::Notify {
        peer: waiter.peer,
        event: Event::ConnectWaiting(ConnectWaiting {
            request: waiter.request,
            reason: WaitReason::Login,
        }),
    }
}

/// Puts the open question of `episode` to its claimant, unanswered after the question bound.
fn put(episode: &mut Episode, cx: &Context<'_>) -> Option<Effect> {
    episode
        .put(cx.now.at + cx.question)
        .map(|(peer, question)| Effect::Notify {
            peer,
            event: Event::Question(question),
        })
}

/// The wall-clock time `wall`, in milliseconds since the Unix epoch.
fn time(wall: u64) -> Time {
    Time::new(wall.min(MAXIMUM_COUNT)).expect("bounded by the count range")
}

/// The wall-clock time `delay` after `wall`.
fn later(wall: u64, delay: Duration) -> Time {
    time(wall.saturating_add(u64::try_from(delay.as_millis()).unwrap_or(u64::MAX)))
}

/// `value` as a count; a count of requests never reaches the bound.
fn count(value: usize) -> Count {
    Count::new(
        u64::try_from(value)
            .unwrap_or(MAXIMUM_COUNT)
            .min(MAXIMUM_COUNT),
    )
    .expect("bounded by the count range")
}

#[cfg(test)]
mod tests;
