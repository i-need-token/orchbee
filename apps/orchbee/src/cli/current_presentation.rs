//! Passive Client-local member reads and exact canonical identity validation.
use super::{Failure, FailureKind, client_control, query, results, source::ArmedSignals};
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::*;
use std::time::Instant;

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selected: &ClientViewRecord,
    panel_id: &WorkbenchPanelId,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientPresentationSelection, Failure> {
    signals.check_before(deadline)?;
    let command = ClientViewControl {
        operation_id: query::request_id()?,
        client_id: selected.client_id.clone(),
        expected_view_revision: selected.view_revision,
        minimum_workbench_revision: snapshot.revision,
        intent: ClientViewIntent::Presentation {
            panel_id: panel_id.clone(),
        },
    };
    let answer = client_control::windows::submit_control(
        client,
        &snapshot.site_id,
        command.clone(),
        deadline,
        signals,
    )?;
    signals.check_before(deadline)?;
    let outcome = answer.reply.as_ref().map(|reply| &reply.outcome);
    if let Some(ClientViewOutcome::Operation { operation }) = outcome
        && operation.command != command
    {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    }
    if let Some(kind) = super::workbench_query::submission_failure(answer.exit_code(), outcome) {
        return Err(Failure::new(kind));
    }
    let Some(WorkbenchClientViewResult {
        outcome: ClientViewOutcome::Operation { operation },
        ..
    }) = answer.reply
    else {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    };
    let ClientViewControlStatus::PresentationRead { presentation, view } = operation.status else {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    };
    if presentation.workbench_revision != snapshot.revision || view != selected.view {
        return Err(super::workbench_agents::failure("client-changed"));
    }
    if !presentation.answers(panel_id, &view, snapshot.revision) {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    }
    let member = presentation.presentation.ok_or_else(|| {
        client_control::selection::failure(
            FailureKind::NotFound,
            "client-panel-unavailable",
            vec![selected.client_id.to_string()],
        )
    })?;
    let rows = results::terminal_rows(snapshot);
    if rows
        .iter()
        .filter(|row| row.terminal_id == member.binding.terminal_id.as_str())
        .count()
        != 1
        || !rows.iter().any(|row| {
            row.panel_id == panel_id.as_str()
                && row.presentation_id == member.presentation_id.as_str()
                && row.terminal_id == member.binding.terminal_id.as_str()
                && row.worker_id == member.binding.worker_id.as_str()
        })
    {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    }
    Ok(member)
}
