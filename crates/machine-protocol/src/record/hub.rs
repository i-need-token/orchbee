//! The records of `machine-hub` generation 1, the connection from a TUI, a CLI command, or
//! Desktop's bundled Client to the hub: the requests and their replies, the state events, the
//! login episode and its questions, and the channel kinds `session` and `route`. The
//! capability-gated `install` request, the `connect` member `install`, the `machine` members
//! `offer` and `transfer`, the state `installing`, and the wait reason `install-consent` are not
//! part of this build: they decode as unknown types, unknown members, or growable fallbacks.
//!
//! A member whose presence depends on a growable value — a state, outcome, or question kind — is
//! checked against a known value and admitted, then dropped, with a value this build does not
//! know, since the receiver reads that value as its fallback and knows nothing of what it implies.

use orchbee_connection_handshake::ProductVersion;
use serde::{Deserialize, Serialize};

use crate::{
    AnswerToken, Count, HostKey, Id, InvalidValue, MachineId, Name, PathText, Prompt, ResetReason,
    SessionName, Target, Time, Token,
    protocol::{KindRules, Members, Protocol, Refused, sealed::Codec},
    record::{closed, members_of},
    value::{SHARED_RESET_REASONS, True, name_enum},
};

/// The `machine-hub` connection: initiated by a client, accepted by the hub of the client's user
/// and environment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Hub {}

impl Protocol for Hub {}

/// The most machine ids one `catalog` event lists.
pub const MAXIMUM_CATALOG_MACHINES: usize = 64;

name_enum!(
    /// A hub channel kind; clients open every one, for a Machine that is `online`.
    pub enum ChannelKind {
        /// Relayed to a link `session` channel for the Machine's Session.
        Session = "session",
        /// Relayed to a link `route` channel.
        Route = "route",
    }
);

/// A hub channel kind with its `open` members.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Channel {
    /// The Machine's Session, through the link.
    Session {
        /// The Machine.
        machine: MachineId,
    },
    /// One routed command on the Machine.
    Route {
        /// The Machine.
        machine: MachineId,
    },
}

impl Channel {
    /// The channel's kind.
    #[must_use]
    pub const fn kind(&self) -> ChannelKind {
        match self {
            Self::Session { .. } => ChannelKind::Session,
            Self::Route { .. } => ChannelKind::Route,
        }
    }

    /// The Machine the channel reaches.
    #[must_use]
    pub const fn machine(&self) -> &MachineId {
        match self {
            Self::Session { machine } | Self::Route { machine } => machine,
        }
    }
}

name_enum!(
    /// A hub channel's `close` outcome; it grows with fallback `unknown`. `session` lists `ended`,
    /// `failed`, and `lost`; `route` lists `replied` and `lost`.
    pub enum Outcome {
        /// The link channel ended.
        Ended = "ended",
        /// The link channel failed.
        Failed = "failed",
        /// The link was lost after the channel could have caused an effect.
        Lost = "lost",
        /// The reply packet was relayed.
        Replied = "replied",
        /// An outcome this build does not know.
        Unknown = "unknown",
    }
);

/// What a client is to the hub; each `attach` replaces the last, and a client that never sends one
/// has all three false.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Attach {
    /// The connection holds the hub's lifetime until it closes.
    pub presence: bool,
    /// The client shows command-triggered login notices and may claim them.
    pub asks: bool,
    /// The client receives `hub`, `catalog`, and `machine` events.
    pub watch: bool,
}

/// Who asked for a connection and whether it can answer questions itself.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Origin {
    /// A person asked; the requester shows questions and delivers answers itself.
    User,
    /// A command asked.
    Command {
        /// The requester itself can show questions and deliver answers.
        answers: bool,
    },
}

/// A Machine not yet in the catalog, connected under the id it will be saved with.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Draft {
    /// The `ssh` connection target.
    pub target: Target,
    /// The Session the Machine will use.
    pub session: SessionName,
}

/// Asks for a Machine to be `online`; replies when it is or when the attempt ends.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Connect {
    /// The Machine.
    pub machine: MachineId,
    /// Who asked.
    pub origin: Origin,
    /// The draft Machine, for one not yet in the catalog.
    pub draft: Option<Draft>,
}

/// A request a client sends to the hub.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Request {
    /// States what the client is to the hub.
    Attach(Attach),
    /// Asks the hub to reread the catalog now.
    Reload,
    /// Asks for a Machine to be `online`.
    Connect(Connect),
    /// Ends a Machine's login and link and leaves its remote Session running.
    Disconnect {
        /// The Machine.
        machine: MachineId,
    },
    /// Claims a login episode.
    Claim {
        /// The episode.
        login: Id,
    },
    /// Dismisses a login episode.
    Dismiss {
        /// The episode.
        login: Id,
    },
    /// Stops a Machine's remote Session and every terminal in it.
    StopSession {
        /// The Machine.
        machine: MachineId,
    },
}

/// The hub's lifetime state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HubState {
    /// A holder exists.
    Held,
    /// No holder exists; every login ends at `expires_at`.
    Expiring {
        /// When the hub starts closing.
        expires_at: Time,
    },
    /// The hub is closing and never returns to another state.
    Closing,
}

/// The `attach-result` outcome.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AttachOutcome {
    /// Attached; the hub's lifetime state at the reply.
    Attached {
        /// The members of the `hub` event.
        hub: HubState,
    },
    /// The hub is closing.
    Stopping,
    /// An outcome this build does not know.
    Unknown,
}

/// The `reload-result` outcome; after a failure the hub keeps the catalog it had.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReloadOutcome {
    /// The catalog was reread.
    Reloaded {
        /// The `revision` of `machines.json` now in use.
        revision: Count,
    },
    /// The catalog file is invalid.
    InvalidCatalog,
    /// The catalog could not be read.
    Storage,
    /// The hub is closing.
    Stopping,
    /// An outcome this build does not know.
    Unknown,
}

name_enum!(
    /// Every `connect` outcome except `online`; it grows with fallback `unknown`.
    pub enum ConnectFailure {
        /// The record is disabled.
        DisabledMachine = "disabled-machine",
        /// The host could not be reached, resolved, or connected in time.
        HostUnreachable = "host-unreachable",
        /// No usable `ssh` client.
        SshUnavailable = "ssh-unavailable",
        /// `ssh` failed for a reason no diagnostic matched.
        SshFailed = "ssh-failed",
        /// The remote end broke the link protocol.
        RemoteProtocol = "remote-protocol",
        /// The remote Session did not report ready.
        RemoteNotReady = "remote-not-ready",
        /// A person must answer a question.
        AuthRequired = "auth-required",
        /// `ssh` gave up after refused answers or a refused key.
        AuthFailed = "auth-failed",
        /// The host key is not in `known_hosts`.
        HostUntrusted = "host-untrusted",
        /// The host key differs from `known_hosts`.
        HostKeyChanged = "host-key-changed",
        /// The remote has no Orchbee of this environment.
        RemoteNotInstalled = "remote-not-installed",
        /// The remote system is not supported.
        RemoteUnsupported = "remote-unsupported",
        /// The record's connection kind is not supported.
        UnsupportedMachine = "unsupported-machine",
        /// One side is too old for the other.
        VersionIncompatible = "version-incompatible",
        /// The remote Orchbee belongs to another environment.
        RemoteOtherEnvironment = "remote-other-environment",
        /// The catalog holds no such Machine.
        NotFound = "not-found",
        /// The draft is invalid.
        InvalidDraft = "invalid-draft",
        /// A `disconnect` ended the attempt.
        Cancelled = "cancelled",
        /// The attempt timed out.
        Timeout = "timeout",
        /// Installing Orchbee on the remote failed.
        RemoteInstallFailed = "remote-install-failed",
        /// The hub failed internally.
        Internal = "internal",
        /// The hub is closing.
        Stopping = "stopping",
        /// An outcome this build does not know.
        Unknown = "unknown",
    }
);

/// The `connect-result` outcome.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ConnectOutcome {
    /// The Machine is `online`.
    Online {
        /// The remote product version.
        remote_version: ProductVersion,
    },
    /// The attempt ended without the Machine `online`.
    NotOnline(ConnectFailure),
}

/// The reply to `connect`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ConnectResult {
    /// The outcome.
    pub outcome: ConnectOutcome,
    /// The Machine's state name at the reply, whenever the hub knows the Machine.
    pub state: Option<MachineStateName>,
}

name_enum!(
    /// The `disconnect-result` outcome; it grows with fallback `unknown`.
    pub enum DisconnectOutcome {
        /// The login and link ended.
        Disconnected = "disconnected",
        /// The catalog holds no such Machine.
        NotFound = "not-found",
        /// The hub is closing.
        Stopping = "stopping",
        /// An outcome this build does not know.
        Unknown = "unknown",
    }
);

name_enum!(
    /// The `claim-result` outcome; it grows with fallback `unknown`.
    pub enum ClaimOutcome {
        /// The episode is now the requester's.
        Claimed = "claimed",
        /// Another client claimed it first.
        Taken = "taken",
        /// The episode ended, or the hub does not know it.
        Ended = "ended",
        /// The hub is closing.
        Stopping = "stopping",
        /// An outcome this build does not know.
        Unknown = "unknown",
    }
);

name_enum!(
    /// The `dismiss-result` outcome; it grows with fallback `unknown`.
    pub enum DismissOutcome {
        /// The episode ended dismissed.
        Dismissed = "dismissed",
        /// Another client claimed the episode.
        NotClaimant = "not-claimant",
        /// The episode ended, or the hub does not know it.
        Ended = "ended",
        /// The hub is closing.
        Stopping = "stopping",
        /// An outcome this build does not know.
        Unknown = "unknown",
    }
);

name_enum!(
    /// The `stop-session-result` outcome; it grows with fallback `unknown`.
    pub enum StopSessionOutcome {
        /// The remote Session stopped.
        Stopped = "stopped",
        /// The remote Session was not running.
        NotRunning = "not-running",
        /// The Machine is not `online`.
        NotOnline = "not-online",
        /// The catalog holds no such Machine.
        NotFound = "not-found",
        /// The stop failed.
        Failed = "failed",
        /// The link was lost after submission, or an outcome this build does not know.
        Unknown = "unknown",
        /// The hub is closing.
        Stopping = "stopping",
    }
);

/// A reply the hub sends to a request.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Reply {
    /// `attach-result`.
    Attach(AttachOutcome),
    /// `reload-result`.
    Reload(ReloadOutcome),
    /// `connect-result`.
    Connect(ConnectResult),
    /// `disconnect-result`.
    Disconnect(DisconnectOutcome),
    /// `claim-result`.
    Claim(ClaimOutcome),
    /// `dismiss-result`.
    Dismiss(DismissOutcome),
    /// `stop-session-result`.
    StopSession(StopSessionOutcome),
}

/// The catalog the hub serves: at most 64 machine ids in catalog order. A Machine absent from the
/// newest catalog is gone.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Catalog {
    revision: Count,
    machines: Vec<MachineId>,
}

impl Catalog {
    /// The catalog of `revision` listing `machines`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] for more than 64 machine ids.
    pub fn new(revision: Count, machines: Vec<MachineId>) -> Result<Self, InvalidValue> {
        if machines.len() <= MAXIMUM_CATALOG_MACHINES {
            Ok(Self { revision, machines })
        } else {
            Err(InvalidValue)
        }
    }

    /// The `revision` of `machines.json`.
    #[must_use]
    pub const fn revision(&self) -> Count {
        self.revision
    }

    /// The machine ids in catalog order.
    #[must_use]
    pub fn machines(&self) -> &[MachineId] {
        &self.machines
    }
}

name_enum!(
    /// The name of a Machine state; it grows with fallback `unknown`.
    pub enum MachineStateName {
        /// The record is disabled.
        Disabled = "disabled",
        /// A first attempt is running.
        Connecting = "connecting",
        /// The link is up and the Machine's Session answered ready.
        Online = "online",
        /// The hub retries by itself.
        Reconnecting = "reconnecting",
        /// No link and no attempt in progress.
        Offline = "offline",
        /// A `ping` went unanswered for 15 seconds.
        Unresponsive = "unresponsive",
        /// A person must answer a question.
        NeedsLogin = "needs-login",
        /// `ssh` gave up after refused answers or a refused key.
        LoginFailed = "login-failed",
        /// The host key is not in `known_hosts`.
        Untrusted = "untrusted",
        /// The host key differs from `known_hosts`.
        KeyChanged = "key-changed",
        /// The remote has no Orchbee of this environment.
        NotInstalled = "not-installed",
        /// The remote system or the record's connection kind is not supported.
        Unsupported = "unsupported",
        /// The remote Orchbee or its Session is too old.
        NeedsUpdate = "needs-update",
        /// This computer's Orchbee is too old for the remote.
        LocalNeedsUpdate = "local-needs-update",
        /// The remote Orchbee belongs to another environment.
        Incompatible = "incompatible",
        /// A state this build does not know.
        Unknown = "unknown",
    }
);

name_enum!(
    /// Why a Machine is `offline`; it grows and an unknown cause reads as absent.
    pub enum OfflineCause {
        /// The host is unreachable.
        HostUnreachable = "host-unreachable",
        /// The host did not answer in time.
        HostTimeout = "host-timeout",
        /// The host refused the connection.
        HostRefused = "host-refused",
        /// The host name did not resolve.
        HostUnresolved = "host-unresolved",
        /// No usable `ssh` client.
        SshUnavailable = "ssh-unavailable",
        /// `ssh` failed for a reason no diagnostic matched.
        SshFailed = "ssh-failed",
        /// The remote end broke the link protocol.
        RemoteProtocol = "remote-protocol",
        /// The remote Session did not report ready.
        RemoteNotReady = "remote-not-ready",
    }
);

name_enum!(
    /// Why a Machine is `reconnecting`: an `offline` cause or `link-lost`; it grows and an unknown
    /// cause reads as absent.
    pub enum ReconnectCause {
        /// The host is unreachable.
        HostUnreachable = "host-unreachable",
        /// The host did not answer in time.
        HostTimeout = "host-timeout",
        /// The host refused the connection.
        HostRefused = "host-refused",
        /// The host name did not resolve.
        HostUnresolved = "host-unresolved",
        /// No usable `ssh` client.
        SshUnavailable = "ssh-unavailable",
        /// `ssh` failed for a reason no diagnostic matched.
        SshFailed = "ssh-failed",
        /// The remote end broke the link protocol.
        RemoteProtocol = "remote-protocol",
        /// The remote Session did not report ready.
        RemoteNotReady = "remote-not-ready",
        /// The link was lost.
        LinkLost = "link-lost",
    }
);

name_enum!(
    /// Why a Machine `needs-login`; it grows and an unknown cause reads as absent.
    pub enum NeedsLoginCause {
        /// `ssh` asked a question.
        AuthRequired = "auth-required",
        /// A remembered answer was refused.
        RememberedRejected = "remembered-rejected",
        /// A command episode was dismissed and the Machine is muted.
        LoginDismissed = "login-dismissed",
        /// A `disconnect` ended the login.
        Disconnected = "disconnected",
    }
);

name_enum!(
    /// Why a Machine's login failed; it grows and an unknown cause reads as absent.
    pub enum LoginFailedCause {
        /// `ssh` refused the answers.
        PasswordRejected = "password-rejected",
        /// The server refused the key.
        KeyRejected = "key-rejected",
    }
);

name_enum!(
    /// Why a Machine is `unsupported`; it grows and an unknown cause reads as absent.
    pub enum UnsupportedCause {
        /// The remote system is not supported.
        RemotePlatform = "remote-platform",
        /// The record's connection kind is not supported.
        ConnectionKind = "connection-kind",
    }
);

name_enum!(
    /// Why a Machine `needs-update`; it grows and an unknown cause reads as absent.
    pub enum NeedsUpdateCause {
        /// The remote Orchbee is too old.
        RemoteTooOld = "remote-too-old",
        /// The remote Session is too old.
        SessionTooOld = "session-too-old",
    }
);

name_enum!(
    /// The step of a running attempt; it grows and an unknown stage reads as absent.
    pub enum Stage {
        /// Reaching the host.
        Probing = "probing",
        /// Logging in.
        Authenticating = "authenticating",
        /// Starting the remote helper or Session.
        Starting = "starting",
        /// Copying files.
        Copying = "copying",
    }
);

/// A Machine's state with the members the state admits.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MachineState {
    /// The record is disabled.
    Disabled,
    /// A first attempt is running.
    Connecting {
        /// The attempt's step.
        stage: Option<Stage>,
    },
    /// The link is up and the Session answered ready.
    Online,
    /// The link was lost or an attempt failed, and the hub retries by itself.
    Reconnecting {
        /// Why.
        cause: Option<ReconnectCause>,
        /// The running attempt's step.
        stage: Option<Stage>,
        /// When the next automatic attempt starts.
        next_attempt: Option<Time>,
    },
    /// No link and no attempt in progress.
    Offline {
        /// Why.
        cause: Option<OfflineCause>,
        /// When the next automatic attempt starts.
        next_attempt: Option<Time>,
    },
    /// The link exists and a `ping` went unanswered for 15 seconds.
    Unresponsive,
    /// A person must answer a question; not an error.
    NeedsLogin {
        /// Why.
        cause: Option<NeedsLoginCause>,
    },
    /// `ssh` gave up after refused answers or a refused key.
    LoginFailed {
        /// Why.
        cause: Option<LoginFailedCause>,
    },
    /// The host key is not in `known_hosts`.
    Untrusted {
        /// The offered key.
        host_key: HostKey,
    },
    /// The host key differs from `known_hosts`; no question is offered.
    KeyChanged {
        /// The offered key, when known.
        host_key: Option<HostKey>,
    },
    /// The remote has no Orchbee of this environment.
    NotInstalled,
    /// The remote system, or the record's connection kind, is not supported.
    Unsupported {
        /// Why.
        cause: Option<UnsupportedCause>,
    },
    /// The remote Orchbee or its Session is too old to work with this one.
    NeedsUpdate {
        /// Why.
        cause: Option<NeedsUpdateCause>,
    },
    /// This computer's Orchbee is too old for the remote.
    LocalNeedsUpdate,
    /// The remote Orchbee belongs to another environment.
    Incompatible,
    /// A state this build does not know.
    Unknown,
}

impl MachineState {
    /// The state's name.
    #[must_use]
    pub const fn name(&self) -> MachineStateName {
        match self {
            Self::Disabled => MachineStateName::Disabled,
            Self::Connecting { .. } => MachineStateName::Connecting,
            Self::Online => MachineStateName::Online,
            Self::Reconnecting { .. } => MachineStateName::Reconnecting,
            Self::Offline { .. } => MachineStateName::Offline,
            Self::Unresponsive => MachineStateName::Unresponsive,
            Self::NeedsLogin { .. } => MachineStateName::NeedsLogin,
            Self::LoginFailed { .. } => MachineStateName::LoginFailed,
            Self::Untrusted { .. } => MachineStateName::Untrusted,
            Self::KeyChanged { .. } => MachineStateName::KeyChanged,
            Self::NotInstalled => MachineStateName::NotInstalled,
            Self::Unsupported { .. } => MachineStateName::Unsupported,
            Self::NeedsUpdate { .. } => MachineStateName::NeedsUpdate,
            Self::LocalNeedsUpdate => MachineStateName::LocalNeedsUpdate,
            Self::Incompatible => MachineStateName::Incompatible,
            Self::Unknown => MachineStateName::Unknown,
        }
    }
}

/// What the hub knows of the remote Orchbee: at least one member.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Remote {
    product_version: Option<ProductVersion>,
    platform: Option<Token>,
    architecture: Option<Token>,
}

impl Remote {
    /// The remote facts, at least one of them known.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] when all three are absent.
    pub fn new(
        product_version: Option<ProductVersion>,
        platform: Option<Token>,
        architecture: Option<Token>,
    ) -> Result<Self, InvalidValue> {
        if product_version.is_none() && platform.is_none() && architecture.is_none() {
            return Err(InvalidValue);
        }
        Ok(Self {
            product_version,
            platform,
            architecture,
        })
    }

    /// The remote product version.
    #[must_use]
    pub const fn product_version(&self) -> Option<&ProductVersion> {
        self.product_version.as_ref()
    }

    /// The remote platform token.
    #[must_use]
    pub const fn platform(&self) -> Option<&Token> {
        self.platform.as_ref()
    }

    /// The remote architecture token.
    #[must_use]
    pub const fn architecture(&self) -> Option<&Token> {
        self.architecture.as_ref()
    }
}

/// Everything known about one Machine; each event replaces the last.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MachineEvent {
    /// The Machine.
    pub machine: MachineId,
    /// Its state.
    pub state: MachineState,
    /// When the last connection attempt ended.
    pub last_attempt: Option<Time>,
    /// The remote facts, when known.
    pub remote: Option<Remote>,
    /// Whether this is a draft Machine, sent to its owner only.
    pub draft: bool,
}

name_enum!(
    /// Why a `connect` waits for a person; it grows and an unknown reason reads as `login`.
    pub enum WaitReason {
        /// A login episode.
        Login = "login",
    }
);

/// A `connect` request started waiting for a person.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConnectWaiting {
    /// The waiting request's id.
    pub request: Id,
    /// Why it waits.
    pub reason: WaitReason,
}

name_enum!(
    /// What started a login episode; it grows with fallback `command`.
    pub enum Trigger {
        /// A person's `connect`.
        User = "user",
        /// A command's `connect`.
        Command = "command",
    }
);

name_enum!(
    /// Which client holds a claimed episode, from the receiver's view.
    pub enum Claimant {
        /// The receiver.
        You = "you",
        /// Another client.
        Other = "other",
    }
);

name_enum!(
    /// How an episode ended; it grows with fallback `unknown`.
    pub enum LoginEnd {
        /// The link authenticated.
        Succeeded = "succeeded",
        /// `ssh` gave up on the answers.
        Failed = "failed",
        /// A client dismissed it.
        Dismissed = "dismissed",
        /// Nobody remained to answer.
        Abandoned = "abandoned",
        /// The attempt ended for a reason the Machine's state names.
        Unavailable = "unavailable",
        /// An end this build does not know.
        Unknown = "unknown",
    }
);

/// An episode's phase with the members it admits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LoginPhase {
    /// Offered to every client that asks.
    Offered,
    /// Claimed by one client.
    Claimed {
        /// Which one.
        claim: Claimant,
    },
    /// Ended.
    Ended {
        /// How.
        end: LoginEnd,
    },
}

/// Everything known about one login episode; each event replaces the last.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LoginEvent {
    /// The episode, unique in the hub's process life and ascending in creation order.
    pub login: Id,
    /// The Machine.
    pub machine: MachineId,
    /// What started it.
    pub trigger: Trigger,
    /// Its phase.
    pub phase: LoginPhase,
    /// The `command` requests waiting on it.
    pub waiters: Count,
}

/// The hub's reading of a question's prompt.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum QuestionKind {
    /// A password.
    Password,
    /// A key passphrase.
    KeyPassphrase,
    /// Whether to trust a host key, answered with `yes` or dismissed.
    HostKey {
        /// The offered key.
        host_key: HostKey,
    },
    /// A one-time code or anything unrecognized: shown verbatim, entered masked, never remembered.
    Other,
}

/// Where the claimant delivers the answer: a one-shot endpoint and its token.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AnswerEndpoint {
    /// The endpoint's path.
    pub endpoint: PathText,
    /// The token that admits one delivery.
    pub token: AnswerToken,
}

/// A question to the claimant of an episode. No member holds an answer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Question {
    /// The episode.
    pub login: Id,
    /// The question, ascending within the episode.
    pub question: Id,
    /// The hub's reading of the prompt.
    pub kind: QuestionKind,
    /// The question as `ssh` worded it.
    pub prompt: Prompt,
    /// `ssh` refused the previous answer of this kind in this episode.
    pub repeated: bool,
    /// A remembered answer was tried for this question and refused.
    pub remembered_rejected: bool,
    /// Where the answer goes.
    pub answer: AnswerEndpoint,
}

/// An event the hub sends to a client, other than `bye`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Event {
    /// The hub's lifetime state.
    Hub(HubState),
    /// The catalog.
    Catalog(Catalog),
    /// One Machine.
    Machine(MachineEvent),
    /// A `connect` started waiting for a person.
    ConnectWaiting(ConnectWaiting),
    /// One login episode.
    Login(LoginEvent),
    /// A question to the claimant.
    Question(Question),
}

const SESSION_REASONS: [ResetReason; 10] = [
    SHARED_RESET_REASONS[0],
    SHARED_RESET_REASONS[1],
    SHARED_RESET_REASONS[2],
    SHARED_RESET_REASONS[3],
    SHARED_RESET_REASONS[4],
    ResetReason::NotFound,
    ResetReason::NotOnline,
    ResetReason::LinkLost,
    ResetReason::SessionNotRunning,
    ResetReason::InvalidSession,
];
const ROUTE_REASONS: [ResetReason; 9] = [
    SHARED_RESET_REASONS[0],
    SHARED_RESET_REASONS[1],
    SHARED_RESET_REASONS[2],
    SHARED_RESET_REASONS[3],
    SHARED_RESET_REASONS[4],
    ResetReason::NotFound,
    ResetReason::NotOnline,
    ResetReason::LinkLost,
    ResetReason::TooLarge,
];

const ATTACH: &str = "attach";
const RELOAD: &str = "reload";
const CONNECT: &str = "connect";
const DISCONNECT: &str = "disconnect";
const CLAIM: &str = "claim";
const DISMISS: &str = "dismiss";
const STOP_SESSION: &str = "stop-session";

const HUB: &str = "hub";
const CATALOG: &str = "catalog";
const MACHINE: &str = "machine";
const CONNECT_WAITING: &str = "connect-waiting";
const LOGIN: &str = "login";
const QUESTION: &str = "question";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireMachineMember {
    machine: MachineId,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireLoginMember {
    login: Id,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireEmpty {}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireAttach {
    presence: bool,
    asks: bool,
    watch: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireConnection {
    kind: Name,
    target: Target,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireDraft {
    connection: WireConnection,
    session: SessionName,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireConnect {
    machine: MachineId,
    origin: Name,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    answers: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    draft: Option<WireDraft>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireHubState {
    state: Name,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<Time>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireOutcome {
    outcome: Name,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireAttachResult {
    outcome: Name,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hub: Option<WireHubState>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireReloadResult {
    outcome: Name,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revision: Option<Count>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireConnectResult {
    outcome: Name,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state: Option<Name>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote_version: Option<ProductVersion>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireCatalog {
    revision: Count,
    machines: Vec<MachineId>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireRemote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    product_version: Option<ProductVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    platform: Option<Token>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    architecture: Option<Token>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireMachine {
    machine: Option<MachineId>,
    state: Option<Name>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cause: Option<Name>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stage: Option<Name>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_attempt: Option<Time>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next_attempt: Option<Time>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote: Option<WireRemote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host_key: Option<HostKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    draft: Option<True>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireConnectWaiting {
    request: Id,
    reason: Name,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireLogin {
    login: Id,
    machine: MachineId,
    trigger: Name,
    phase: Name,
    waiters: Count,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claim: Option<Name>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end: Option<Name>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireAnswer {
    endpoint: PathText,
    token: AnswerToken,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireQuestion {
    login: Id,
    question: Id,
    kind: Name,
    prompt: Prompt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host_key: Option<HostKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repeated: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remembered: Option<Name>,
    answer: WireAnswer,
}

const SSH: &str = "ssh";
const USER: &str = "user";
const COMMAND: &str = "command";
const REJECTED: &str = "rejected";

/// A member that exactly one known value of a growable name admits: required with `required`,
/// refused with another known value, and admitted then dropped with an unknown one.
fn member_of<Value>(
    known: bool,
    admitted: bool,
    required: bool,
    member: Option<Value>,
) -> Result<Option<Value>, Refused> {
    match (known, admitted, member) {
        (false, _, _) => Ok(None),
        (true, true, None) if required => Err(Refused),
        (true, true, member) => Ok(member),
        (true, false, Some(_)) => Err(Refused),
        (true, false, None) => Ok(None),
    }
}

impl HubState {
    fn decode(wire: WireHubState) -> Result<Self, Refused> {
        match (wire.state.as_str(), wire.expires_at) {
            ("held", None) => Ok(Self::Held),
            ("expiring", Some(expires_at)) => Ok(Self::Expiring { expires_at }),
            ("closing", None) => Ok(Self::Closing),
            _ => Err(Refused),
        }
    }

    fn encode(self) -> WireHubState {
        let (state, expires_at) = match self {
            Self::Held => ("held", None),
            Self::Expiring { expires_at } => ("expiring", Some(expires_at)),
            Self::Closing => ("closing", None),
        };
        WireHubState {
            state: Name::literal(state),
            expires_at,
        }
    }
}

fn decode_connect(wire: WireConnect) -> Result<Connect, Refused> {
    let origin = match (wire.origin.as_str(), wire.answers) {
        (USER, Some(true)) => Origin::User,
        (COMMAND, answers) => Origin::Command {
            answers: answers.unwrap_or(false),
        },
        _ => return Err(Refused),
    };
    let draft = match wire.draft {
        Some(draft) if draft.connection.kind.as_str() == SSH => Some(Draft {
            target: draft.connection.target,
            session: draft.session,
        }),
        Some(_) => return Err(Refused),
        None => None,
    };
    Ok(Connect {
        machine: wire.machine,
        origin,
        draft,
    })
}

fn encode_connect(connect: &Connect) -> WireConnect {
    let (origin, answers) = match connect.origin {
        Origin::User => (USER, Some(true)),
        Origin::Command { answers } => (COMMAND, answers.then_some(true)),
    };
    WireConnect {
        machine: connect.machine.clone(),
        origin: Name::literal(origin),
        answers,
        draft: connect.draft.as_ref().map(|draft| WireDraft {
            connection: WireConnection {
                kind: Name::literal(SSH),
                target: draft.target.clone(),
            },
            session: draft.session.clone(),
        }),
    }
}

fn decode_attach_result(wire: WireAttachResult) -> Result<AttachOutcome, Refused> {
    let name = wire.outcome.as_str();
    let known = matches!(name, "attached" | "stopping");
    let hub = member_of(known, name == "attached", true, wire.hub)?;
    Ok(match (name, hub) {
        ("attached", Some(hub)) => AttachOutcome::Attached {
            hub: HubState::decode(hub)?,
        },
        ("stopping", _) => AttachOutcome::Stopping,
        _ => AttachOutcome::Unknown,
    })
}

fn decode_reload_result(wire: WireReloadResult) -> Result<ReloadOutcome, Refused> {
    let name = wire.outcome.as_str();
    let known = matches!(
        name,
        "reloaded" | "invalid-catalog" | "storage" | "stopping"
    );
    let revision = member_of(known, name == "reloaded", true, wire.revision)?;
    Ok(match (name, revision) {
        ("reloaded", Some(revision)) => ReloadOutcome::Reloaded { revision },
        ("invalid-catalog", _) => ReloadOutcome::InvalidCatalog,
        ("storage", _) => ReloadOutcome::Storage,
        ("stopping", _) => ReloadOutcome::Stopping,
        _ => ReloadOutcome::Unknown,
    })
}

fn decode_connect_result(wire: WireConnectResult) -> Result<ConnectResult, Refused> {
    let name = wire.outcome.as_str();
    let failure = ConnectFailure::from_name(name);
    let known = name == "online" || failure.is_some();
    let remote_version = member_of(known, name == "online", true, wire.remote_version)?;
    let outcome = match remote_version {
        Some(remote_version) => ConnectOutcome::Online { remote_version },
        None => ConnectOutcome::NotOnline(failure.unwrap_or(ConnectFailure::Unknown)),
    };
    Ok(ConnectResult {
        outcome,
        state: wire.state.map(|state| {
            MachineStateName::from_name(state.as_str()).unwrap_or(MachineStateName::Unknown)
        }),
    })
}

fn encode_connect_result(result: &ConnectResult) -> WireConnectResult {
    let (outcome, remote_version) = match &result.outcome {
        ConnectOutcome::Online { remote_version } => ("online", Some(remote_version.clone())),
        ConnectOutcome::NotOnline(failure) => (failure.as_str(), None),
    };
    WireConnectResult {
        outcome: Name::literal(outcome),
        state: result.state.map(|state| Name::literal(state.as_str())),
        remote_version,
    }
}

/// The cause of a state that lists causes: a name the state does not list reads as absent.
fn cause<Cause>(name: Option<Name>, read: fn(&str) -> Option<Cause>) -> Option<Cause> {
    name.and_then(|name| read(name.as_str()))
}

fn stage(name: Option<Name>) -> Option<Stage> {
    name.and_then(|name| Stage::from_name(name.as_str()))
}

fn decode_machine(wire: WireMachine) -> Result<MachineEvent, Refused> {
    let (Some(machine), Some(state)) = (wire.machine, wire.state) else {
        return Err(Refused);
    };
    let name = MachineStateName::from_name(state.as_str()).unwrap_or(MachineStateName::Unknown);
    let admits_cause = matches!(
        name,
        MachineStateName::Reconnecting
            | MachineStateName::Offline
            | MachineStateName::NeedsLogin
            | MachineStateName::LoginFailed
            | MachineStateName::Unsupported
            | MachineStateName::NeedsUpdate
    );
    let admits_stage = matches!(
        name,
        MachineStateName::Connecting | MachineStateName::Reconnecting
    );
    let admits_next = matches!(
        name,
        MachineStateName::Reconnecting | MachineStateName::Offline
    );
    let admits_key = matches!(
        name,
        MachineStateName::Untrusted | MachineStateName::KeyChanged
    );
    let known = name != MachineStateName::Unknown;
    let cause_name = member_of(known, admits_cause, false, wire.cause)?;
    let stage_name = member_of(known, admits_stage, false, wire.stage)?;
    let next_attempt = member_of(known, admits_next, false, wire.next_attempt)?;
    let host_key = member_of(
        known,
        admits_key,
        name == MachineStateName::Untrusted,
        wire.host_key,
    )?;
    let state = match name {
        MachineStateName::Disabled => MachineState::Disabled,
        MachineStateName::Connecting => MachineState::Connecting {
            stage: stage(stage_name),
        },
        MachineStateName::Online => MachineState::Online,
        MachineStateName::Reconnecting => MachineState::Reconnecting {
            cause: cause(cause_name, ReconnectCause::from_name),
            stage: stage(stage_name),
            next_attempt,
        },
        MachineStateName::Offline => MachineState::Offline {
            cause: cause(cause_name, OfflineCause::from_name),
            next_attempt,
        },
        MachineStateName::Unresponsive => MachineState::Unresponsive,
        MachineStateName::NeedsLogin => MachineState::NeedsLogin {
            cause: cause(cause_name, NeedsLoginCause::from_name),
        },
        MachineStateName::LoginFailed => MachineState::LoginFailed {
            cause: cause(cause_name, LoginFailedCause::from_name),
        },
        MachineStateName::Untrusted => MachineState::Untrusted {
            host_key: host_key.ok_or(Refused)?,
        },
        MachineStateName::KeyChanged => MachineState::KeyChanged { host_key },
        MachineStateName::NotInstalled => MachineState::NotInstalled,
        MachineStateName::Unsupported => MachineState::Unsupported {
            cause: cause(cause_name, UnsupportedCause::from_name),
        },
        MachineStateName::NeedsUpdate => MachineState::NeedsUpdate {
            cause: cause(cause_name, NeedsUpdateCause::from_name),
        },
        MachineStateName::LocalNeedsUpdate => MachineState::LocalNeedsUpdate,
        MachineStateName::Incompatible => MachineState::Incompatible,
        MachineStateName::Unknown => MachineState::Unknown,
    };
    let remote = wire
        .remote
        .map(|remote| Remote::new(remote.product_version, remote.platform, remote.architecture))
        .transpose()
        .map_err(|_| Refused)?;
    Ok(MachineEvent {
        machine,
        state,
        last_attempt: wire.last_attempt,
        remote,
        draft: wire.draft.is_some(),
    })
}

fn encode_machine(event: &MachineEvent) -> WireMachine {
    fn name(text: &'static str) -> Option<Name> {
        Some(Name::literal(text))
    }
    let mut wire = WireMachine {
        machine: Some(event.machine.clone()),
        state: name(event.state.name().as_str()),
        last_attempt: event.last_attempt,
        remote: event.remote.as_ref().map(|remote| WireRemote {
            product_version: remote.product_version.clone(),
            platform: remote.platform.clone(),
            architecture: remote.architecture.clone(),
        }),
        draft: True::when(event.draft),
        ..WireMachine::default()
    };
    match &event.state {
        MachineState::Connecting { stage } => wire.stage = stage.and_then(|s| name(s.as_str())),
        MachineState::Reconnecting {
            cause,
            stage,
            next_attempt,
        } => {
            wire.cause = cause.and_then(|c| name(c.as_str()));
            wire.stage = stage.and_then(|s| name(s.as_str()));
            wire.next_attempt = *next_attempt;
        }
        MachineState::Offline {
            cause,
            next_attempt,
        } => {
            wire.cause = cause.and_then(|c| name(c.as_str()));
            wire.next_attempt = *next_attempt;
        }
        MachineState::NeedsLogin { cause } => wire.cause = cause.and_then(|c| name(c.as_str())),
        MachineState::LoginFailed { cause } => wire.cause = cause.and_then(|c| name(c.as_str())),
        MachineState::Untrusted { host_key } => wire.host_key = Some(host_key.clone()),
        MachineState::KeyChanged { host_key } => wire.host_key.clone_from(host_key),
        MachineState::Unsupported { cause } => wire.cause = cause.and_then(|c| name(c.as_str())),
        MachineState::NeedsUpdate { cause } => wire.cause = cause.and_then(|c| name(c.as_str())),
        MachineState::Disabled
        | MachineState::Online
        | MachineState::Unresponsive
        | MachineState::NotInstalled
        | MachineState::LocalNeedsUpdate
        | MachineState::Incompatible
        | MachineState::Unknown => {}
    }
    wire
}

fn decode_login(wire: WireLogin) -> Result<LoginEvent, Refused> {
    let phase = match (wire.phase.as_str(), wire.claim, wire.end) {
        ("offered", None, None) => LoginPhase::Offered,
        ("claimed", Some(claim), None) => LoginPhase::Claimed {
            claim: Claimant::from_name(claim.as_str()).ok_or(Refused)?,
        },
        ("ended", None, Some(end)) => LoginPhase::Ended {
            end: LoginEnd::from_name(end.as_str()).unwrap_or(LoginEnd::Unknown),
        },
        _ => return Err(Refused),
    };
    Ok(LoginEvent {
        login: wire.login,
        machine: wire.machine,
        trigger: Trigger::from_name(wire.trigger.as_str()).unwrap_or(Trigger::Command),
        phase,
        waiters: wire.waiters,
    })
}

fn encode_login(event: &LoginEvent) -> WireLogin {
    let (phase, claim, end) = match event.phase {
        LoginPhase::Offered => ("offered", None, None),
        LoginPhase::Claimed { claim } => ("claimed", Some(Name::literal(claim.as_str())), None),
        LoginPhase::Ended { end } => ("ended", None, Some(Name::literal(end.as_str()))),
    };
    WireLogin {
        login: event.login,
        machine: event.machine.clone(),
        trigger: Name::literal(event.trigger.as_str()),
        phase: Name::literal(phase),
        waiters: event.waiters,
        claim,
        end,
    }
}

fn decode_question(wire: WireQuestion) -> Result<Question, Refused> {
    let name = wire.kind.as_str();
    let known = matches!(name, "password" | "key-passphrase" | "host-key" | "other");
    let host_key = member_of(known, name == "host-key", true, wire.host_key)?;
    let kind = match (name, host_key) {
        ("password", _) => QuestionKind::Password,
        ("key-passphrase", _) => QuestionKind::KeyPassphrase,
        ("host-key", Some(host_key)) => QuestionKind::HostKey { host_key },
        _ => QuestionKind::Other,
    };
    let remembered_rejected = match wire.remembered {
        Some(remembered) if remembered.as_str() == REJECTED => true,
        Some(_) => return Err(Refused),
        None => false,
    };
    Ok(Question {
        login: wire.login,
        question: wire.question,
        kind,
        prompt: wire.prompt,
        repeated: wire.repeated.is_some(),
        remembered_rejected,
        answer: AnswerEndpoint {
            endpoint: wire.answer.endpoint,
            token: wire.answer.token,
        },
    })
}

fn encode_question(question: &Question) -> WireQuestion {
    let (kind, host_key) = match &question.kind {
        QuestionKind::Password => ("password", None),
        QuestionKind::KeyPassphrase => ("key-passphrase", None),
        QuestionKind::HostKey { host_key } => ("host-key", Some(host_key.clone())),
        QuestionKind::Other => ("other", None),
    };
    WireQuestion {
        login: question.login,
        question: question.question,
        kind: Name::literal(kind),
        prompt: question.prompt.clone(),
        host_key,
        repeated: True::when(question.repeated),
        remembered: question
            .remembered_rejected
            .then(|| Name::literal(REJECTED)),
        answer: WireAnswer {
            endpoint: question.answer.endpoint.clone(),
            token: question.answer.token,
        },
    }
}

impl Codec for Hub {
    const FACTS_FIRST: bool = false;
    const RESET_REASONS: &'static [ResetReason] = &ROUTE_REASONS;

    type Channel = Channel;
    type ChannelKind = ChannelKind;
    type Outcome = Outcome;
    type Request = Request;
    type Reply = Reply;
    type Event = Event;

    fn decode_open(kind: &str, members: Members) -> Result<Option<Channel>, Refused> {
        Ok(Some(match ChannelKind::from_name(kind) {
            None => return Ok(None),
            Some(ChannelKind::Session) => Channel::Session {
                machine: closed::<WireMachineMember>(members)?.machine,
            },
            Some(ChannelKind::Route) => Channel::Route {
                machine: closed::<WireMachineMember>(members)?.machine,
            },
        }))
    }

    fn encode_open(channel: &Channel) -> (&'static str, Members) {
        (
            channel.kind().as_str(),
            members_of(&WireMachineMember {
                machine: channel.machine().clone(),
            }),
        )
    }

    fn channel_kind(channel: &Channel) -> ChannelKind {
        channel.kind()
    }

    fn rules(kind: ChannelKind) -> KindRules {
        match kind {
            ChannelKind::Session => KindRules::STREAM,
            ChannelKind::Route => KindRules::ROUTE,
        }
    }

    fn reset_reasons(kind: ChannelKind) -> &'static [ResetReason] {
        match kind {
            ChannelKind::Session => &SESSION_REASONS,
            ChannelKind::Route => &ROUTE_REASONS,
        }
    }

    fn decode_outcome(members: Members) -> Result<Outcome, Refused> {
        let name = closed::<WireOutcome>(members)?.outcome;
        Ok(Outcome::from_name(name.as_str()).unwrap_or(Outcome::Unknown))
    }

    fn encode_outcome(outcome: &Outcome) -> Members {
        members_of(&WireOutcome {
            outcome: Name::literal(outcome.as_str()),
        })
    }

    fn outcome_for(outcome: Outcome, kind: ChannelKind) -> Outcome {
        if Self::kind_lists(&outcome, kind) {
            outcome
        } else {
            Outcome::Unknown
        }
    }

    fn kind_lists(outcome: &Outcome, kind: ChannelKind) -> bool {
        matches!(
            (kind, outcome),
            (
                ChannelKind::Session,
                Outcome::Ended | Outcome::Failed | Outcome::Lost
            ) | (ChannelKind::Route, Outcome::Replied | Outcome::Lost)
        )
    }

    fn decode_request(name: &str, members: Members) -> Result<Option<Request>, Refused> {
        Ok(Some(match name {
            ATTACH => {
                let wire = closed::<WireAttach>(members)?;
                Request::Attach(Attach {
                    presence: wire.presence,
                    asks: wire.asks,
                    watch: wire.watch,
                })
            }
            RELOAD => {
                closed::<WireEmpty>(members)?;
                Request::Reload
            }
            CONNECT => Request::Connect(decode_connect(closed(members)?)?),
            DISCONNECT => Request::Disconnect {
                machine: closed::<WireMachineMember>(members)?.machine,
            },
            CLAIM => Request::Claim {
                login: closed::<WireLoginMember>(members)?.login,
            },
            DISMISS => Request::Dismiss {
                login: closed::<WireLoginMember>(members)?.login,
            },
            STOP_SESSION => Request::StopSession {
                machine: closed::<WireMachineMember>(members)?.machine,
            },
            _ => return Ok(None),
        }))
    }

    fn request_name(request: &Request) -> &'static str {
        match request {
            Request::Attach(_) => ATTACH,
            Request::Reload => RELOAD,
            Request::Connect(_) => CONNECT,
            Request::Disconnect { .. } => DISCONNECT,
            Request::Claim { .. } => CLAIM,
            Request::Dismiss { .. } => DISMISS,
            Request::StopSession { .. } => STOP_SESSION,
        }
    }

    fn encode_request(request: &Request) -> Members {
        match request {
            Request::Attach(attach) => members_of(&WireAttach {
                presence: attach.presence,
                asks: attach.asks,
                watch: attach.watch,
            }),
            Request::Reload => members_of(&WireEmpty {}),
            Request::Connect(connect) => members_of(&encode_connect(connect)),
            Request::Disconnect { machine } | Request::StopSession { machine } => {
                members_of(&WireMachineMember {
                    machine: machine.clone(),
                })
            }
            Request::Claim { login } | Request::Dismiss { login } => {
                members_of(&WireLoginMember { login: *login })
            }
        }
    }

    fn decode_reply(request: &str, members: Members) -> Result<Option<Reply>, Refused> {
        let simple =
            |members| -> Result<Name, Refused> { Ok(closed::<WireOutcome>(members)?.outcome) };
        Ok(Some(match request {
            ATTACH => Reply::Attach(decode_attach_result(closed(members)?)?),
            RELOAD => Reply::Reload(decode_reload_result(closed(members)?)?),
            CONNECT => Reply::Connect(decode_connect_result(closed(members)?)?),
            DISCONNECT => Reply::Disconnect(
                DisconnectOutcome::from_name(simple(members)?.as_str())
                    .unwrap_or(DisconnectOutcome::Unknown),
            ),
            CLAIM => Reply::Claim(
                ClaimOutcome::from_name(simple(members)?.as_str()).unwrap_or(ClaimOutcome::Unknown),
            ),
            DISMISS => Reply::Dismiss(
                DismissOutcome::from_name(simple(members)?.as_str())
                    .unwrap_or(DismissOutcome::Unknown),
            ),
            STOP_SESSION => Reply::StopSession(
                StopSessionOutcome::from_name(simple(members)?.as_str())
                    .unwrap_or(StopSessionOutcome::Unknown),
            ),
            _ => return Ok(None),
        }))
    }

    fn reply_request(reply: &Reply) -> &'static str {
        match reply {
            Reply::Attach(_) => ATTACH,
            Reply::Reload(_) => RELOAD,
            Reply::Connect(_) => CONNECT,
            Reply::Disconnect(_) => DISCONNECT,
            Reply::Claim(_) => CLAIM,
            Reply::Dismiss(_) => DISMISS,
            Reply::StopSession(_) => STOP_SESSION,
        }
    }

    fn encode_reply(reply: &Reply) -> Members {
        let simple = |name: &'static str| {
            members_of(&WireOutcome {
                outcome: Name::literal(name),
            })
        };
        match reply {
            Reply::Attach(outcome) => {
                let (name, hub) = match outcome {
                    AttachOutcome::Attached { hub } => ("attached", Some(hub.encode())),
                    AttachOutcome::Stopping => ("stopping", None),
                    AttachOutcome::Unknown => ("unknown", None),
                };
                members_of(&WireAttachResult {
                    outcome: Name::literal(name),
                    hub,
                })
            }
            Reply::Reload(outcome) => {
                let (name, revision) = match outcome {
                    ReloadOutcome::Reloaded { revision } => ("reloaded", Some(*revision)),
                    ReloadOutcome::InvalidCatalog => ("invalid-catalog", None),
                    ReloadOutcome::Storage => ("storage", None),
                    ReloadOutcome::Stopping => ("stopping", None),
                    ReloadOutcome::Unknown => ("unknown", None),
                };
                members_of(&WireReloadResult {
                    outcome: Name::literal(name),
                    revision,
                })
            }
            Reply::Connect(result) => members_of(&encode_connect_result(result)),
            Reply::Disconnect(outcome) => simple(outcome.as_str()),
            Reply::Claim(outcome) => simple(outcome.as_str()),
            Reply::Dismiss(outcome) => simple(outcome.as_str()),
            Reply::StopSession(outcome) => simple(outcome.as_str()),
        }
    }

    fn decode_event(name: &str, members: Members) -> Result<Option<Event>, Refused> {
        Ok(Some(match name {
            HUB => Event::Hub(HubState::decode(closed(members)?)?),
            CATALOG => {
                let wire = closed::<WireCatalog>(members)?;
                Event::Catalog(Catalog::new(wire.revision, wire.machines).map_err(|_| Refused)?)
            }
            MACHINE => Event::Machine(decode_machine(closed(members)?)?),
            CONNECT_WAITING => {
                let wire = closed::<WireConnectWaiting>(members)?;
                Event::ConnectWaiting(ConnectWaiting {
                    request: wire.request,
                    reason: WaitReason::from_name(wire.reason.as_str())
                        .unwrap_or(WaitReason::Login),
                })
            }
            LOGIN => Event::Login(decode_login(closed(members)?)?),
            QUESTION => Event::Question(decode_question(closed(members)?)?),
            _ => return Ok(None),
        }))
    }

    fn event_name(event: &Event) -> &'static str {
        match event {
            Event::Hub(_) => HUB,
            Event::Catalog(_) => CATALOG,
            Event::Machine(_) => MACHINE,
            Event::ConnectWaiting(_) => CONNECT_WAITING,
            Event::Login(_) => LOGIN,
            Event::Question(_) => QUESTION,
        }
    }

    fn encode_event(event: &Event) -> Members {
        match event {
            Event::Hub(state) => members_of(&state.encode()),
            Event::Catalog(catalog) => members_of(&WireCatalog {
                revision: catalog.revision,
                machines: catalog.machines.clone(),
            }),
            Event::Machine(machine) => members_of(&encode_machine(machine)),
            Event::ConnectWaiting(waiting) => members_of(&WireConnectWaiting {
                request: waiting.request,
                reason: Name::literal(waiting.reason.as_str()),
            }),
            Event::Login(login) => members_of(&encode_login(login)),
            Event::Question(question) => members_of(&encode_question(question)),
        }
    }

    fn event_subject(event: &Event) -> Option<String> {
        match event {
            Event::Hub(_) => Some(HUB.to_owned()),
            Event::Catalog(_) => Some(CATALOG.to_owned()),
            Event::Machine(machine) => Some(format!("{MACHINE}:{}", machine.machine)),
            Event::Login(login) => Some(format!("{LOGIN}:{}", login.login.get())),
            Event::ConnectWaiting(_) | Event::Question(_) => None,
        }
    }

    fn is_facts(_event: &Event) -> bool {
        false
    }
}
