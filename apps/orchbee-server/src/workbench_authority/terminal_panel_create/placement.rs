//! Canonical placement of a newly reserved terminal, independent of dispatch and settlement.
use super::{
    PanelSplitReplacement, PlanFailure, allocate, count_panel_nodes, identity, replace_panel,
    split_ids,
};
use orchbee_workbench_protocol::{
    MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL, MAXIMUM_PANEL_NODES_PER_CONTEXT, PanelNode,
    Presentation, PresentationBinding, TerminalContext, TerminalPresentationId, WorkbenchAction,
    WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchPanelSplitDirection,
    WorkbenchPanelTerminalTarget, WorkbenchRejectionReason, WorkbenchSplitId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum TerminalPlacement {
    Split {
        new_panel_id: WorkbenchPanelId,
        split_id: WorkbenchSplitId,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
        direction: WorkbenchPanelSplitDirection,
    },
    Child,
    Unbound,
}

pub(super) struct PreparedPlacement {
    pub(super) placement: TerminalPlacement,
    pub(super) presentation_id: TerminalPresentationId,
    pub(super) last_identity: u128,
}

impl TerminalPlacement {
    pub(super) fn terminal_panel<'a>(
        &'a self,
        existing: &'a WorkbenchPanelId,
    ) -> &'a WorkbenchPanelId {
        match self {
            Self::Split { new_panel_id, .. } => new_panel_id,
            Self::Child | Self::Unbound => existing,
        }
    }

    pub(super) const fn direction(&self) -> Option<WorkbenchPanelSplitDirection> {
        match self {
            Self::Split { direction, .. } => Some(*direction),
            Self::Child | Self::Unbound => None,
        }
    }

    pub(super) fn apply(
        &self,
        root: &mut PanelNode,
        panel_id: &WorkbenchPanelId,
        presentation_id: &TerminalPresentationId,
        action: &WorkbenchAction,
        binding: PresentationBinding,
    ) -> bool {
        match (self, action) {
            (
                Self::Split {
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    direction,
                },
                WorkbenchAction::PanelSplit {
                    label, right_click, ..
                },
            ) => replace_panel(
                root,
                &PanelSplitReplacement {
                    target_panel_id: panel_id,
                    direction: *direction,
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                    label: label.as_str(),
                    right_click: *right_click,
                    binding,
                },
            ),
            (
                Self::Child,
                WorkbenchAction::PanelCreateTerminal {
                    target: WorkbenchPanelTerminalTarget::Child { label },
                    ..
                },
            ) => {
                let Some(PanelNode::Panel {
                    child_presentations,
                    ..
                }) = find_panel_mut(root, panel_id)
                else {
                    return false;
                };
                if child_presentations.len() >= MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL {
                    return false;
                }
                child_presentations.push(Presentation {
                    presentation_id: presentation_id.clone(),
                    label: label.as_str().to_owned(),
                    custom_label: None,
                    binding: Some(binding),
                    launch_failure: None,
                });
                true
            }
            (
                Self::Unbound,
                WorkbenchAction::PanelCreateTerminal {
                    target:
                        WorkbenchPanelTerminalTarget::Unbound {
                            presentation_id: requested,
                        },
                    ..
                },
            ) if requested == presentation_id => {
                let Some(PanelNode::Panel {
                    primary_presentation,
                    child_presentations,
                    ..
                }) = find_panel_mut(root, panel_id)
                else {
                    return false;
                };
                let Some(member) = std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .find(|p| &p.presentation_id == presentation_id && p.binding.is_none())
                else {
                    return false;
                };
                member.binding = Some(binding);
                true
            }
            _ => false,
        }
    }
}

pub(super) fn prepare(
    context: &TerminalContext,
    root: &PanelNode,
    panel_id: &WorkbenchPanelId,
    action: &WorkbenchAction,
    last_identity: u128,
) -> Result<PreparedPlacement, PlanFailure> {
    let Some(PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    }) = find_panel(root, panel_id)
    else {
        return Err(PlanFailure::Rejected(
            WorkbenchRejectionReason::PanelUnavailable,
        ));
    };
    match action {
        WorkbenchAction::PanelSplit { direction, .. } => {
            if count_panel_nodes(context)
                .checked_add(2)
                .is_none_or(|count| count > MAXIMUM_PANEL_NODES_PER_CONTEXT)
            {
                return Err(PlanFailure::Rejected(
                    WorkbenchRejectionReason::ResourceLimit,
                ));
            }
            let last_identity = allocate(last_identity, 5)?;
            let ids = split_ids(last_identity).ok_or(PlanFailure::Internal)?;
            Ok(PreparedPlacement {
                placement: TerminalPlacement::Split {
                    new_panel_id: ids.new_panel_id,
                    split_id: ids.split_id,
                    first_slot_id: ids.first_slot_id,
                    second_slot_id: ids.second_slot_id,
                    direction: *direction,
                },
                presentation_id: ids.presentation_id,
                last_identity,
            })
        }
        WorkbenchAction::PanelCreateTerminal {
            target: WorkbenchPanelTerminalTarget::Child { .. },
            ..
        } => {
            if child_presentations.len() >= MAXIMUM_CHILD_PRESENTATIONS_PER_PANEL {
                return Err(PlanFailure::Rejected(
                    WorkbenchRejectionReason::ResourceLimit,
                ));
            }
            let last_identity = allocate(last_identity, 1)?;
            Ok(PreparedPlacement {
                placement: TerminalPlacement::Child,
                presentation_id: TerminalPresentationId::parse(&identity(last_identity))
                    .map_err(|_| PlanFailure::Internal)?,
                last_identity,
            })
        }
        WorkbenchAction::PanelCreateTerminal {
            target: WorkbenchPanelTerminalTarget::Unbound { presentation_id },
            ..
        } => {
            if !std::iter::once(primary_presentation)
                .chain(child_presentations)
                .any(|p| &p.presentation_id == presentation_id && p.binding.is_none())
            {
                return Err(PlanFailure::Rejected(
                    WorkbenchRejectionReason::PanelUnavailable,
                ));
            }
            Ok(PreparedPlacement {
                placement: TerminalPlacement::Unbound,
                presentation_id: presentation_id.clone(),
                last_identity,
            })
        }
        _ => Err(PlanFailure::Internal),
    }
}

fn find_panel<'a>(node: &'a PanelNode, id: &WorkbenchPanelId) -> Option<&'a PanelNode> {
    match node {
        PanelNode::Panel { panel_id, .. } => (panel_id == id).then_some(node),
        PanelNode::Split { children, .. } => {
            children.iter().find_map(|slot| find_panel(&slot.node, id))
        }
    }
}

fn find_panel_mut<'a>(node: &'a mut PanelNode, id: &WorkbenchPanelId) -> Option<&'a mut PanelNode> {
    match node {
        PanelNode::Panel { panel_id, .. } if panel_id == id => Some(node),
        PanelNode::Panel { .. } => None,
        PanelNode::Split { children, .. } => children
            .iter_mut()
            .find_map(|slot| find_panel_mut(&mut slot.node, id)),
    }
}
