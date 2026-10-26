//! Saved-machine carrier capability; local Client transport is independent.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(windows)]
pub(crate) use windows::{Tunnel, serve};
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
use unix as platform;

use crate::{
    local_client::{LocalClient, cancellation::Cancellation},
    machine_ssh,
    machines::Profile,
    tui_capabilities::Capability,
};
use std::{io, time::Instant};

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
    platform::connect(profile, admitted, deadline, cancelled)
}

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
        LocalClientConnectError::Admission(ConnectClientError::Rejected(_)) => {
            io::ErrorKind::PermissionDenied.into()
        }
        LocalClientConnectError::Admission(
            ConnectClientError::Protocol(_) | ConnectClientError::UnexpectedResponse,
        ) => io::ErrorKind::InvalidData.into(),
    }
}
