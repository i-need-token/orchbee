//! Passive Client-local member reads and exact canonical identity validation.
use super::{Failure, FailureKind, client_control, console_interrupt::SignalWatch, query, results};
use crate::cli_text::CliText;
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::{
    ClientPresentationSelection, ClientViewControl, ClientViewControlStatus, ClientViewIntent,
    ClientViewOutcome, ClientViewRecord, PresentationBinding, Snapshot, TerminalId,
    TerminalPresentationId, WorkbenchClientViewResult, WorkbenchPanelId, WorkbenchRequestId,
    WorkerId,
};
use std::time::Instant;

#[derive(Debug, Clone)]
pub(super) struct Read {
    pub(super) member: ClientPresentationSelection,
    pub(super) operation_id: WorkbenchRequestId,
}

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selected: &ClientViewRecord,
    panel_id: &WorkbenchPanelId,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<ClientPresentationSelection, Failure> {
    read_with_receipt(client, snapshot, selected, panel_id, deadline, signals)
        .map(|read| read.member)
}

pub(super) fn read_with_receipt(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selected: &ClientViewRecord,
    panel_id: &WorkbenchPanelId,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<Read, Failure> {
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
    let answer = client_control::exchange::submit_control(
        client,
        &snapshot.session_id,
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
    if let Some(failure) =
        super::client_control::exchange::submission_refusal(answer.exit_code(), outcome)
    {
        return Err(failure);
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
        return Err(super::query::workbench_failure("client-changed"));
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
    if let Some(binding) = member.binding() {
        let rows = results::terminal_rows(snapshot);
        if rows
            .iter()
            .filter(|row| row.terminal_id == binding.terminal_id.as_str())
            .count()
            != 1
            || !rows.iter().any(|row| {
                row.panel_id == panel_id.as_str()
                    && row.presentation_id == member.presentation_id().as_str()
                    && row.terminal_id == binding.terminal_id.as_str()
                    && row.worker_id == binding.worker_id.as_str()
            })
        {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
    } else {
        let results::Projection::PanelGet(panel) = results::project(
            &crate::invocation::QueryCommand::PanelGet(crate::invocation::Target::Id(
                panel_id.to_string(),
            )),
            snapshot,
        )?
        else {
            return Err(Failure::new(FailureKind::InternalFailure));
        };
        if !std::iter::once(&panel.primary_presentation)
            .chain(&panel.child_presentations)
            .any(|row| {
                row.presentation_id == member.presentation_id().as_str()
                    && row.terminal_id.is_none()
                    && row.worker_id.is_none()
            })
        {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
    }
    Ok(Read {
        member,
        operation_id: command.operation_id,
    })
}

#[derive(Debug, Clone)]
pub(super) enum Origin {
    Primary,
    Caller(TerminalId),
    Client(Box<Read>),
}

/// One resolved member shared by every enrichment; canonical Panel structure stays immutable.
#[derive(Debug, Clone)]
pub(super) struct Resolved {
    pub(super) panel_id: WorkbenchPanelId,
    pub(super) presentation_id: TerminalPresentationId,
    pub(super) binding: Option<PresentationBinding>,
    presentation: results::PresentationRow,
    origin: Origin,
}
impl Resolved {
    pub(super) fn new(panel: &results::PanelDoc, origin: Origin) -> Result<Self, Failure> {
        let invalid = || Failure::new(FailureKind::ProtocolInvalid);
        let members =
            || std::iter::once(&panel.primary_presentation).chain(&panel.child_presentations);
        let presentation = match &origin {
            Origin::Primary => &panel.primary_presentation,
            Origin::Caller(terminal) => members()
                .find(|row| row.terminal_id.as_deref() == Some(terminal.as_str()))
                .ok_or_else(invalid)?,
            Origin::Client(read) => members()
                .find(|row| row.presentation_id == read.member.presentation_id().as_str())
                .ok_or_else(invalid)?,
        };
        let binding = match (&presentation.terminal_id, &presentation.worker_id) {
            (Some(terminal), Some(worker)) => Some(PresentationBinding {
                terminal_id: TerminalId::parse(terminal).map_err(|_| invalid())?,
                worker_id: WorkerId::parse(worker).map_err(|_| invalid())?,
            }),
            (None, None) => None,
            _ => return Err(invalid()),
        };
        if let Origin::Client(read) = &origin
            && binding.as_ref() != read.member.binding()
        {
            return Err(invalid());
        }
        Ok(Self {
            panel_id: WorkbenchPanelId::parse(&panel.row.panel_id).map_err(|_| invalid())?,
            presentation_id: TerminalPresentationId::parse(&presentation.presentation_id)
                .map_err(|_| invalid())?,
            binding,
            presentation: presentation.clone(),
            origin,
        })
    }
    pub(super) fn client(&self) -> Option<&Read> {
        match &self.origin {
            Origin::Client(read) => Some(read),
            _ => None,
        }
    }
    pub(super) fn value(&self) -> serde_json::Value {
        let mut value = results::presentation_json(&self.presentation);
        value["source"] = match &self.origin {
            Origin::Primary => serde_json::json!({"kind":"primary"}),
            Origin::Caller(_) => serde_json::json!({"kind":"caller"}),
            Origin::Client(read) => match &read.member {
                ClientPresentationSelection::Bound {
                    attachment_generation,
                    incarnation,
                    ..
                } => {
                    serde_json::json!({"kind":"client", "attachmentGeneration":attachment_generation,"incarnation":incarnation})
                }
                ClientPresentationSelection::Unbound {
                    selection_generation,
                    ..
                } => {
                    serde_json::json!({"kind":"client-unbound", "selectionGeneration":selection_generation})
                }
            },
        };
        value
    }
    pub(super) fn render(
        &self,
        out: &mut super::output::Bounded,
        locale: crate::localization::Locale,
    ) -> Result<(), super::output::TooLarge> {
        out.write(format_args!(
            "{}: {}  {}\n",
            CliText::new(locale).cli_current_presentation_label(),
            self.presentation_id,
            crate::cli_text::escape::escape(&self.presentation.display_label)
        ))
    }
}

#[cfg(test)]
mod tests;
