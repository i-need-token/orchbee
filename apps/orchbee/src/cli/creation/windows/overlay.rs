//! Overlay return identity is pinned before creation and never reselected by ordinal.
use super::{
    ClientViewIntent, ClientViewRecord, Created, Failure, FailureKind, PanelNode, Snapshot,
    WorkbenchPanelId, internal,
};
use orchbee_workbench_protocol::{ClientOverlayMember, Presentation};

pub(super) fn capture_return(
    snapshot: &Snapshot,
    selected: &ClientViewRecord,
    chosen: &orchbee_workbench_protocol::ClientPresentationSelection,
) -> Result<ClientOverlayMember, Failure> {
    let selection = selected.view.selection.as_ref().ok_or_else(internal)?;
    let panel_id = selection.panel_id.as_ref().ok_or_else(internal)?;
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == selection.terminal_context_id)
        .and_then(|context| {
            context
                .tabs
                .iter()
                .find(|tab| Some(&tab.tab_id) == selection.tab_id.as_ref())
        })
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    let presentation = member(&tab.root, panel_id, chosen.presentation_id())
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    if presentation.binding.as_ref() != chosen.binding() {
        return Err(Failure::new(FailureKind::Denied));
    }
    Ok(ClientOverlayMember {
        panel_id: panel_id.clone(),
        presentation_id: presentation.presentation_id.clone(),
        binding: presentation.binding.clone(),
    })
}

pub(super) fn intent(
    snapshot: &Snapshot,
    created: &Created,
    return_to: ClientOverlayMember,
    selected: &ClientViewRecord,
) -> Result<ClientViewIntent, Failure> {
    let selection = selected.view.selection.as_ref().ok_or_else(internal)?;
    if !created.split
        || created.context != selection.terminal_context_id
        || Some(&created.tab) != selection.tab_id.as_ref()
    {
        return Err(Failure::new(FailureKind::Denied));
    }
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == created.context)
        .and_then(|context| context.tabs.iter().find(|tab| tab.tab_id == created.tab))
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    let presentation =
        primary(&tab.root, &created.panel).ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    let binding = presentation
        .binding
        .clone()
        .ok_or_else(|| Failure::new(FailureKind::ServiceUnavailable))?;
    let previous = member(&tab.root, &return_to.panel_id, &return_to.presentation_id)
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    if previous.presentation_id != return_to.presentation_id
        || previous.binding != return_to.binding
    {
        return Err(Failure::new(FailureKind::Denied));
    }
    Ok(ClientViewIntent::Overlay {
        terminal_context_id: created.context.clone(),
        tab_id: created.tab.clone(),
        panel_id: created.panel.clone(),
        presentation_id: presentation.presentation_id.clone(),
        expected_binding: binding,
        return_to,
        previously_zoomed: selection.zoomed,
    })
}

fn primary<'a>(node: &'a PanelNode, id: &WorkbenchPanelId) -> Option<&'a Presentation> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } => (panel_id == id).then_some(primary_presentation),
        PanelNode::Split { children, .. } => {
            children.iter().find_map(|child| primary(&child.node, id))
        }
    }
}

fn member<'a>(
    node: &'a PanelNode,
    panel: &WorkbenchPanelId,
    presentation: &orchbee_workbench_protocol::TerminalPresentationId,
) -> Option<&'a Presentation> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } if panel_id == panel => std::iter::once(primary_presentation)
            .chain(child_presentations.iter())
            .find(|p| &p.presentation_id == presentation),
        PanelNode::Panel { .. } => None,
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|child| member(&child.node, panel, presentation)),
    }
}
