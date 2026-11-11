//! Retained graph fields are explicit even when the live wire representation omits defaults.

use orchbee_workbench_protocol::{
    PanelCustomLabel, PanelNode, PanelOrientation, PanelRightClickTarget, PanelSlot, Presentation,
    PresentationBinding, Tab, TabGroup, TerminalContext, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchSplitId, WorkbenchSplitRatio, WorkbenchTabGroupId,
    WorkbenchTabId, WorkbenchTerminalContextId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredContext {
    groups: Vec<TabGroup>,
    tabs: Vec<StoredTab>,
    terminal_context_id: WorkbenchTerminalContextId,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredTab {
    automatic_label: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    group_id: Option<WorkbenchTabGroupId>,
    label: String,
    pinned: bool,
    root: StoredPanel,
    tab_id: WorkbenchTabId,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredSlot {
    node: StoredPanel,
    slot_id: WorkbenchPanelSlotId,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum StoredPanel {
    Panel {
        right_click: PanelRightClickTarget,
        child_presentations: Vec<StoredPresentation>,
        panel_id: WorkbenchPanelId,
        primary_presentation: StoredPresentation,
    },
    Split {
        children: Box<[StoredSlot; 2]>,
        orientation: PanelOrientation,
        split_id: WorkbenchSplitId,
        ratio: WorkbenchSplitRatio,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredPresentation {
    #[serde(deserialize_with = "Option::deserialize")]
    binding: Option<PresentationBinding>,
    #[serde(deserialize_with = "Option::deserialize")]
    custom_label: Option<PanelCustomLabel>,
    label: String,
    presentation_id: TerminalPresentationId,
}

impl From<TerminalContext> for StoredContext {
    fn from(context: TerminalContext) -> Self {
        Self {
            groups: context.groups,
            tabs: context.tabs.into_iter().map(StoredTab::from).collect(),
            terminal_context_id: context.terminal_context_id,
        }
    }
}

impl From<StoredContext> for TerminalContext {
    fn from(context: StoredContext) -> Self {
        Self {
            groups: context.groups,
            tabs: context.tabs.into_iter().map(Tab::from).collect(),
            terminal_context_id: context.terminal_context_id,
        }
    }
}

impl From<Tab> for StoredTab {
    fn from(tab: Tab) -> Self {
        Self {
            automatic_label: tab.automatic_label,
            group_id: tab.group_id,
            label: tab.label,
            pinned: tab.pinned,
            root: tab.root.into(),
            tab_id: tab.tab_id,
        }
    }
}

impl From<StoredTab> for Tab {
    fn from(tab: StoredTab) -> Self {
        Self {
            automatic_label: tab.automatic_label,
            group_id: tab.group_id,
            label: tab.label,
            pinned: tab.pinned,
            root: tab.root.into(),
            tab_id: tab.tab_id,
        }
    }
}

impl From<PanelSlot> for StoredSlot {
    fn from(slot: PanelSlot) -> Self {
        Self {
            node: slot.node.into(),
            slot_id: slot.slot_id,
        }
    }
}

impl From<StoredSlot> for PanelSlot {
    fn from(slot: StoredSlot) -> Self {
        Self {
            node: slot.node.into(),
            slot_id: slot.slot_id,
        }
    }
}

impl From<PanelNode> for StoredPanel {
    fn from(node: PanelNode) -> Self {
        match node {
            PanelNode::Panel {
                right_click,
                child_presentations,
                panel_id,
                primary_presentation,
            } => Self::Panel {
                right_click,
                child_presentations: child_presentations
                    .into_iter()
                    .map(StoredPresentation::from)
                    .collect(),
                panel_id,
                primary_presentation: primary_presentation.into(),
            },
            PanelNode::Split {
                children,
                orientation,
                split_id,
                ratio,
            } => Self::Split {
                children: Box::new((*children).map(StoredSlot::from)),
                orientation,
                split_id,
                ratio,
            },
        }
    }
}

impl From<StoredPanel> for PanelNode {
    fn from(node: StoredPanel) -> Self {
        match node {
            StoredPanel::Panel {
                right_click,
                child_presentations,
                panel_id,
                primary_presentation,
            } => Self::Panel {
                right_click,
                child_presentations: child_presentations
                    .into_iter()
                    .map(Presentation::from)
                    .collect(),
                panel_id,
                primary_presentation: primary_presentation.into(),
            },
            StoredPanel::Split {
                children,
                orientation,
                split_id,
                ratio,
            } => Self::Split {
                children: Box::new((*children).map(PanelSlot::from)),
                orientation,
                split_id,
                ratio,
            },
        }
    }
}

impl From<Presentation> for StoredPresentation {
    fn from(presentation: Presentation) -> Self {
        Self {
            binding: presentation.binding,
            custom_label: presentation.custom_label,
            label: presentation.label,
            presentation_id: presentation.presentation_id,
        }
    }
}

impl From<StoredPresentation> for Presentation {
    fn from(presentation: StoredPresentation) -> Self {
        Self {
            binding: presentation.binding,
            custom_label: presentation.custom_label,
            label: presentation.label,
            presentation_id: presentation.presentation_id,
            launch_failure: None,
            agent_not_resumed: None,
            agent_start_failure: None,
            launch_program: None,
        }
    }
}
