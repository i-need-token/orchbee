//! Shared canonical proof for Agent and metadata report submission.
use super::{Failure, FailureKind, results};
use crate::invocation::{QueryCommand, Target};
use orchbee_workbench_protocol::{
    PresentationBinding, Snapshot, TerminalId, TerminalPresentationId, WorkbenchPanelId, WorkerId,
};

pub(super) fn resolve(
    snapshot: &Snapshot,
    target: &Target,
    presentation_id: Option<&TerminalPresentationId>,
) -> Result<(WorkbenchPanelId, PresentationBinding), Failure> {
    let results::Projection::PanelGet(panel) =
        results::project(&QueryCommand::PanelGet(target.clone()), snapshot)?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    let unavailable = || {
        Failure::new(FailureKind::Rejected).details(serde_json::Map::from_iter([
            ("stage".to_owned(), serde_json::json!("command")),
            ("reason".to_owned(), serde_json::json!("panel-unavailable")),
            ("revision".to_owned(), serde_json::json!(snapshot.revision)),
        ]))
    };
    let member = match presentation_id {
        None => &panel.primary_presentation,
        Some(id) => std::iter::once(&panel.primary_presentation)
            .chain(&panel.child_presentations)
            .find(|member| member.presentation_id == id.as_str())
            .ok_or_else(unavailable)?,
    };
    let (Some(terminal), Some(worker)) = (&member.terminal_id, &member.worker_id) else {
        return Err(unavailable());
    };
    let invalid = || Failure::new(FailureKind::ProtocolInvalid);
    Ok((
        WorkbenchPanelId::parse(&panel.row.panel_id).map_err(|_| invalid())?,
        PresentationBinding {
            terminal_id: TerminalId::parse(terminal).map_err(|_| invalid())?,
            worker_id: WorkerId::parse(worker).map_err(|_| invalid())?,
        },
    ))
}
