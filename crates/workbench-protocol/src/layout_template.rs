//! Identity-free structure admission for atomic unbound Tab materialization.
use crate::{
    ContractError, PanelCustomLabel, PanelOrientation, PanelRightClickTarget, WorkbenchLabel,
};
use serde::{Deserialize, Deserializer, Serialize, de};

pub const MAXIMUM_LAYOUT_PANELS: usize = 24;
pub const MAXIMUM_LAYOUT_DEPTH: usize = 16;
pub const MAXIMUM_LAYOUT_CHILDREN: usize = 16;
pub const MAXIMUM_ENCODED_LAYOUT_COMMAND_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchLayoutPresentation {
    pub label: WorkbenchLabel,
    #[serde(deserialize_with = "Option::deserialize")]
    pub custom_label: Option<PanelCustomLabel>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchLayoutNode {
    Panel {
        primary: WorkbenchLayoutPresentation,
        children: Vec<WorkbenchLayoutPresentation>,
        right_click: PanelRightClickTarget,
    },
    Split {
        orientation: PanelOrientation,
        children: Box<[Self; 2]>,
    },
}

/// Only this validated wrapper crosses the command boundary. Nodes contain no authority-bearing IDs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkbenchLayoutTemplate(WorkbenchLayoutNode);

impl WorkbenchLayoutTemplate {
    pub fn new(root: WorkbenchLayoutNode) -> Result<Self, ContractError> {
        fn visit(node: &WorkbenchLayoutNode, depth: usize, leaves: &mut usize) -> bool {
            if depth > MAXIMUM_LAYOUT_DEPTH {
                return false;
            }
            match node {
                WorkbenchLayoutNode::Panel { children, .. } => {
                    *leaves += 1;
                    children.len() <= MAXIMUM_LAYOUT_CHILDREN && *leaves <= MAXIMUM_LAYOUT_PANELS
                }
                WorkbenchLayoutNode::Split { children, .. } => {
                    children.iter().all(|node| visit(node, depth + 1, leaves))
                }
            }
        }
        if !visit(&root, 1, &mut 0) {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(root))
    }
    #[must_use]
    pub const fn root(&self) -> &WorkbenchLayoutNode {
        &self.0
    }
}

impl<'de> Deserialize<'de> for WorkbenchLayoutTemplate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(WorkbenchLayoutNode::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
