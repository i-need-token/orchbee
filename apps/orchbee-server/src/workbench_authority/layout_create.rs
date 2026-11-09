//! Build an entirely unbound tree inside the existing atomic navigation transaction.
use super::{PlanFailure, allocate, identity};
use orchbee_workbench_protocol::{
    MAXIMUM_PANEL_NODES_PER_CONTEXT, MAXIMUM_TABS_PER_CONTEXT, PanelNode, PanelSlot, Presentation,
    Tab, TerminalContext, TerminalPresentationId, WorkbenchLabel, WorkbenchLayoutNode,
    WorkbenchLayoutPresentation, WorkbenchLayoutTemplate, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchRejectionReason, WorkbenchSplitId, WorkbenchTabId,
};

pub(super) fn materialize(
    context: &mut TerminalContext,
    layout: &WorkbenchLayoutTemplate,
    label: &WorkbenchLabel,
    automatic_label: bool,
    cursor: &mut u128,
) -> Result<WorkbenchTabId, PlanFailure> {
    fn nodes(node: &PanelNode) -> usize {
        match node {
            PanelNode::Panel { .. } => 1,
            PanelNode::Split { children, .. } => {
                1 + children.iter().map(|slot| nodes(&slot.node)).sum::<usize>()
            }
        }
    }
    fn required(node: &WorkbenchLayoutNode) -> (usize, u128) {
        match node {
            WorkbenchLayoutNode::Panel { children, .. } => (1, 2 + children.len() as u128),
            WorkbenchLayoutNode::Split { children, .. } => {
                let (an, ai) = required(&children[0]);
                let (bn, bi) = required(&children[1]);
                (1 + an + bn, 3 + ai + bi)
            }
        }
    }
    let (count, ids) = required(layout.root());
    if context.tabs.len() >= MAXIMUM_TABS_PER_CONTEXT
        || context
            .tabs
            .iter()
            .map(|tab| nodes(&tab.root))
            .sum::<usize>()
            + count
            > MAXIMUM_PANEL_NODES_PER_CONTEXT
    {
        return Err(PlanFailure::Rejected(
            WorkbenchRejectionReason::ResourceLimit,
        ));
    }
    // Check the entire identity budget before constructing any candidate identity.
    let end = allocate(*cursor, ids + 1)?;
    let tab_id = WorkbenchTabId::parse(&take(cursor)).map_err(|_| PlanFailure::Internal)?;
    let root = tree(layout.root(), cursor)?;
    if *cursor != end {
        return Err(PlanFailure::Internal);
    }
    context.tabs.push(Tab {
        automatic_label,
        group_id: None,
        pinned: false,
        label: label.as_str().to_owned(),
        root,
        tab_id: tab_id.clone(),
    });
    Ok(tab_id)
}

fn take(cursor: &mut u128) -> String {
    *cursor += 1;
    identity(*cursor)
}
fn member(
    template: &WorkbenchLayoutPresentation,
    cursor: &mut u128,
) -> Result<Presentation, PlanFailure> {
    Ok(Presentation {
        binding: None,
        label: template.label.as_str().to_owned(),
        custom_label: template.custom_label.clone(),
        presentation_id: TerminalPresentationId::parse(&take(cursor))
            .map_err(|_| PlanFailure::Internal)?,
        launch_failure: None,
        agent_not_resumed: None,
        launch_program: None,
    })
}
fn tree(node: &WorkbenchLayoutNode, cursor: &mut u128) -> Result<PanelNode, PlanFailure> {
    Ok(match node {
        WorkbenchLayoutNode::Panel {
            primary,
            children,
            right_click,
        } => PanelNode::Panel {
            panel_id: WorkbenchPanelId::parse(&take(cursor)).map_err(|_| PlanFailure::Internal)?,
            primary_presentation: member(primary, cursor)?,
            child_presentations: children
                .iter()
                .map(|child| member(child, cursor))
                .collect::<Result<_, _>>()?,
            right_click: *right_click,
        },
        WorkbenchLayoutNode::Split {
            orientation,
            children,
        } => {
            let split_id =
                WorkbenchSplitId::parse(&take(cursor)).map_err(|_| PlanFailure::Internal)?;
            let first =
                WorkbenchPanelSlotId::parse(&take(cursor)).map_err(|_| PlanFailure::Internal)?;
            let second =
                WorkbenchPanelSlotId::parse(&take(cursor)).map_err(|_| PlanFailure::Internal)?;
            PanelNode::Split {
                orientation: *orientation,
                split_id,
                ratio: Default::default(),
                children: Box::new([
                    PanelSlot {
                        slot_id: first,
                        node: tree(&children[0], cursor)?,
                    },
                    PanelSlot {
                        slot_id: second,
                        node: tree(&children[1], cursor)?,
                    },
                ]),
            }
        }
    })
}
