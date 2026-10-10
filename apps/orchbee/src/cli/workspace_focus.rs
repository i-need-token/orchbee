//! Focus admission is shared by Workspace creation and Worktree opening, with distinct navigation guards.
use super::{
    Failure, FailureKind,
    client_control::{self, windows as control},
    query,
    source::ArmedSignals,
};
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewControl, ClientViewIntent,
    ClientViewOutcome, ClientViewPhase, ClientViewRecord, Snapshot, WorkbenchPanelId,
    WorkbenchRevision, WorkbenchTabId, WorkbenchWorkspaceId,
};
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) enum Navigation<'a> {
    Unchanged,
    Created {
        tab: &'a WorkbenchTabId,
        panel: &'a WorkbenchPanelId,
    },
}
pub(super) struct Target<'a> {
    pub(super) workspace: &'a WorkbenchWorkspaceId,
    pub(super) minimum: WorkbenchRevision,
    pub(super) navigation: Navigation<'a>,
}
pub(super) fn execute(
    client: &mut LocalClient,
    original: &Snapshot,
    target: Target<'_>,
    selected: &ClientViewRecord,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<client_control::Answer, Failure> {
    let fresh =
        query::refresh_snapshot(client, &original.site_id, target.minimum, deadline, signals)?;
    let workspace = fresh
        .navigation
        .workspace(target.workspace)
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    if let Navigation::Created { tab, panel } = &target.navigation {
        let valid =
            super::results::panel_entries(&fresh)
                .into_iter()
                .any(|(id, context, entry)| {
                    id == *panel
                        && &entry.tab_id == *tab
                        && context.terminal_context_id == workspace.terminal_context_id
                });
        if !valid {
            return Err(Failure::new(FailureKind::NotFound));
        }
    }
    let current = loop {
        signals.check_before(deadline)?;
        let reply = control::exchange(
            client,
            &fresh.site_id,
            ClientViewAction::Get {
                client_id: selected.client_id.clone(),
            },
            deadline,
            signals,
        )?;
        signals.check_before(deadline)?;
        let current = match reply.outcome {
            ClientViewOutcome::Found { client } => client,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::NotFound {} | ClientViewOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
        };
        if current.client_id != selected.client_id {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        if current.availability != ClientViewAvailability::Current
            || current.view.phase == ClientViewPhase::Unavailable
        {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        if current.view.phase == ClientViewPhase::Ready {
            let allowed = unchanged(selected, &current)
                || matches!(&target.navigation,Navigation::Created {tab,panel} if current.view.workbench_revision>=selected.view.workbench_revision && current.view.selection.as_ref().is_some_and(|view| view.terminal_context_id==workspace.terminal_context_id && view.tab_id.as_ref()==Some(*tab) && view.panel_id.as_ref()==Some(*panel) && !view.zoomed));
            if !allowed {
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
    signals.check_before(deadline)?;
    control::submit_control(
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
    )
}

pub(super) fn unchanged(original: &ClientViewRecord, current: &ClientViewRecord) -> bool {
    current.client_id == original.client_id
        && current.availability == ClientViewAvailability::Current
        && current.view.phase == ClientViewPhase::Ready
        && current.view.workbench_revision >= original.view.workbench_revision
        && matches!((&original.view.selection,&current.view.selection),(Some(before),Some(after)) if before.terminal_context_id==after.terminal_context_id && before.tab_id==after.tab_id && before.panel_id==after.panel_id && before.zoomed==after.zoomed)
}
