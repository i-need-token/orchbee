//! Post-open Client control is fenced separately from the retained Git operation.
use super::post_open::{self, Receipt};
use crate::{
    cli::{
        Failure, FailureKind, client_control::windows as control, query, source::ArmedSignals,
        worktree_source::windows::check,
    },
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewControl, ClientViewIntent,
    ClientViewOutcome, ClientViewPhase, ClientViewRecord, ProtocolVersion, Snapshot,
    SnapshotReadIntent, WorkbenchProjectId, WorkbenchSnapshotOutcome, WorkbenchWorkspaceId,
};
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) fn execute(
    client: &mut LocalClient,
    original: &Snapshot,
    target: (&WorkbenchProjectId, &WorkbenchWorkspaceId),
    selected: &ClientViewRecord,
    waiting: (Instant, &mut ArmedSignals),
    receipt: &mut Receipt,
) -> Result<(), Failure> {
    let (deadline, signals) = waiting;
    check(signals, deadline)?;
    let reply = client.submit_workbench_snapshot_read_before(
        SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: query::request_id()?,
        },
        deadline.min(Instant::now() + Duration::from_secs(2)),
    );
    check(signals, deadline)?;
    let fresh = match reply
        .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))?
        .outcome
    {
        WorkbenchSnapshotOutcome::Available { snapshot }
            if snapshot.site_id == original.site_id && snapshot.revision >= original.revision =>
        {
            snapshot
        }
        WorkbenchSnapshotOutcome::Available { .. } => {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        WorkbenchSnapshotOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
        WorkbenchSnapshotOutcome::Unavailable { .. } => {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
    };
    let workspace = fresh
        .navigation
        .projects
        .iter()
        .find(|project| &project.project_id == target.0)
        .and_then(|project| {
            project
                .workspaces
                .iter()
                .find(|workspace| &workspace.workspace_id == target.1)
        })
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    let current = loop {
        check(signals, deadline)?;
        let reply = control::exchange(
            client,
            &fresh.site_id,
            ClientViewAction::Get {
                client_id: selected.client_id.clone(),
            },
            deadline,
            signals,
        )?;
        check(signals, deadline)?;
        let current = match reply.outcome {
            ClientViewOutcome::Found { client } => client,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::NotFound {} | ClientViewOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
        };
        if current.availability != ClientViewAvailability::Current
            || current.view.phase == ClientViewPhase::Unavailable
        {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        if current.view.phase == ClientViewPhase::Ready {
            if !post_open::unchanged(selected, &current) {
                return Err(Failure::new(FailureKind::Denied));
            }
            if current.view.workbench_revision >= fresh.revision {
                break current;
            }
        }
        thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    };
    check(signals, deadline)?;
    receipt.stage = "focus";
    receipt.record(control::submit_control(
        client,
        &fresh.site_id,
        ClientViewControl {
            operation_id: query::request_id()?,
            client_id: selected.client_id.clone(),
            expected_view_revision: current.view_revision,
            minimum_workbench_revision: fresh.revision,
            intent: ClientViewIntent::Context {
                terminal_context_id: workspace.terminal_context_id.clone(),
            },
        },
        deadline,
        signals,
    )?);
    Ok(())
}
