//! Saved-machine carrier capability; local Client transport is independent.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(windows)]
pub use windows::serve;
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
use unix as platform;

use crate::{
    local_client::{LocalClient, RetainedCarrier, cancellation::Cancellation},
    machine_refusal::Refusal,
    machine_ssh,
    machines::Profile,
    tui_capabilities::Capability,
};
use std::{
    io,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    thread::JoinHandle,
    time::Instant,
};

/// Native creation transfers these resources directly into the retained Client guard.
struct StartedCarrier {
    endpoint: PathBuf,
    stop: Arc<AtomicBool>,
    task: JoinHandle<io::Result<()>>,
}

/// Admit one saved-machine connection without changing its deadline or replaying its probe.
pub(crate) fn connect(
    profile: &Profile,
    deadline: Instant,
    cancelled: Cancellation<'_>,
) -> io::Result<LocalClient> {
    // An unavailable host refuses before inspecting the profile or cancellation source.
    Capability::SavedMachines.require()?;
    if !profile.enabled {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let admitted = machine_ssh::probe(&profile.draft, deadline, || cancelled.is_cancelled())?;
    if cancelled.is_cancelled()? {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let StartedCarrier {
        endpoint,
        stop,
        task,
    } = platform::prepare(profile, &admitted)?;
    let guard = Arc::new(RetainedCarrier::new(stop, task));
    // The tunnel carries an ordinary Client connection: the remote Session admits this build's
    // own offer through the handshake. The invocation-owned wake is observed throughout.
    let offer = crate::local_client::client_offer(None);
    let client =
        LocalClient::connect_before_cancellable(&endpoint, &offer, deadline, cancelled.wake())
            .map_err(admission_error)?;
    if cancelled.is_cancelled()? {
        return Err(io::ErrorKind::Interrupted.into());
    }
    Ok(client.with_retained_carrier(guard))
}

/// The connection failure discovery classifies. A handshake refusal that needs the user to change
/// one side carries its [`Refusal`]; capacity, stopping, and timeout stay transient.
pub(crate) fn admission_error(error: crate::local_client::LocalClientConnectError) -> io::Error {
    use crate::local_client::LocalClientConnectError;
    use orchbee_local_client_protocol::ConnectClientError;
    match error {
        LocalClientConnectError::RequestIoConfiguration(error)
        | LocalClientConnectError::Admission(
            ConnectClientError::ConnectFailed(error)
            | ConnectClientError::Io(error)
            | ConnectClientError::CancellationSourceFault(error),
        ) => error,
        LocalClientConnectError::Admission(ConnectClientError::CancellationReady) => {
            io::ErrorKind::Interrupted.into()
        }
        LocalClientConnectError::Admission(ConnectClientError::Rejected(
            orchbee_local_client_protocol::RejectionReason::CapacityExceeded
            | orchbee_local_client_protocol::RejectionReason::HandshakeTimeout
            | orchbee_local_client_protocol::RejectionReason::Stopping,
        )) => io::ErrorKind::ConnectionRefused.into(),
        LocalClientConnectError::Admission(ConnectClientError::Refused(rejection)) => {
            use orchbee_connection_handshake::RejectionReason as Reason;
            match rejection.reason {
                Reason::Capacity | Reason::Stopping | Reason::Timeout => {
                    io::ErrorKind::ConnectionRefused.into()
                }
                Reason::EnvironmentMismatch => Refusal::OtherEnvironment.into(),
                Reason::InitiatorTooOld => Refusal::ClientTooOld.into(),
                Reason::InitiatorTooNew => Refusal::SessionTooOld.into(),
                Reason::FingerprintMismatch => Refusal::DevelopmentBuildMismatch.into(),
                Reason::Malformed | Reason::ConnectionUnsupported | Reason::Unknown => {
                    io::ErrorKind::PermissionDenied.into()
                }
            }
        }
        // A remote Session that predates the handshake is the older side.
        LocalClientConnectError::Admission(ConnectClientError::PredatesHandshake) => {
            Refusal::SessionTooOld.into()
        }
        LocalClientConnectError::Admission(ConnectClientError::Rejected(_)) => {
            io::ErrorKind::PermissionDenied.into()
        }
        LocalClientConnectError::Admission(
            ConnectClientError::Protocol(_) | ConnectClientError::UnexpectedResponse,
        ) => io::ErrorKind::InvalidData.into(),
    }
}
