//! Pure portable tree projection; canonical identities fence all observed ratios and leaf facts.
use orchbee_client_server_protocol::LaunchSelection;
use orchbee_workbench_protocol::{
    ClientSplitArea, PanelNode, PanelOrientation, Presentation, TerminalPresentationId,
    WorkbenchPanelId,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const MAX_PANELS: usize = 24;
pub(super) const MAX_DEPTH: usize = 16;
pub(super) struct Leaf {
    pub(super) cwd: Option<String>,
    pub(super) launch: Option<LaunchSelection>,
}

pub(super) fn leaves(
    root: &PanelNode,
) -> Result<Vec<(&WorkbenchPanelId, &Presentation)>, &'static str> {
    fn walk<'a>(
        node: &'a PanelNode,
        depth: usize,
        panels: &mut usize,
        out: &mut Vec<(&'a WorkbenchPanelId, &'a Presentation)>,
    ) -> Result<(), &'static str> {
        if depth > MAX_DEPTH {
            return Err("layout-too-large");
        }
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                ..
            } => {
                if child_presentations.len() > orchbee_workbench_protocol::MAXIMUM_LAYOUT_CHILDREN {
                    return Err("layout-too-large");
                }
                if *panels == MAX_PANELS {
                    return Err("layout-too-large");
                }
                *panels += 1;
                out.push((panel_id, primary_presentation));
                out.extend(child_presentations.iter().map(|member| (panel_id, member)));
            }
            PanelNode::Split { children, .. } => {
                for child in children.iter() {
                    walk(&child.node, depth + 1, panels, out)?;
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, 1, &mut 0, &mut out)?;
    Ok(out)
}

pub(super) fn tree(
    root: &PanelNode,
    splits: &[ClientSplitArea],
    facts: &BTreeMap<TerminalPresentationId, Leaf>,
) -> Result<Value, &'static str> {
    project(root, Some(splits), facts)
}

pub(super) fn shared_tree(
    root: &PanelNode,
    facts: &BTreeMap<TerminalPresentationId, Leaf>,
) -> Result<Value, &'static str> {
    project(root, None, facts)
}

fn project(
    root: &PanelNode,
    splits: Option<&[ClientSplitArea]>,
    facts: &BTreeMap<TerminalPresentationId, Leaf>,
) -> Result<Value, &'static str> {
    let panels = leaves(root)?;
    let panel_count = panels
        .iter()
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>()
        .len();
    let bound: Vec<_> = panels
        .iter()
        .filter(|(_, member)| member.binding.is_some())
        .collect();
    if bound.len() != facts.len()
        || splits.is_some_and(|splits| panel_count != splits.len() + 1)
        || bound
            .iter()
            .any(|(_, member)| !facts.contains_key(&member.presentation_id))
    {
        return Err("layout-changed");
    }
    let mut used = BTreeSet::new();
    fn walk(
        node: &PanelNode,
        splits: Option<&[ClientSplitArea]>,
        facts: &BTreeMap<TerminalPresentationId, Leaf>,
        used: &mut BTreeSet<String>,
    ) -> Result<Value, &'static str> {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                right_click,
            } => {
                fn member(
                    presentation: &Presentation,
                    facts: &BTreeMap<TerminalPresentationId, Leaf>,
                ) -> Result<Value, &'static str> {
                    let mut value = json!({"presentationId":presentation.presentation_id,"name":presentation.label,
                        "start":presentation.binding.is_some(),"env":{}});
                    if let Some(label) = &presentation.custom_label {
                        value["label"] = json!(label);
                    }
                    if presentation.binding.is_some() {
                        let leaf = facts
                            .get(&presentation.presentation_id)
                            .ok_or("layout-changed")?;
                        if let Some(cwd) = &leaf.cwd {
                            value["cwd"] = json!(cwd);
                        }
                        if let Some(launch) = &leaf.launch {
                            value["command"] = json!(
                                std::iter::once(launch.executable())
                                    .chain(launch.arguments().iter().map(String::as_str))
                                    .collect::<Vec<_>>()
                            );
                        }
                    }
                    Ok(value)
                }
                Ok(
                    json!({"type":"panel","panelId":panel_id,"rightClick":right_click,
                    "primary":member(primary_presentation,facts)?,
                    "children":child_presentations.iter().map(|p| member(p,facts)).collect::<Result<Vec<_>,_>>()?}),
                )
            }
            PanelNode::Split {
                split_id,
                orientation,
                children,
                ratio,
            } => {
                if !used.insert(split_id.to_string()) {
                    return Err("layout-changed");
                }
                if let Some(splits) = splits {
                    let observed = splits
                        .iter()
                        .find(|split| split.split_id == *split_id)
                        .ok_or("layout-changed")?;
                    if observed.orientation != *orientation
                        || observed.first_slot_id != children[0].slot_id
                        || observed.second_slot_id != children[1].slot_id
                        || observed.ratio != *ratio
                    {
                        return Err("layout-changed");
                    }
                }
                Ok(json!({"type":"split","direction":match orientation {
                    PanelOrientation::Horizontal => "right", PanelOrientation::Vertical => "down",
                },"ratio":ratio,"first":walk(&children[0].node,splits,facts,used)?,
                    "second":walk(&children[1].node,splits,facts,used)?}))
            }
        }
    }
    let result = walk(root, splits, facts, &mut used)?;
    if splits.is_some_and(|splits| used.len() != splits.len()) {
        return Err("layout-changed");
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
