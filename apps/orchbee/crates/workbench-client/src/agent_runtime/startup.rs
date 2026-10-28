//! Shared startup protocol operations. Callers retain discovery, selectors, presentation and signal ownership.
pub(crate) use super::processes::{Selection, interpret};
pub(crate) use super::{SendVerdict, wait_backoff};
pub mod binding;
pub mod observation;
pub mod processes;
pub mod send;
pub mod snapshot;
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
pub use platform::admit;

/// This native Agent launch operation has no adapter on the current host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unsupported;

/// Fresh OS entropy could not be obtained or admitted as a protocol request identity.
#[derive(Debug)]
pub struct IdentityError;

fn identity() -> Result<String, IdentityError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| IdentityError)?;
    Ok(format!("{:032x}", u128::from_be_bytes(bytes)))
}

/// Generate a fresh OBAP request identity without a fallback or reusable counter.
///
/// # Errors
/// Returns `IdentityError` if entropy acquisition or protocol admission fails.
pub fn obap_identity() -> Result<ObapRequestId, IdentityError> {
    ObapRequestId::parse(&identity()?).map_err(|_| IdentityError)
}
/// Generate a fresh OBCS request identity before any associated submission.
///
/// # Errors
/// Returns `IdentityError` if entropy acquisition or protocol admission fails.
pub(crate) fn obcs_identity() -> Result<orchbee_client_server_protocol::RequestId, IdentityError> {
    orchbee_client_server_protocol::RequestId::parse(&identity()?).map_err(|_| IdentityError)
}
/// Generate a fresh Workbench request identity without consulting caller presentation state.
///
/// # Errors
/// Returns `IdentityError` if entropy acquisition or protocol admission fails.
pub(crate) fn workbench_request_id()
-> Result<orchbee_workbench_protocol::WorkbenchRequestId, IdentityError> {
    orchbee_workbench_protocol::WorkbenchRequestId::parse(&identity()?).map_err(|_| IdentityError)
}

/// Failure to obtain a usable Snapshot on a new lane of an already selected origin.
/// Connection and exchange evidence remain distinct from a settled deadline or Server refusal.
#[derive(Debug)]
pub enum AdmissionError {
    Connect(LocalClientConnectError),
    Snapshot(snapshot::Error),
    Timeout,
    Denied,
    Unavailable,
}

/// Preserve the caller's operation clock, retained carrier and absolute budget on its new lane.
/// The returned Client owns that lane; callers must release it before finalizing their signal source.
///
/// # Errors
/// Distinguishes connection and Snapshot exchange failures from a post-read deadline,
/// access denial, or unavailable Snapshot. No new budget or alternate origin is chosen.
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
