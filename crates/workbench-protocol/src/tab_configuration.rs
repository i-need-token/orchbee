//! Atomic Tab metadata and order transitions shared by the authority and receipt verification.
//! Callers own revision checks, identity allocation, persistence and publication; no terminal effects occur here.
use crate::{
    MAXIMUM_TAB_GROUPS_PER_CONTEXT, TabGroup, TabGroupColor, TerminalContext, WorkbenchLabel,
    WorkbenchRejectionReason, WorkbenchTabGroupId, WorkbenchTabId,
};
use serde::{Deserialize, Serialize};

/// One bounded configuration operation against an existing canonical context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TabConfiguration {
    CreateGroup {
        tab_id: WorkbenchTabId,
        label: WorkbenchLabel,
        color: TabGroupColor,
    },
    SetGroup {
        tab_id: WorkbenchTabId,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        group_id: Option<WorkbenchTabGroupId>,
    },
    UpdateGroup {
        group_id: WorkbenchTabGroupId,
        label: WorkbenchLabel,
        color: TabGroupColor,
    },
    Ungroup {
        group_id: WorkbenchTabGroupId,
    },
    SetPinned {
        tab_id: WorkbenchTabId,
        pinned: bool,
    },
    MoveGroup {
        group_id: WorkbenchTabGroupId,
        direction: TabGroupMoveDirection,
    },
}

/// Move past one adjacent group or ungrouped Tab, without wrapping or crossing pinned Tabs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabGroupMoveDirection {
    Previous,
    Next,
}

/// Returns the complete candidate, or `None` for an intentional no-op. The input is never mutated.
/// `created_group` must be a fresh authority-allocated identity exactly for `CreateGroup`.
/// # Errors
/// Rejects missing members, an incompatible allocated identity, and the group capacity bound.
pub fn configure_tabs(
    context: &TerminalContext,
    operation: &TabConfiguration,
    created_group: Option<&WorkbenchTabGroupId>,
) -> Result<Option<TerminalContext>, WorkbenchRejectionReason> {
    use TabConfiguration::*;
    use WorkbenchRejectionReason as R;
    if matches!(operation, CreateGroup { .. }) != created_group.is_some() {
        return Err(R::TabGroupUnavailable);
    }
    let mut candidate = context.clone();
    match operation {
        CreateGroup {
            tab_id,
            label,
            color,
        } => {
            let id = created_group.ok_or(R::TabGroupUnavailable)?;
            if candidate.groups.iter().any(|group| &group.group_id == id) {
                return Err(R::TabGroupUnavailable);
            }
            if candidate.groups.len() >= MAXIMUM_TAB_GROUPS_PER_CONTEXT {
                return Err(R::ResourceLimit);
            }
            candidate.groups.push(TabGroup {
                group_id: id.clone(),
                label: label.as_str().to_owned(),
                color: *color,
            });
            assign_group(&mut candidate, tab_id, Some(id))?;
        }
        SetGroup { tab_id, group_id } => assign_group(&mut candidate, tab_id, group_id.as_ref())?,
        UpdateGroup {
            group_id,
            label,
            color,
        } => {
            let group = candidate
                .groups
                .iter_mut()
                .find(|group| &group.group_id == group_id)
                .ok_or(R::TabGroupUnavailable)?;
            group.label = label.as_str().to_owned();
            group.color = *color;
        }
        Ungroup { group_id } => {
            require_group(&candidate, group_id)?;
            for tab in &mut candidate.tabs {
                if tab.group_id.as_ref() == Some(group_id) {
                    tab.group_id = None;
                }
            }
            candidate.groups.retain(|group| &group.group_id != group_id);
        }
        SetPinned { tab_id, pinned } => {
            let index = tab_index(&candidate, tab_id)?;
            if candidate.tabs[index].pinned != *pinned {
                let mut tab = candidate.tabs.remove(index);
                tab.pinned = *pinned;
                if *pinned {
                    tab.group_id = None;
                }
                let boundary = candidate.tabs.iter().take_while(|tab| tab.pinned).count();
                candidate.tabs.insert(boundary, tab);
                remove_empty_groups(&mut candidate);
            }
        }
        MoveGroup {
            group_id,
            direction,
        } => {
            require_group(&candidate, group_id)?;
            if let Some(start) = candidate
                .tabs
                .iter()
                .position(|tab| tab.group_id.as_ref() == Some(group_id))
            {
                let end = start
                    + candidate.tabs[start..]
                        .iter()
                        .take_while(|tab| tab.group_id.as_ref() == Some(group_id))
                        .count();
                let neighbor = match direction {
                    TabGroupMoveDirection::Previous => start.checked_sub(1),
                    TabGroupMoveDirection::Next => (end < candidate.tabs.len()).then_some(end),
                };
                if let Some(neighbor) = neighbor.filter(|index| !candidate.tabs[*index].pinned) {
                    let adjacent = candidate.tabs[neighbor].group_id.clone();
                    match direction {
                        TabGroupMoveDirection::Previous => {
                            let mut boundary = neighbor;
                            if adjacent.is_some() {
                                while boundary > 0
                                    && candidate.tabs[boundary - 1].group_id == adjacent
                                {
                                    boundary -= 1;
                                }
                            }
                            candidate.tabs[boundary..end].rotate_right(end - start);
                        }
                        TabGroupMoveDirection::Next => {
                            let mut boundary = neighbor + 1;
                            if adjacent.is_some() {
                                while boundary < candidate.tabs.len()
                                    && candidate.tabs[boundary].group_id == adjacent
                                {
                                    boundary += 1;
                                }
                            }
                            candidate.tabs[start..boundary].rotate_left(end - start);
                        }
                    }
                }
            }
        }
    }
    Ok((candidate != *context).then_some(candidate))
}

fn tab_index(
    context: &TerminalContext,
    id: &WorkbenchTabId,
) -> Result<usize, WorkbenchRejectionReason> {
    context
        .tabs
        .iter()
        .position(|tab| &tab.tab_id == id)
        .ok_or(WorkbenchRejectionReason::TabUnavailable)
}

fn require_group(
    context: &TerminalContext,
    id: &WorkbenchTabGroupId,
) -> Result<(), WorkbenchRejectionReason> {
    context
        .groups
        .iter()
        .any(|group| &group.group_id == id)
        .then_some(())
        .ok_or(WorkbenchRejectionReason::TabGroupUnavailable)
}

fn assign_group(
    context: &mut TerminalContext,
    id: &WorkbenchTabId,
    group: Option<&WorkbenchTabGroupId>,
) -> Result<(), WorkbenchRejectionReason> {
    if let Some(group) = group {
        require_group(context, group)?;
    }
    let index = tab_index(context, id)?;
    if context.tabs[index].group_id.as_ref() == group {
        return Ok(());
    }
    let mut tab = context.tabs.remove(index);
    let source = tab.group_id.clone();
    tab.group_id = group.cloned();
    if group.is_some() {
        tab.pinned = false;
    }
    let pinned = context.tabs.iter().take_while(|tab| tab.pinned).count();
    let insertion = if let Some(group) = group {
        context
            .tabs
            .iter()
            .rposition(|tab| tab.group_id.as_ref() == Some(group))
            .map_or(index.clamp(pinned, context.tabs.len()), |last| last + 1)
    } else if !context.tabs.iter().any(|tab| tab.group_id == source) {
        index.clamp(pinned, context.tabs.len())
    } else {
        context.tabs.len()
    };
    // A new single-member group cannot split a surviving source group.
    let insertion = if insertion > 0
        && insertion < context.tabs.len()
        && context.tabs[insertion - 1].group_id.is_some()
        && context.tabs[insertion - 1].group_id == context.tabs[insertion].group_id
    {
        let neighbor = &context.tabs[insertion].group_id;
        insertion
            + context.tabs[insertion..]
                .iter()
                .take_while(|tab| &tab.group_id == neighbor)
                .count()
    } else {
        insertion
    };
    context.tabs.insert(insertion, tab);
    remove_empty_groups(context);
    Ok(())
}

fn remove_empty_groups(context: &mut TerminalContext) {
    context.groups.retain(|group| {
        context
            .tabs
            .iter()
            .any(|tab| tab.group_id.as_ref() == Some(&group.group_id))
    });
}
