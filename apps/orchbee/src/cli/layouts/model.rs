//! Pure portable tree projection; canonical identities fence all observed ratios and leaf facts.
use orchbee_client_server_protocol::LaunchSelection;
use orchbee_workbench_protocol::{
    ClientSplitArea, PanelNode, PanelOrientation, Presentation, WorkbenchPanelId,
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
                if !child_presentations.is_empty() || primary_presentation.binding.is_none() {
                    return Err("unsupported-presentation");
                }
                if out.len() == MAX_PANELS {
                    return Err("layout-too-large");
                }
                out.push((panel_id, primary_presentation));
            }
            PanelNode::Split { children, .. } => {
                for child in children.iter() {
                    walk(&child.node, depth + 1, out)?;
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, 1, &mut out)?;
    Ok(out)
}

pub(super) fn tree(
    root: &PanelNode,
    splits: &[ClientSplitArea],
    facts: &BTreeMap<WorkbenchPanelId, Leaf>,
) -> Result<Value, &'static str> {
    let panels = leaves(root)?;
    if panels.len() != facts.len()
        || panels.len() != splits.len() + 1
        || panels.iter().any(|(id, _)| !facts.contains_key(*id))
    {
        return Err("layout-changed");
    }
    let mut used = BTreeSet::new();
    fn walk(
        node: &PanelNode,
        splits: &[ClientSplitArea],
        facts: &BTreeMap<WorkbenchPanelId, Leaf>,
        used: &mut BTreeSet<String>,
    ) -> Result<Value, &'static str> {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                ..
            } => {
                let leaf = facts.get(panel_id).ok_or("layout-changed")?;
                let mut value = json!({"type":"panel","panelId":panel_id,"env":{}});
                if let Some(label) = &primary_presentation.custom_label {
                    value["label"] = json!(label);
                }
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
                Ok(value)
            }
            PanelNode::Split {
                split_id,
                orientation,
                children,
            } => {
                let observed = splits
                    .iter()
                    .find(|split| split.split_id == *split_id)
                    .ok_or("layout-changed")?;
                if !used.insert(split_id.to_string())
                    || observed.orientation != *orientation
                    || observed.first_slot_id != children[0].slot_id
                    || observed.second_slot_id != children[1].slot_id
                {
                    return Err("layout-changed");
                }
                Ok(json!({"type":"split","direction":match orientation {
                    PanelOrientation::Horizontal => "right", PanelOrientation::Vertical => "down",
                },"ratio":observed.ratio,"first":walk(&children[0].node,splits,facts,used)?,
                    "second":walk(&children[1].node,splits,facts,used)?}))
            }
        }
    }
    let result = walk(root, splits, facts, &mut used)?;
    if used.len() != splits.len() {
        return Err("layout-changed");
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
