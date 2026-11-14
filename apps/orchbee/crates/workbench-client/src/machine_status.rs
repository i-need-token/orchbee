//! What every interface says about a remote Machine beyond its one word: the detail lines that
//! say why it is in its state, and the one action that helps, from the hub's state and the facts
//! it reported with it. Drawing-free: an interface lays the lines out and offers the action in its
//! own way, and `localization` words both. `apps/orchbee/docs/machines.md#states-and-details`
//! owns the table.
use crate::machine_discovery::{HubStatus, LoginState, Observation};
use orchbee_machine_protocol::hub::{
    MachineState, NeedsUpdateCause, OfflineCause, ReconnectCause, Remote, Stage, UnsupportedCause,
};

/// The facts the hub last reported with a Machine's state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Facts {
    /// The state with its members.
    pub state: Option<MachineState>,
    /// When the last connection attempt ended, in milliseconds since the Unix epoch.
    pub last_attempt: Option<u64>,
    /// The remote's version, system and processor, as far as known.
    pub remote: Option<Remote>,
}

/// The one thing a person can do about a Machine's state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Use the Machine.
    Open,
    /// Try to connect again.
    Retry,
    /// Start the automatic attempt that waits now.
    RetryNow,
    /// End the link that does not answer and connect again.
    Reconnect,
    /// Enable the record.
    Enable,
    LogIn,
    LogInAgain,
    /// See the host's fingerprint in the login step and trust it there.
    Trust,
}

/// Why a host could not be reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unreachable {
    TimedOut,
    Refused,
    Unresolved,
    /// No route, or `ssh` gave no more precise reason.
    Other,
}

/// One fact of a Machine's detail, worded by `localization`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Line {
    /// The remote's Orchbee version.
    Version(String),
    /// What an online Machine's Session holds.
    Contents {
        projects: usize,
        workspaces: usize,
        tabs: usize,
    },
    /// A Project or Workspace name of an online Machine.
    Label(String),
    /// The host does not answer; `host` is the record's host as the person named it.
    Unreachable { host: String, why: Unreachable },
    /// This computer has no OpenSSH client.
    NoSsh,
    /// How to install the OpenSSH client on this computer.
    InstallSsh,
    /// `ssh` failed for a reason no diagnostic matched.
    SshFailed { host: String },
    /// The remote end broke the link protocol.
    RemoteProtocol,
    /// The remote Session did not report ready.
    NotStarted,
    /// The link was lost.
    LinkLost,
    /// When the next automatic attempt starts, in milliseconds since the Unix epoch.
    NextAttempt(u64),
    /// When the last attempt ended, in milliseconds since the Unix epoch.
    LastAttempt(u64),
    /// The link is up but the remote does not answer.
    Unresponsive,
    /// Orchbee is being copied to the remote.
    Installing,
    /// The remote has no Orchbee of this environment.
    NotInstalled,
    /// The remote's system or processor is not supported, as far as it said which.
    Unsupported {
        platform: Option<String>,
        architecture: Option<String>,
    },
    /// The systems Orchbee runs on.
    Supported,
    /// The remote's Orchbee is too old.
    RemoteTooOld { version: Option<String> },
    /// The Session running on the remote is too old.
    SessionTooOld,
    /// How to update the remote.
    UpdateRemote,
    /// This computer's Orchbee is too old for the remote.
    LocalTooOld,
    /// How to update this computer's Orchbee.
    UpdateLocal,
    /// The remote is an Orchbee of another environment.
    OtherEnvironment,
    /// A login state's explanation.
    Login(LoginState),
    /// Why no state of the hub's holds now.
    Hub(HubStatus),
}

/// A Machine's detail lines and the action offered with them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Explanation {
    pub lines: Vec<Line>,
    pub action: Option<Action>,
}

impl Explanation {
    /// A disabled record: nothing to explain, and enabling it helps.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            lines: Vec::new(),
            action: Some(Action::Enable),
        }
    }

    /// The explanation of `observation`, with the `facts` the hub reported and the record's `host`.
    #[must_use]
    pub fn of(observation: &Observation, facts: &Facts, host: &str) -> Self {
        let version = || {
            facts
                .remote
                .as_ref()
                .and_then(Remote::product_version)
                .map(|version| Line::Version(version.as_str().to_owned()))
        };
        match observation {
            Observation::Hub(HubStatus::Unknown) => Self::default(),
            Observation::Hub(status) => Self {
                lines: vec![Line::Hub(status.clone())],
                action: None,
            },
            Observation::Online(overview) => {
                let mut lines: Vec<Line> = version().into_iter().collect();
                lines.push(Line::Contents {
                    projects: overview.projects,
                    workspaces: overview.workspaces,
                    tabs: overview.tabs,
                });
                lines.extend(overview.labels.iter().take(3).cloned().map(Line::Label));
                Self {
                    lines,
                    action: Some(Action::Open),
                }
            }
            Observation::Login(state) => Self {
                action: match state {
                    LoginState::NeedsLogin(_) => Some(Action::LogIn),
                    LoginState::LoginFailed(_) => Some(Action::LogInAgain),
                    LoginState::Untrusted(_) => Some(Action::Trust),
                    LoginState::KeyChanged { .. } => None,
                },
                lines: vec![Line::Login(state.clone())],
            },
            Observation::State(_) => facts
                .state
                .as_ref()
                .map_or_else(Self::default, |state| Self::state(state, facts, host)),
        }
    }

    fn state(state: &MachineState, facts: &Facts, host: &str) -> Self {
        let remote = facts.remote.as_ref();
        let (lines, action) = match state {
            MachineState::Online => (
                remote
                    .and_then(Remote::product_version)
                    .map(|version| Line::Version(version.as_str().to_owned()))
                    .into_iter()
                    .collect(),
                Some(Action::Open),
            ),
            MachineState::Connecting {
                stage: Some(Stage::Copying),
            }
            | MachineState::Reconnecting {
                stage: Some(Stage::Copying),
                ..
            } => (vec![Line::Installing], None),
            MachineState::Connecting { .. } | MachineState::Unknown => (Vec::new(), None),
            MachineState::Reconnecting {
                cause,
                next_attempt,
                ..
            } => {
                let mut lines = cause
                    .map(|cause| reconnect(cause, host))
                    .unwrap_or_default();
                lines.extend(next_attempt.map(|at| Line::NextAttempt(at.get())));
                (lines, Some(Action::RetryNow))
            }
            MachineState::Offline { cause, .. } => {
                let mut lines = cause.map(|cause| offline(cause, host)).unwrap_or_default();
                let local = matches!(cause, Some(OfflineCause::SshUnavailable));
                lines.extend(facts.last_attempt.map(Line::LastAttempt));
                // Without OpenSSH a retry fails the same way until it is installed.
                (lines, (!local).then_some(Action::Retry))
            }
            MachineState::Unresponsive => (vec![Line::Unresponsive], Some(Action::Reconnect)),
            MachineState::Disabled => (Vec::new(), Some(Action::Enable)),
            MachineState::NeedsLogin { cause } => (
                vec![Line::Login(LoginState::NeedsLogin(*cause))],
                Some(Action::LogIn),
            ),
            MachineState::LoginFailed { cause } => (
                vec![Line::Login(LoginState::LoginFailed(*cause))],
                Some(Action::LogInAgain),
            ),
            MachineState::Untrusted { host_key } => (
                vec![Line::Login(LoginState::Untrusted(host_key.clone()))],
                Some(Action::Trust),
            ),
            MachineState::KeyChanged { host_key } => (
                vec![Line::Login(LoginState::KeyChanged {
                    host_key: host_key.clone(),
                    host: crate::machines::known_hosts_name(host),
                })],
                None,
            ),
            MachineState::NotInstalled => (vec![Line::NotInstalled], None),
            MachineState::Unsupported { cause } => match cause {
                Some(UnsupportedCause::ConnectionKind) => (Vec::new(), None),
                _ => (
                    vec![
                        Line::Unsupported {
                            platform: remote
                                .and_then(Remote::platform)
                                .map(|token| token.as_str().to_owned()),
                            architecture: remote
                                .and_then(Remote::architecture)
                                .map(|token| token.as_str().to_owned()),
                        },
                        Line::Supported,
                    ],
                    None,
                ),
            },
            MachineState::NeedsUpdate { cause } => (
                vec![
                    match cause {
                        Some(NeedsUpdateCause::SessionTooOld) => Line::SessionTooOld,
                        Some(NeedsUpdateCause::RemoteTooOld) | None => Line::RemoteTooOld {
                            version: remote
                                .and_then(Remote::product_version)
                                .map(|version| version.as_str().to_owned()),
                        },
                    },
                    Line::UpdateRemote,
                ],
                None,
            ),
            MachineState::LocalNeedsUpdate => (vec![Line::LocalTooOld, Line::UpdateLocal], None),
            MachineState::Incompatible => (vec![Line::OtherEnvironment], None),
        };
        Self { lines, action }
    }
}

/// The detail of an `offline` cause.
fn offline(cause: OfflineCause, host: &str) -> Vec<Line> {
    let unreachable = |why| {
        vec![Line::Unreachable {
            host: display_host(host),
            why,
        }]
    };
    match cause {
        OfflineCause::HostTimeout => unreachable(Unreachable::TimedOut),
        OfflineCause::HostRefused => unreachable(Unreachable::Refused),
        OfflineCause::HostUnresolved => unreachable(Unreachable::Unresolved),
        OfflineCause::HostUnreachable => unreachable(Unreachable::Other),
        OfflineCause::SshUnavailable => vec![Line::NoSsh, Line::InstallSsh],
        OfflineCause::SshFailed => vec![Line::SshFailed {
            host: display_host(host),
        }],
        OfflineCause::RemoteProtocol => vec![Line::RemoteProtocol],
        OfflineCause::RemoteNotReady => vec![Line::NotStarted],
    }
}

/// The detail of a `reconnecting` cause: an `offline` cause, or the lost link.
fn reconnect(cause: ReconnectCause, host: &str) -> Vec<Line> {
    let offline_cause = match cause {
        ReconnectCause::LinkLost => return vec![Line::LinkLost],
        ReconnectCause::HostUnreachable => OfflineCause::HostUnreachable,
        ReconnectCause::HostTimeout => OfflineCause::HostTimeout,
        ReconnectCause::HostRefused => OfflineCause::HostRefused,
        ReconnectCause::HostUnresolved => OfflineCause::HostUnresolved,
        ReconnectCause::SshUnavailable => OfflineCause::SshUnavailable,
        ReconnectCause::SshFailed => OfflineCause::SshFailed,
        ReconnectCause::RemoteProtocol => OfflineCause::RemoteProtocol,
        ReconnectCause::RemoteNotReady => OfflineCause::RemoteNotReady,
    };
    offline(offline_cause, host)
}

/// The host of an `ssh` target as the person named it, without the user and the `ssh://` form: a
/// configured alias stays an alias, and a port shows only when the target names one, since the
/// `ssh` configuration may name another.
fn display_host(target: &str) -> String {
    let authority = target.strip_prefix("ssh://").unwrap_or(target);
    authority
        .rsplit_once('@')
        .map_or(authority, |(_, address)| address)
        .to_owned()
}

#[cfg(test)]
mod tests;
