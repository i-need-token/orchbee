//! Secret-free receipts for the layout-specific send-once command adapter.
use super::error;
use crate::{
    cli::{Failure, FailureKind, query},
    local_client::{
        LocalClient,
        workbench::creation::{CreationRequest, CreationSubmission},
    },
};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
};
use serde_json::{Value, json};
use std::{io, time::Instant};

pub(super) fn submit(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    action: WorkbenchAction,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<(Value, Result<WorkbenchCommandOutcome, Failure>), Failure> {
    let kind = match &action {
        WorkbenchAction::TabCreateLayout { .. } => "tab.create-layout",
        WorkbenchAction::PanelCreateTerminal { .. } => "panel.create-terminal",
        _ => return Err(error(FailureKind::InternalFailure, "invalid-receipt")),
    };
    let request_id = query::request_id()?;
    let request = CreationRequest::try_from(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: request_id.clone(),
        site_id: snapshot.site_id.clone(),
        expected_revision: snapshot.revision,
        action,
    })
    .map_err(|_| error(FailureKind::InternalFailure, "invalid-receipt"))?;
    let (result, outcome) = match client.submit_workbench_creation(request, deadline, wake) {
        CreationSubmission::Outcome(outcome) => {
            let outcome = *outcome;
            let result = json!(outcome);
            let failure = match &outcome {
                WorkbenchCommandOutcome::Denied {} => Some(FailureKind::Denied),
                WorkbenchCommandOutcome::Rejected { .. } => Some(FailureKind::Rejected),
                WorkbenchCommandOutcome::Unavailable { .. } => {
                    Some(FailureKind::ServiceUnavailable)
                }
                _ => None,
            };
            (
                result,
                failure.map_or_else(|| Ok(outcome), |kind| Err(error(kind, "layout-changed"))),
            )
        }
        CreationSubmission::NotSubmitted { kind, reason } => {
            let failure = match kind {
                io::ErrorKind::Interrupted => FailureKind::Cancelled,
                io::ErrorKind::TimedOut => FailureKind::Timeout,
                io::ErrorKind::InvalidInput => FailureKind::ProtocolInvalid,
                _ => FailureKind::ServiceUnavailable,
            };
            (
                json!({"kind":"not-submitted","reason":reason}),
                Err(error(failure, "launch-unavailable")),
            )
        }
        CreationSubmission::Unknown { reason } => (
            json!({"kind":"outcome-unknown","reason":reason}),
            Err(error(FailureKind::OutcomeUnknown, "launch-unavailable")),
        ),
    };
    Ok((
        json!({"action":kind,"siteId":snapshot.site_id,"requestId":request_id,"baseRevision":snapshot.revision,"result":result}),
        outcome,
    ))
}
