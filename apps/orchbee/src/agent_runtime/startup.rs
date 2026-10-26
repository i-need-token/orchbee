//! Shared startup protocol operations. Callers retain discovery, selectors, presentation and signal ownership.
pub(crate) use super::processes::{Selection, interpret};
pub(crate) use super::{SendVerdict, wait_backoff};
pub(crate) mod binding;
pub(crate) mod observation;
pub(crate) mod processes;
pub(crate) mod send;
pub(crate) mod snapshot;
use crate::local_client::{
    LocalClient, LocalClientConnectError, LocalClientOrigin, LocalOperationClock,
};
use orchbee_agent_observation_protocol::ObapRequestId;
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{Snapshot, WorkbenchSnapshotOutcome};
use std::time::Instant;
#[cfg(windows)]
#[path = "startup/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "startup/unix.rs"]
mod platform;
pub(crate) use platform::admit;

/// This native Agent launch operation has no adapter on the current host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Unsupported;

#[derive(Debug)]
pub(crate) struct IdentityError;

fn identity() -> Result<String, IdentityError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| IdentityError)?;
    Ok(format!("{:032x}", u128::from_be_bytes(bytes)))
}

pub(crate) fn obap_identity() -> Result<ObapRequestId, IdentityError> {
    ObapRequestId::parse(&identity()?).map_err(|_| IdentityError)
}
pub(crate) fn obcs_identity() -> Result<orchbee_client_server_protocol::RequestId, IdentityError> {
    orchbee_client_server_protocol::RequestId::parse(&identity()?).map_err(|_| IdentityError)
}
pub(crate) fn workbench_request_id()
-> Result<orchbee_workbench_protocol::WorkbenchRequestId, IdentityError> {
    orchbee_workbench_protocol::WorkbenchRequestId::parse(&identity()?).map_err(|_| IdentityError)
}

#[derive(Debug)]
pub(crate) enum AdmissionError {
    Connect(LocalClientConnectError),
    Snapshot(snapshot::Error),
    Timeout,
    Denied,
    Unavailable,
}

/// Preserve the caller's operation clock, retained carrier and absolute budget on its new lane.
pub(crate) fn snapshot_from_origin(
    origin: &LocalClientOrigin,
    clock: LocalOperationClock,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<(LocalClient, Snapshot), AdmissionError> {
    let mut client = origin
        .lane_before_cancellable(deadline, clock, wake)
        .map_err(AdmissionError::Connect)?;
    let observed = snapshot::read(&mut client, deadline, wake).map_err(AdmissionError::Snapshot)?;
    if Instant::now() >= deadline {
        return Err(AdmissionError::Timeout);
    }
    match observed {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok((client, snapshot)),
        WorkbenchSnapshotOutcome::Denied {} => Err(AdmissionError::Denied),
        WorkbenchSnapshotOutcome::Unavailable { .. } => Err(AdmissionError::Unavailable),
    }
}
