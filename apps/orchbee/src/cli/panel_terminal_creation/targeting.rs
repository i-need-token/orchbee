//! Resolve only canonical members; a child never substitutes for an empty primary.
use crate::{
    cli::{Failure, FailureKind, results},
    invocation::{
        Resource, Target,
        panel_terminal_creation::{Member, PanelTerminalCreationCommand},
    },
};
use orchbee_workbench_protocol::{
    PanelNode, Snapshot, WorkbenchLabel, WorkbenchPanelId, WorkbenchPanelTerminalTarget,
    WorkbenchTabId, WorkbenchTerminalContextId,
};

pub(super) struct Resolved {
    pub(super) context: WorkbenchTerminalContextId,
    pub(super) tab: WorkbenchTabId,
    pub(super) panel: WorkbenchPanelId,
    pub(super) target: WorkbenchPanelTerminalTarget,
}
fn node<'a>(root: &'a PanelNode, id: &WorkbenchPanelId) -> Option<&'a PanelNode> {
    match root {
        PanelNode::Panel { panel_id, .. } => (panel_id == id).then_some(root),
        PanelNode::Split { children, .. } => children.iter().find_map(|slot| node(&slot.node, id)),
    }
}
pub(super) fn resolve(
    snapshot: &Snapshot,
    panel: &Target,
    command: &PanelTerminalCreationCommand,
) -> Result<Resolved, Failure> {
    let (id, context, tab) = results::resolve_or_fail(
        results::panel_entries(snapshot)
            .into_iter()
            .map(|(id, context, tab)| (id.as_str(), "", (id, context, tab))),
        panel,
        Resource::Panel,
    )?;
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = node(&tab.root, id).ok_or_else(|| Failure::new(FailureKind::InternalFailure))?
    else {
        unreachable!()
    };
    let target = if command.member == Member::Child {
        if child_presentations.len() >= 16 {
            return Err(rejected(snapshot, "resource-limit"));
        }
        WorkbenchPanelTerminalTarget::Child {
            label: command.label.clone().map(Ok).unwrap_or_else(|| {
                WorkbenchLabel::parse(&(child_presentations.len() + 2).to_string())
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))
            })?,
        }
    } else {
        let member = match &command.member {
            Member::Primary => primary_presentation,
            Member::Existing(presentation) => std::iter::once(primary_presentation)
                .chain(child_presentations)
                .find(|member| &member.presentation_id == presentation)
                .ok_or_else(|| rejected(snapshot, "panel-unavailable"))?,
            Member::Child => unreachable!(),
        };
        if member.binding.is_some() {
            return Err(rejected(snapshot, "panel-unavailable"));
        }
        WorkbenchPanelTerminalTarget::Unbound {
            presentation_id: member.presentation_id.clone(),
        }
    };
    Ok(Resolved {
        context: context.terminal_context_id.clone(),
        tab: tab.tab_id.clone(),
        panel: id.clone(),
        target,
    })
}

pub(super) fn presentation_exists(
    snapshot: &Snapshot,
    id: &orchbee_workbench_protocol::TerminalPresentationId,
) -> bool {
    fn contains(node: &PanelNode, id: &orchbee_workbench_protocol::TerminalPresentationId) -> bool {
        match node {
            PanelNode::Panel {
                primary_presentation,
                child_presentations,
                ..
            } => std::iter::once(primary_presentation)
                .chain(child_presentations)
                .any(|p| &p.presentation_id == id),
            PanelNode::Split { children, .. } => children.iter().any(|c| contains(&c.node, id)),
        }
    }
    snapshot
        .terminal_contexts
        .iter()
        .flat_map(|c| &c.tabs)
        .any(|tab| contains(&tab.root, id))
}

fn rejected(snapshot: &Snapshot, reason: &'static str) -> Failure {
    Failure::new(FailureKind::Rejected)
        .details(serde_json::Map::from_iter([
            ("stage".to_owned(), serde_json::json!("command")),
            ("reason".to_owned(), serde_json::json!(reason)),
            ("revision".to_owned(), serde_json::json!(snapshot.revision)),
        ]))
        .source(results::snapshot_source(snapshot))
}
