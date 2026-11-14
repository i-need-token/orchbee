//! What ends an attempt: the result a carrier's end, a refused offer or the link reads as, and the
//! state and `connect` outcome each result gives. `docs/machines.md#how-a-carrier-end-reads` owns
//! the table; OpenSSH exits 255 for every failure, so only its standard error tells them apart.

use crate::cli::machines::askpass::question::changed_key;
use orchbee_connection_handshake::RejectionReason;
use orchbee_machine_protocol::{
    HostKey,
    hub::{
        ConnectFailure, LoginFailedCause, MachineState, NeedsLoginCause, NeedsUpdateCause,
        OfflineCause, ReconnectCause, UnsupportedCause,
    },
};
use orchbee_workbench_client::application::machine_ssh::PROGRAM_NOT_FOUND;

/// The exit status OpenSSH gives every failure of its own.
const SSH_FAILED: i32 = 255;

/// How a carrier ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum End {
    /// The program could not be started; `missing` when it does not exist.
    NotStarted { missing: bool },
    /// The child ended with `code`, unknown when it could not be settled, and the last bytes of
    /// its standard error.
    Exited { code: Option<i32>, stderr: Vec<u8> },
}

/// The result of an attempt that did not reach `online`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum Failure {
    /// Transient: a hub that keeps the Machine retries by itself.
    Offline(OfflineCause),
    /// A person must answer a question and nobody did.
    NeedsLogin,
    /// `ssh` refused the remembered answer and nobody could be asked instead.
    RememberedRejected,
    LoginFailed(LoginFailedCause),
    /// A first-time host key nobody confirmed.
    Untrusted(HostKey),
    /// The host key changed; the offered key when `ssh` printed it.
    KeyChanged(Option<HostKey>),
    NotInstalled,
    Unsupported(UnsupportedCause),
    NeedsUpdate(NeedsUpdateCause),
    LocalNeedsUpdate,
    Incompatible,
}

impl Failure {
    /// The cause of a transient failure; none for a permanent one.
    pub(super) const fn transient(&self) -> Option<OfflineCause> {
        match self {
            Self::Offline(cause) => Some(*cause),
            _ => None,
        }
    }

    /// The state the Machine reads after this failure, when nothing retries it.
    pub(super) fn state(&self) -> MachineState {
        match self {
            Self::Offline(cause) => MachineState::Offline {
                cause: Some(*cause),
                next_attempt: None,
            },
            Self::NeedsLogin => MachineState::NeedsLogin {
                cause: Some(NeedsLoginCause::AuthRequired),
            },
            Self::RememberedRejected => MachineState::NeedsLogin {
                cause: Some(NeedsLoginCause::RememberedRejected),
            },
            Self::LoginFailed(cause) => MachineState::LoginFailed {
                cause: Some(*cause),
            },
            Self::Untrusted(host_key) => MachineState::Untrusted {
                host_key: host_key.clone(),
            },
            Self::KeyChanged(host_key) => MachineState::KeyChanged {
                host_key: host_key.clone(),
            },
            Self::NotInstalled => MachineState::NotInstalled,
            Self::Unsupported(cause) => MachineState::Unsupported {
                cause: Some(*cause),
            },
            Self::NeedsUpdate(cause) => MachineState::NeedsUpdate {
                cause: Some(*cause),
            },
            Self::LocalNeedsUpdate => MachineState::LocalNeedsUpdate,
            Self::Incompatible => MachineState::Incompatible,
        }
    }

    /// The `connect` outcome the state table gives this result.
    pub(super) const fn outcome(&self) -> ConnectFailure {
        match self {
            Self::Offline(cause) => match cause {
                OfflineCause::HostUnreachable
                | OfflineCause::HostTimeout
                | OfflineCause::HostRefused
                | OfflineCause::HostUnresolved => ConnectFailure::HostUnreachable,
                OfflineCause::SshUnavailable => ConnectFailure::SshUnavailable,
                OfflineCause::SshFailed => ConnectFailure::SshFailed,
                OfflineCause::RemoteProtocol => ConnectFailure::RemoteProtocol,
                OfflineCause::RemoteNotReady => ConnectFailure::RemoteNotReady,
            },
            Self::NeedsLogin | Self::RememberedRejected => ConnectFailure::AuthRequired,
            Self::LoginFailed(_) => ConnectFailure::AuthFailed,
            Self::Untrusted(_) => ConnectFailure::HostUntrusted,
            Self::KeyChanged(_) => ConnectFailure::HostKeyChanged,
            Self::NotInstalled => ConnectFailure::RemoteNotInstalled,
            Self::Unsupported(UnsupportedCause::RemotePlatform) => {
                ConnectFailure::RemoteUnsupported
            }
            Self::Unsupported(UnsupportedCause::ConnectionKind) => {
                ConnectFailure::UnsupportedMachine
            }
            Self::NeedsUpdate(_) | Self::LocalNeedsUpdate => ConnectFailure::VersionIncompatible,
            Self::Incompatible => ConnectFailure::RemoteOtherEnvironment,
        }
    }
}

/// The `reconnecting` cause of a transient failure.
pub(super) const fn reconnect_cause(cause: OfflineCause) -> ReconnectCause {
    match cause {
        OfflineCause::HostUnreachable => ReconnectCause::HostUnreachable,
        OfflineCause::HostTimeout => ReconnectCause::HostTimeout,
        OfflineCause::HostRefused => ReconnectCause::HostRefused,
        OfflineCause::HostUnresolved => ReconnectCause::HostUnresolved,
        OfflineCause::SshUnavailable => ReconnectCause::SshUnavailable,
        OfflineCause::SshFailed => ReconnectCause::SshFailed,
        OfflineCause::RemoteProtocol => ReconnectCause::RemoteProtocol,
        OfflineCause::RemoteNotReady => ReconnectCause::RemoteNotReady,
    }
}

/// What a carrier's end reads as when it ended before the helper answered.
pub(in super::super) fn classify(end: &End) -> Failure {
    let (code, stderr) = match end {
        End::NotStarted { missing: true } => {
            return Failure::Offline(OfflineCause::SshUnavailable);
        }
        End::NotStarted { missing: false } => return Failure::Offline(OfflineCause::SshFailed),
        End::Exited { code, stderr } => (*code, stderr),
    };
    match code {
        Some(PROGRAM_NOT_FOUND) => Failure::NotInstalled,
        Some(SSH_FAILED) => {
            match ssh_failure(&String::from_utf8_lossy(stderr).to_ascii_lowercase()) {
                Failure::KeyChanged(_) => Failure::KeyChanged(changed_key(stderr)),
                failure => failure,
            }
        }
        _ => Failure::Offline(OfflineCause::RemoteProtocol),
    }
}

/// An OpenSSH failure by the wording of its standard error, already in lower case.
fn ssh_failure(text: &str) -> Failure {
    let any = |phrases: &[&str]| phrases.iter().any(|phrase| text.contains(phrase));
    if text.contains("remote host identification has changed") {
        Failure::KeyChanged(None)
    } else if text.contains("host key verification failed") {
        Failure::NeedsLogin
    } else if let Some(at) = text.find("permission denied") {
        let methods = text.get(at..).unwrap_or_default();
        if methods.contains("password") || methods.contains("keyboard-interactive") {
            Failure::NeedsLogin
        } else {
            Failure::LoginFailed(LoginFailedCause::KeyRejected)
        }
    } else if any(&[
        "could not resolve hostname",
        "no such host is known",
        "name or service not known",
    ]) {
        Failure::Offline(OfflineCause::HostUnresolved)
    } else if text.contains("connection refused") {
        Failure::Offline(OfflineCause::HostRefused)
    } else if text.contains("timed out") {
        Failure::Offline(OfflineCause::HostTimeout)
    } else if any(&[
        "network is unreachable",
        "no route to host",
        "host is unreachable",
    ]) {
        Failure::Offline(OfflineCause::HostUnreachable)
    } else {
        Failure::Offline(OfflineCause::SshFailed)
    }
}

/// What a refused `machine-link` offer reads as.
pub(in super::super) const fn refusal(reason: RejectionReason) -> Failure {
    match reason {
        RejectionReason::InitiatorTooOld => Failure::LocalNeedsUpdate,
        RejectionReason::InitiatorTooNew => Failure::NeedsUpdate(NeedsUpdateCause::RemoteTooOld),
        RejectionReason::EnvironmentMismatch | RejectionReason::FingerprintMismatch => {
            Failure::Incompatible
        }
        _ => Failure::Offline(OfflineCause::RemoteProtocol),
    }
}
