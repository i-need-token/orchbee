//! Change one canonical proportion on a pre-commit candidate graph.
use super::{PlanFailure, WorkbenchRejectionReason as Reason};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, PanelNode, Snapshot, WorkbenchSplitId, WorkbenchSplitRatio,
    WorkbenchTabId, WorkbenchTerminalContextId,
};

pub(super) fn set(
    candidate: &mut Snapshot,
    context_id: &WorkbenchTerminalContextId,
    tab_id: &WorkbenchTabId,
    identity: &ClientSplitIdentity,
    previous: WorkbenchSplitRatio,
    requested: WorkbenchSplitRatio,
) -> Result<bool, PlanFailure> {
    let context = candidate
        .terminal_contexts
        .iter_mut()
        .find(|context| &context.terminal_context_id == context_id)
        .ok_or(PlanFailure::Rejected(Reason::TerminalContextUnavailable))?;
    let tab = context
        .tabs
        .iter_mut()
        .find(|tab| &tab.tab_id == tab_id)
        .ok_or(PlanFailure::Rejected(Reason::TabUnavailable))?;
    let PanelNode::Split {
        children, ratio, ..
    } = find(&mut tab.root, &identity.split_id)
        .ok_or(PlanFailure::Rejected(Reason::SplitUnavailable))?
    else {
        return Err(PlanFailure::Internal);
    };
    if children[0].slot_id != identity.first_slot_id
        || children[1].slot_id != identity.second_slot_id
    {
        return Err(PlanFailure::Rejected(Reason::SplitUnavailable));
    }
    if *ratio != previous {
        return Err(PlanFailure::Rejected(Reason::RevisionMismatch));
    }
    let changed = *ratio != requested;
    *ratio = requested;
    Ok(changed)
}

fn find<'a>(node: &'a mut PanelNode, id: &WorkbenchSplitId) -> Option<&'a mut PanelNode> {
    if matches!(node, PanelNode::Split { split_id, .. } if split_id == id) {
        return Some(node);
    }
    let PanelNode::Split { children, .. } = node else {
        return None;
    };
    children
        .iter_mut()
        .find_map(|child| find(&mut child.node, id))
}
