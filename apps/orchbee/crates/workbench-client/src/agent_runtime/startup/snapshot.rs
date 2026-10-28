//! A single Snapshot exchange on the caller's admitted Client and absolute budget.
use crate::local_client::{LocalClient, workbench::SnapshotReadError};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{ProtocolVersion, SnapshotReadIntent, WorkbenchSnapshotOutcome};
use std::time::Instant;

/// Distinguish failure before request submission from the Client's typed Snapshot exchange failure.
#[derive(Debug)]
pub enum Error {
    Identity,
    Exchange(SnapshotReadError),
}

/// Submit one Snapshot read on the borrowed Client, retaining the caller's deadline and wake.
/// Return the Server outcome unchanged so each caller can retain its post-read settlement policy.
///
/// # Errors
/// Returns `Identity` before submission, or preserves the exchange failure without retrying.
pub fn read(
    client: &mut LocalClient,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<WorkbenchSnapshotOutcome, Error> {
    let request_id = super::workbench_request_id().map_err(|_| Error::Identity)?;
    client
        .submit_workbench_snapshot_read_before_cancellable(
            SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id,
            },
            deadline,
            wake,
        )
        .map(|result| result.outcome)
        .map_err(Error::Exchange)
}
