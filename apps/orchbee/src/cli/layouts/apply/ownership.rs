//! Exact-tree fences for workflow-owned resources; never absorb a concurrent edit.
use super::error;
use crate::cli::{Failure, FailureKind};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, PanelNode, PanelOrientation, Snapshot, Tab, WorkbenchPanelId,
    WorkbenchPanelSplitDirection, WorkbenchTabId, WorkbenchTerminalContextId,
};

pub(super) fn tab<'a>(
    snapshot: &'a Snapshot,
    context: &WorkbenchTerminalContextId,
    id: &WorkbenchTabId,
) -> Result<&'a Tab, Failure> {
    snapshot
        .terminal_contexts
        .iter()
        .find(|entry| &entry.terminal_context_id == context)
        .and_then(|entry| entry.tabs.iter().find(|tab| &tab.tab_id == id))
        .ok_or_else(changed)
}
pub(super) fn unchanged(
    snapshot: &Snapshot,
    context: &WorkbenchTerminalContextId,
    expected: &Tab,
) -> Result<(), Failure> {
    if tab(snapshot, context, &expected.tab_id)? == expected {
        Ok(())
    } else {
        Err(changed())
    }
}
pub(super) fn changed() -> Failure {
    error(FailureKind::Denied, "layout-changed")
}
pub(super) fn leaf<'a>(node: &'a PanelNode, id: &WorkbenchPanelId) -> Option<&'a PanelNode> {
    match node {
        PanelNode::Panel { panel_id, .. } => (panel_id == id).then_some(node),
        PanelNode::Split { children, .. } => children.iter().find_map(|slot| leaf(&slot.node, id)),
    }
}
pub(super) fn validate_leaf(
    node: &PanelNode,
    presentation: &str,
    panel: &WorkbenchPanelId,
) -> Result<(), Failure> {
    match node {
        PanelNode::Panel {
            right_click,
            child_presentations,
            panel_id,
            primary_presentation,
        } if right_click.is_default()
            && child_presentations.is_empty()
            && panel_id == panel
            && primary_presentation.custom_label.is_none()
            && primary_presentation.binding.is_some()
            && primary_presentation.presentation_id.as_str() == presentation =>
        {
            Ok(())
        }
        _ => Err(changed()),
    }
}

pub(super) fn matches_template(
    plan: &super::super::template::Plan,
    panels: &[WorkbenchPanelId],
    root: &serde_json::Value,
) -> bool {
    for split in &plan.splits {
        let mut node = root;
        for index in split.path.indices() {
            node = &node[if *index == 0 { "first" } else { "second" }];
        }
        if node["type"] != "split"
            || node["ratio"].as_f64() != Some(split.ratio.value())
            || node["direction"]
                != match split.direction {
                    WorkbenchPanelSplitDirection::Right => "right",
                    WorkbenchPanelSplitDirection::Below => "down",
                    _ => return false,
                }
        {
            return false;
        }
    }
    fn leaves<'a>(node: &'a serde_json::Value, result: &mut Vec<&'a serde_json::Value>) {
        if node["type"] == "split" {
            leaves(&node["first"], result);
            leaves(&node["second"], result);
        } else {
            result.push(node);
        }
    }
    let mut observed = Vec::new();
    leaves(root, &mut observed);
    observed.len() == panels.len()
        && panels.len() == plan.leaves.len()
        && panels.iter().zip(&plan.leaves).all(|(panel, leaf)| {
            observed
                .iter()
                .filter(|node| node["panelId"] == panel.as_str())
                .count()
                == 1
                && observed.iter().any(|node| {
                    node["panelId"] == panel.as_str()
                        && node["label"].as_str()
                            == leaf.label.custom_label().map(|label| label.as_str())
                })
        })
}

#[cfg(test)]
mod tests;
pub(super) fn graft(
    expected: &mut PanelNode,
    fresh: &PanelNode,
    source: &WorkbenchPanelId,
    new: &WorkbenchPanelId,
    identity: &ClientSplitIdentity,
    direction: WorkbenchPanelSplitDirection,
    presentation: &str,
) -> Result<(), Failure> {
    match expected {
        PanelNode::Panel { panel_id, .. } if panel_id == source => {
            let PanelNode::Split {
                split_id,
                orientation,
                children,
            } = fresh
            else {
                return Err(changed());
            };
            let expected_orientation = match direction {
                WorkbenchPanelSplitDirection::Right => PanelOrientation::Horizontal,
                WorkbenchPanelSplitDirection::Below => PanelOrientation::Vertical,
                _ => return Err(changed()),
            };
            if split_id != &identity.split_id
                || *orientation != expected_orientation
                || children[0].slot_id != identity.first_slot_id
                || children[1].slot_id != identity.second_slot_id
                || children[0].node != *expected
            {
                return Err(changed());
            }
            validate_leaf(&children[1].node, presentation, new)?;
            *expected = fresh.clone();
            Ok(())
        }
        PanelNode::Split { children, .. } => {
            let PanelNode::Split {
                children: observed, ..
            } = fresh
            else {
                return Err(changed());
            };
            for index in 0..2 {
                if leaf(&children[index].node, source).is_some() {
                    return graft(
                        &mut children[index].node,
                        &observed[index].node,
                        source,
                        new,
                        identity,
                        direction,
                        presentation,
                    );
                }
            }
            Err(changed())
        }
        _ => Err(changed()),
    }
}
pub(super) fn label(
    node: &mut PanelNode,
    panel: &WorkbenchPanelId,
    value: &orchbee_workbench_protocol::PanelLabelUpdate,
) -> Result<(), Failure> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } if panel_id == panel => {
            primary_presentation.custom_label = value.custom_label().cloned();
            Ok(())
        }
        PanelNode::Split { children, .. } => {
            for child in children.iter_mut() {
                if leaf(&child.node, panel).is_some() {
                    return label(&mut child.node, panel, value);
                }
            }
            Err(changed())
        }
        _ => Err(changed()),
    }
}
