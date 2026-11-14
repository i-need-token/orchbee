//! One Machine as the hub carries it: a state machine over its phase, its retry history, its one
//! carrier, its login episode and the `connect` requests waiting on it, with the time, each carrier
//! event and each question as arguments. It returns its effects for the hub to apply; it reads no
//! clock and starts nothing. `docs/machines.md#carrying-a-machine` and
//! `docs/machines.md#login-episode` own the rules.

mod episode;
mod failure;
mod remembered;

pub(super) use failure::{End, Failure, classify, refusal};

use super::lifetime::Moment;
use crate::cli::machines::askpass::question::attributed;
use episode::{Episode, Tag};
use failure::reconnect_cause;
use orchbee_connection_handshake::ProductVersion;
use orchbee_machine_protocol::{
    Count, Id, MAXIMUM_COUNT, MachineId, Prompt, SessionName, Time, Token,
    hub::{
        AnswerEndpoint, ClaimOutcome, Claimant, ConnectFailure, ConnectOutcome, ConnectResult,
        ConnectWaiting, DismissOutcome, Event, LoginEnd, LoginEvent, LoginFailedCause, LoginPhase,
        MachineEvent, MachineState, NeedsLoginCause, OfflineCause, Question, QuestionKind,
        ReconnectCause, Remote, Stage, Trigger, UnsupportedCause, WaitReason,
    },
};
use orchbee_workbench_client::application::machine_credentials::SecretKind;
use orchbee_workbench_client::application::machine_retry::Backoff;
use remembered::{Remembered, Use};
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

/// A `connect` request waiting for its reply: its connection, its request id, and whether it can
/// be asked a question.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Waiter {
    pub(super) peer: u64,
    pub(super) request: Id,
    pub(super) answers: Answers,
}

/// Whether a waiting request can be asked a question.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Answers {
    /// A `command` without `answers`.
    Never,
    /// A `command` with `answers`: asked as `user` while no connection that `asks` is attached.
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
    /// A connection that `asks` is attached.
    pub(super) asking: bool,
    /// How long a question stays open unanswered.
    pub(super) question: Duration,
    /// The numbers given out in the hub's life.
    pub(super) serials: &'a mut Serials,
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
    /// What the attempt did with the Machine's remembered answers.
    remembered: Remembered,
}

impl Attempt {
    /// When the open question goes unanswered, if one is open.
    fn question_due(&self) -> Option<Instant> {
        self.episode.as_ref().and_then(Episode::due)
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
        match &self.phase {
            Phase::Idle => None,
            Phase::Waiting { at, .. } => Some(*at),
            Phase::Attempting(attempt) => Some(attempt.question_due().unwrap_or(attempt.deadline)),
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
            // The claimant learns the new count of requests waiting on its episode.
            Phase::Attempting(_) => self.recount(),
            Phase::Online(_) => Vec::new(),
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
            // While a question is open only its own bound runs.
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
            Phase::Attempting(attempt)
                if attempt.question_due().is_none() && attempt.deadline <= now =>
            {
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
    /// refusal of those answers.
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
                    // `ssh` gave up after the remembered answer, which it refused.
                    Failure::NeedsLogin | Failure::LoginFailed(_)
                        if attempt.remembered.given().next().is_some() =>
                    {
                        Failure::RememberedRejected
                    }
                    failure => failure,
                };
                let forgotten: Vec<Effect> = if failure == Failure::RememberedRejected {
                    attempt
                        .remembered
                        .given()
                        .map(|kind| Effect::Forget {
                            machine: self.id.clone(),
                            kind,
                        })
                        .collect()
                } else {
                    Vec::new()
                };
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
    /// target, so another host's password or new host is `other`. The attempt's first question of
    /// a kind that may be remembered is held while the helper reads the remembered answer; a
    /// second one after that answer went to `ssh` means `ssh` refused it, which is then deleted and
    /// never tried again in this attempt. Otherwise the question is [posed](Self::pose). A helper
    /// that comes while the attempt cannot ask is [refused](Self::refuse).
    pub(super) fn question(
        &mut self,
        carrier: u64,
        raised: Raised,
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
            match attempt.remembered.get(kind) {
                Use::Untried if attempt.remembered.parked.is_none() => {
                    attempt.remembered.set(kind, Use::Reading);
                    attempt.remembered.parked = Some(raised);
                    return vec![Effect::Recall {
                        carrier,
                        machine: self.id.clone(),
                        kind,
                    }];
                }
                Use::Given => {
                    attempt.remembered.set(kind, Use::Spent);
                    let mut effects = vec![Effect::Forget {
                        machine: self.id.clone(),
                        kind,
                    }];
                    effects.extend(self.pose(carrier, raised, true, cx));
                    return effects;
                }
                Use::Untried | Use::Reading | Use::Spent => {}
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
            attempt.remembered.set(kind, Use::Given);
        }
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
        let claimant = self
            .waiters
            .iter()
            .find(|waiter| match waiter.answers {
                Answers::User => true,
                Answers::Command => !cx.asking,
                Answers::Never => false,
            })
            .cloned();
        let login = claimant.as_ref().and_then(|_| {
            let next = cx.serials.logins.checked_add(1)?;
            cx.serials.logins = next;
            Id::new(next)
        });
        let (Some(claimant), Some(login)) = (claimant, login) else {
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
        let episode = Episode::new(login, claimant.clone());
        let mut effects = vec![
            Effect::Notify {
                peer: claimant.peer,
                event: Event::ConnectWaiting(ConnectWaiting {
                    request: claimant.request,
                    reason: WaitReason::Login,
                }),
            },
            self.login_event(
                &episode,
                LoginPhase::Claimed {
                    claim: Claimant::You,
                },
            ),
        ];
        if let Phase::Attempting(attempt) = &mut self.phase {
            attempt.episode = Some(episode);
            self.state = attempting(attempt.retry, attempt.cause, Some(Stage::Authenticating));
        }
        effects.extend(self.ask(carrier, raised, rejected, cx));
        effects
    }

    /// Puts `raised` to the claimant of the running episode as its next question; a helper that
    /// comes while one is open is refused.
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
        let due = cx.now.at + cx.question;
        let Some((question, repeated)) = episode.open(Tag::of(&raised.kind), cx.now.at, due) else {
            return self.refuse(carrier, cx);
        };
        vec![Effect::Notify {
            peer: episode.claimant().peer,
            event: Event::Question(Question {
                login: episode.login(),
                question,
                kind: raised.kind,
                prompt: raised.prompt,
                repeated,
                remembered_rejected: rejected,
                answer: raised.answer,
            }),
        }]
    }

    /// The askpass helper of `carrier` took the answer to the open question: the question closes
    /// and the attempt's clock runs again.
    pub(super) fn taken(&mut self, carrier: u64, cx: &Context<'_>) -> Vec<Effect> {
        if let Phase::Attempting(attempt) = &mut self.phase
            && attempt.carrier == carrier
            && let Some(episode) = &mut attempt.episode
            && let Some(since) = episode.take()
        {
            attempt.deadline += cx.now.at.saturating_duration_since(since);
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
            Phase::Attempting(attempt)
                if attempt.carrier == carrier && attempt.question_due().is_some() =>
            {
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

    /// `claim` of the episode `login` from the connection `peer`.
    pub(super) fn claim(&self, peer: u64, login: Id) -> ClaimOutcome {
        match self.episode() {
            Some(episode) if episode.login() == login && episode.claimant().peer == peer => {
                ClaimOutcome::Claimed
            }
            Some(episode) if episode.login() == login => ClaimOutcome::Taken,
            _ => ClaimOutcome::Ended,
        }
    }

    /// `dismiss` of the episode `login` from the connection `peer`: the claimant's ends the
    /// attempt `auth-required`.
    pub(super) fn dismiss(
        &mut self,
        peer: u64,
        login: Id,
        cx: &Context<'_>,
    ) -> (DismissOutcome, Vec<Effect>) {
        let (carrier, claimant) = match (&self.phase, self.episode()) {
            (Phase::Attempting(attempt), Some(episode)) if episode.login() == login => {
                (attempt.carrier, episode.claimant().peer)
            }
            _ => return (DismissOutcome::Ended, Vec::new()),
        };
        if claimant != peer {
            return (DismissOutcome::NotClaimant, Vec::new());
        }
        let effects = self.conclude(carrier, Failure::NeedsLogin, LoginEnd::Dismissed, None, cx);
        (DismissOutcome::Dismissed, effects)
    }

    /// `disconnect`: the carrier ends, waiters are `cancelled`, the history is forgotten, and an
    /// SSH Machine reads `offline` without a cause until something asks for it again, or
    /// `needs-login` with cause `disconnected` when it ended a login episode.
    pub(super) fn disconnect(&mut self) -> Vec<Effect> {
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

    /// The connection `peer` closed: its requests wait no longer, and an episode it claimed ends
    /// `abandoned` with the attempt.
    pub(super) fn forget(&mut self, peer: u64, cx: &Context<'_>) -> Vec<Effect> {
        let before = self.waiters.len();
        self.waiters.retain(|waiter| waiter.peer != peer);
        let Phase::Attempting(attempt) = &self.phase else {
            return Vec::new();
        };
        let carrier = attempt.carrier;
        match self.episode() {
            Some(episode) if episode.claimant().peer == peer => {
                self.conclude(carrier, Failure::NeedsLogin, LoginEnd::Abandoned, None, cx)
            }
            Some(_) if self.waiters.len() != before => self.recount(),
            _ => Vec::new(),
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
            remembered: Remembered::default(),
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
            }
        }
        effects.extend(self.reply_all(not_online(outcome)));
        effects
    }

    /// Ends the running episode, if any, with `end`: an open question's helper is closed and the
    /// claimant receives the ended `login`.
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
        if episode.due().is_some() {
            effects.push(Effect::Close { carrier });
        }
        effects.push(self.login_event(&episode, LoginPhase::Ended { end }));
        effects
    }

    /// The running episode's `login` event again, after the requests waiting on it changed.
    fn recount(&self) -> Vec<Effect> {
        self.episode()
            .map(|episode| {
                self.login_event(
                    episode,
                    LoginPhase::Claimed {
                        claim: Claimant::You,
                    },
                )
            })
            .into_iter()
            .collect()
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

    /// The `login` event of `episode` in `phase`, for its claimant alone; `waiters` counts the
    /// other requests that are no `user` request.
    fn login_event(&self, episode: &Episode, phase: LoginPhase) -> Effect {
        let claimant = episode.claimant();
        let waiters = self
            .waiters
            .iter()
            .filter(|waiter| *waiter != claimant && waiter.answers != Answers::User)
            .count();
        Effect::Notify {
            peer: claimant.peer,
            event: Event::Login(LoginEvent {
                login: episode.login(),
                machine: self.id.clone(),
                trigger: Trigger::User,
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
