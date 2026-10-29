//! Atomic placement of one fresh terminal Tab. The authority owns IDs, runtime effects and commit;
//! receipt consumers replay this pure transition against the captured canonical context.
use crate::{
    MAXIMUM_PANEL_NODES_PER_CONTEXT, MAXIMUM_TAB_GROUPS_PER_CONTEXT, MAXIMUM_TABS_PER_CONTEXT,
    PanelNode, Presentation, PresentationBinding, PresentationLaunchProgram, Tab, TabGroup,
    TabGroupColor, TerminalContext, TerminalPresentationId, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchRejectionReason, WorkbenchTabGroupId, WorkbenchTabId,
};
use serde::{Deserialize, Serialize};

/// Omission appends an unpinned, ungrouped Tab. Duplication copies metadata, never a live tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TabCreationPlacement {
    AfterTab { tab_id: WorkbenchTabId },
    InGroup { group_id: WorkbenchTabGroupId },
    NewGroup {},
    Duplicate { tab_id: WorkbenchTabId },
}

/// Fresh identities, the ordinary launch label/binding, and the launched program committed with
/// that binding, all supplied by the creation owner.
pub struct NewTerminalTab {
    pub tab_id: WorkbenchTabId,
    pub panel_id: WorkbenchPanelId,
    pub presentation_id: TerminalPresentationId,
    pub label: WorkbenchLabel,
    pub automatic_label: bool,
    pub binding: PresentationBinding,
    pub launch_program: Option<PresentationLaunchProgram>,
}

/// Reject placement and structural capacity before reserving any runtime effect.
/// # Errors
/// Missing exact group/source or exhausted Tab, group or Panel capacity refuses creation.
pub fn validate_tab_creation(
    context: &TerminalContext,
    placement: Option<&TabCreationPlacement>,
) -> Result<(), WorkbenchRejectionReason> {
    use WorkbenchRejectionReason as R;
    fn nodes(node: &PanelNode) -> usize {
        match node {
            PanelNode::Panel { .. } => 1,
            PanelNode::Split { children, .. } => {
                1 + children
                    .iter()
                    .map(|child| nodes(&child.node))
                    .sum::<usize>()
            }
        }
    }
    if context.tabs.len() >= MAXIMUM_TABS_PER_CONTEXT
        || context
            .tabs
            .iter()
            .map(|tab| nodes(&tab.root))
            .sum::<usize>()
            >= MAXIMUM_PANEL_NODES_PER_CONTEXT
    {
        return Err(R::ResourceLimit);
    }
    match placement {
        Some(TabCreationPlacement::InGroup { group_id })
            if !context
                .groups
                .iter()
                .any(|group| &group.group_id == group_id) =>
        {
            Err(R::TabGroupUnavailable)
        }
        Some(
            TabCreationPlacement::Duplicate { tab_id } | TabCreationPlacement::AfterTab { tab_id },
        ) if !context.tabs.iter().any(|tab| &tab.tab_id == tab_id) => Err(R::TabUnavailable),
        Some(TabCreationPlacement::AfterTab { tab_id })
            if context
                .tabs
                .iter()
                .position(|tab| &tab.tab_id == tab_id)
                .is_some_and(|index| {
                    index + 1 < context.tabs.iter().take_while(|tab| tab.pinned).count()
                }) =>
        {
            Err(R::TabMoveOrderConflict)
        }
        Some(TabCreationPlacement::NewGroup {})
            if context
                .groups
                .iter()
                .filter(|group| {
                    context
                        .tabs
                        .iter()
                        .any(|tab| tab.group_id.as_ref() == Some(&group.group_id))
                })
                .count()
                >= MAXIMUM_TAB_GROUPS_PER_CONTEXT =>
        {
            Err(R::ResourceLimit)
        }
        Some(
            TabCreationPlacement::InGroup { .. }
            | TabCreationPlacement::Duplicate { .. }
            | TabCreationPlacement::AfterTab { .. }
            | TabCreationPlacement::NewGroup {},
        )
        | None => Ok(()),
    }
}

/// Returns the complete candidate without modifying the input. `created_group` is required exactly
/// for NewGroup and must be fresh Session-wide (the caller checks other contexts).
/// # Errors
/// Rejects invalid placement, capacity or a reused allocated Tab/group identity.
pub fn create_terminal_tab(
    context: &TerminalContext,
    placement: Option<&TabCreationPlacement>,
    fresh: NewTerminalTab,
    created_group: Option<&WorkbenchTabGroupId>,
) -> Result<TerminalContext, WorkbenchRejectionReason> {
    use WorkbenchRejectionReason as R;
    validate_tab_creation(context, placement)?;
    if context.tabs.iter().any(|tab| tab.tab_id == fresh.tab_id) {
        return Err(R::TabUnavailable);
    }
    if matches!(placement, Some(TabCreationPlacement::NewGroup {})) != created_group.is_some()
        || created_group.is_some_and(|id| context.groups.iter().any(|group| &group.group_id == id))
    {
        return Err(R::TabGroupUnavailable);
    }
    let mut label = fresh.label.as_str().to_owned();
    let mut automatic_label = fresh.automatic_label;
    let mut pinned = false;
    let mut group_id = None;
    let index = match placement {
        None => context.tabs.len(),
        Some(TabCreationPlacement::AfterTab { tab_id }) => {
            let index = context
                .tabs
                .iter()
                .position(|tab| &tab.tab_id == tab_id)
                .ok_or(R::TabUnavailable)?;
            group_id.clone_from(&context.tabs[index].group_id);
            index + 1
        }
        Some(TabCreationPlacement::InGroup { group_id: id }) => {
            group_id = Some(id.clone());
            context
                .tabs
                .iter()
                .rposition(|tab| tab.group_id.as_ref() == Some(id))
                .map_or(context.tabs.len(), |index| index + 1)
        }
        Some(TabCreationPlacement::NewGroup {}) => {
            group_id = created_group.cloned();
            context.tabs.len()
        }
        Some(TabCreationPlacement::Duplicate { tab_id }) => {
            let index = context
                .tabs
                .iter()
                .position(|tab| &tab.tab_id == tab_id)
                .ok_or(R::TabUnavailable)?;
            let source = &context.tabs[index];
            label.clone_from(&source.label);
            automatic_label = source.automatic_label;
            pinned = source.pinned;
            group_id.clone_from(&source.group_id);
            index + 1
        }
    };
    let mut candidate = context.clone();
    if let Some(group_id) = created_group {
        candidate.groups.push(TabGroup {
            group_id: group_id.clone(),
            label: String::new(),
            color: TabGroupColor::Grey,
        });
    }
    candidate.tabs.insert(
        index,
        Tab {
            tab_id: fresh.tab_id,
            label: label.clone(),
            automatic_label,
            pinned,
            group_id,
            root: PanelNode::Panel {
                right_click: Default::default(),
                panel_id: fresh.panel_id,
                child_presentations: Vec::new(),
                primary_presentation: Presentation {
                    presentation_id: fresh.presentation_id,
                    label,
                    custom_label: None,
                    binding: Some(fresh.binding),
                    launch_failure: None,
                    launch_program: fresh.launch_program,
                },
            },
        },
    );
    // GUI createNewTabGroup delegates to createGroup, whose commit prunes empty groups.
    if created_group.is_some() {
        candidate.groups.retain(|group| {
            candidate
                .tabs
                .iter()
                .any(|tab| tab.group_id.as_ref() == Some(&group.group_id))
        });
    }
    Ok(candidate)
}
