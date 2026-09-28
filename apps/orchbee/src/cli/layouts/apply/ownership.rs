//! Exact-tree fences for workflow-owned resources; never absorb a concurrent edit.
use super::error;
use crate::cli::{Failure, FailureKind};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, PanelNode, Presentation, Snapshot, Tab, TerminalPresentationId,
    WorkbenchLayoutNode, WorkbenchLayoutPresentation, WorkbenchPanelId,
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
pub(super) fn member<'a>(
    node: &'a PanelNode,
    panel: &WorkbenchPanelId,
    index: usize,
) -> Option<&'a Presentation> {
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = leaf(node, panel)?
    else {
        return None;
    };
    if index == 0 {
        Some(primary_presentation)
    } else {
        child_presentations.get(index - 1)
    }
}

/// Prove the complete identity-free skeleton before claiming ownership of any allocated IDs.
pub(super) fn materialized(
    plan: &super::super::template::Plan,
    root: &PanelNode,
) -> Result<(Vec<WorkbenchPanelId>, Vec<ClientSplitIdentity>), Failure> {
    fn presentation(expected: &WorkbenchLayoutPresentation, actual: &Presentation) -> bool {
        actual.binding.is_none()
            && actual.label == expected.label.as_str()
            && actual.custom_label == expected.custom_label
    }
    fn matches(expected: &WorkbenchLayoutNode, actual: &PanelNode) -> bool {
        match (expected, actual) {
            (
                WorkbenchLayoutNode::Panel {
                    primary,
                    children,
                    right_click,
                },
                PanelNode::Panel {
                    primary_presentation,
                    child_presentations,
                    right_click: actual_policy,
                    ..
                },
            ) => {
                right_click == actual_policy
                    && presentation(primary, primary_presentation)
                    && children.len() == child_presentations.len()
                    && children
                        .iter()
                        .zip(child_presentations)
                        .all(|(e, a)| presentation(e, a))
            }
            (
                WorkbenchLayoutNode::Split {
                    orientation,
                    children,
                },
                PanelNode::Split {
                    orientation: actual_orientation,
                    children: actual_children,
                    ..
                },
            ) => {
                orientation == actual_orientation
                    && children
                        .iter()
                        .zip(actual_children.iter())
                        .all(|(e, a)| matches(e, &a.node))
            }
            _ => false,
        }
    }
    fn first(node: &PanelNode) -> &WorkbenchPanelId {
        match node {
            PanelNode::Panel { panel_id, .. } => panel_id,
            PanelNode::Split { children, .. } => first(&children[0].node),
        }
    }
    if !matches(plan.layout.root(), root) {
        return Err(changed());
    }
    let mut panels = vec![first(root).clone()];
    let mut splits = Vec::new();
    for split in &plan.splits {
        let mut node = root;
        for index in split.path.indices() {
            let PanelNode::Split { children, .. } = node else {
                return Err(changed());
            };
            node = &children[usize::from(*index)].node;
        }
        let PanelNode::Split {
            split_id, children, ..
        } = node
        else {
            return Err(changed());
        };
        if split.created != panels.len() || first(&children[0].node) != &panels[split.source] {
            return Err(changed());
        }
        panels.push(first(&children[1].node).clone());
        splits.push(ClientSplitIdentity {
            split_id: split_id.clone(),
            first_slot_id: children[0].slot_id.clone(),
            second_slot_id: children[1].slot_id.clone(),
        });
    }
    Ok((panels, splits))
}

/// A launch may change only the acknowledged member's None binding into its first real binding.
pub(super) fn bind(
    expected: &Tab,
    fresh: &Tab,
    panel: &WorkbenchPanelId,
    presentation: &TerminalPresentationId,
) -> Result<Tab, Failure> {
    fn update(
        node: &mut PanelNode,
        fresh: &PanelNode,
        panel: &WorkbenchPanelId,
        id: &TerminalPresentationId,
    ) -> Result<(), Failure> {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                ..
            } if panel_id == panel => {
                let before = std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .find(|p| &p.presentation_id == id)
                    .ok_or_else(changed)?;
                let PanelNode::Panel {
                    primary_presentation,
                    child_presentations,
                    ..
                } = leaf(fresh, panel).ok_or_else(changed)?
                else {
                    return Err(changed());
                };
                let after = std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .find(|p| &p.presentation_id == id)
                    .ok_or_else(changed)?;
                if before.binding.is_some() || after.binding.is_none() {
                    return Err(changed());
                }
                before.binding = after.binding.clone();
                Ok(())
            }
            PanelNode::Split { children, .. } => {
                let child = children
                    .iter_mut()
                    .find(|slot| leaf(&slot.node, panel).is_some())
                    .ok_or_else(changed)?;
                update(&mut child.node, fresh, panel, id)
            }
            _ => Err(changed()),
        }
    }
    let mut candidate = expected.clone();
    update(&mut candidate.root, &fresh.root, panel, presentation)?;
    if &candidate != fresh {
        return Err(changed());
    }
    Ok(candidate)
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
    fn member(expected: &super::super::template::Member, actual: &serde_json::Value) -> bool {
        actual["name"].as_str() == Some(expected.name.as_str())
            && actual["start"].as_bool() == Some(expected.start)
            && actual["label"].as_str() == expected.label.custom_label().map(|label| label.as_str())
    }
    let mut observed = Vec::new();
    leaves(root, &mut observed);
    observed.len() == panels.len()
        && panels.len() == plan.leaves.len()
        && panels.iter().zip(&plan.leaves).all(|(panel, expected)| {
            let nodes: Vec<_> = observed
                .iter()
                .filter(|node| node["panelId"] == panel.as_str())
                .collect();
            let [actual] = nodes.as_slice() else {
                return false;
            };
            actual["rightClick"].as_str() == Some(expected.right_click.as_str())
                && member(&expected.primary, &actual["primary"])
                && actual["children"].as_array().is_some_and(|children| {
                    children.len() == expected.children.len()
                        && children
                            .iter()
                            .zip(&expected.children)
                            .all(|(a, e)| member(e, a))
                })
        })
}
#[cfg(test)]
mod tests;
