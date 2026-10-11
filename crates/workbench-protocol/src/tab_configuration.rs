//! Atomic Tab metadata and order transitions shared by the authority and receipt verification.
//! Callers own revision checks, identity allocation, persistence and publication; no terminal effects occur here.
use crate::{
    ContractError, MAXIMUM_TAB_GROUPS_PER_CONTEXT, MAXIMUM_TABS_PER_CONTEXT, TabGroup,
    TabGroupColor, TerminalContext, WorkbenchRejectionReason, WorkbenchTabGroupId,
    WorkbenchTabGroupLabel, WorkbenchTabId, WorkbenchTabMoveDirection,
};
use serde::{Deserialize, Serialize};

mod drag;

/// An explicit, nonempty, unique set of existing Tab identities. Canonical order, not input order,
/// determines placement. The context's full membership bound also bounds this command input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<WorkbenchTabId>", into = "Vec<WorkbenchTabId>")]
pub struct WorkbenchTabIdSet(Vec<WorkbenchTabId>);

impl TryFrom<Vec<WorkbenchTabId>> for WorkbenchTabIdSet {
    type Error = ContractError;
    fn try_from(ids: Vec<WorkbenchTabId>) -> Result<Self, Self::Error> {
        if ids.is_empty() || ids.len() > MAXIMUM_TABS_PER_CONTEXT {
            return Err(ContractError::InvalidDocument);
        }
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        if unique.len() != ids.len() {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(ids))
    }
}

impl From<WorkbenchTabIdSet> for Vec<WorkbenchTabId> {
    fn from(ids: WorkbenchTabIdSet) -> Self {
        ids.0
    }
}

impl WorkbenchTabIdSet {
    #[must_use]
    pub fn as_slice(&self) -> &[WorkbenchTabId] {
        &self.0
    }
}

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
        label: WorkbenchTabGroupLabel,
        color: TabGroupColor,
    },
    SetGroup {
        tab_id: WorkbenchTabId,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        group_id: Option<WorkbenchTabGroupId>,
    },
    UpdateGroup {
        group_id: WorkbenchTabGroupId,
        label: WorkbenchTabGroupLabel,
        color: TabGroupColor,
    },
    Ungroup {
        group_id: WorkbenchTabGroupId,
    },
    SetPinned {
        tab_id: WorkbenchTabId,
        pinned: bool,
    },
    CreateGroupForTabs {
        tab_ids: WorkbenchTabIdSet,
        label: WorkbenchTabGroupLabel,
        color: TabGroupColor,
    },
    SetTabsGroup {
        tab_ids: WorkbenchTabIdSet,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        group_id: Option<WorkbenchTabGroupId>,
    },
    SetTabsPinned {
        tab_ids: WorkbenchTabIdSet,
        pinned: bool,
    },
    MoveTab {
        tab_id: WorkbenchTabId,
        direction: WorkbenchTabMoveDirection,
        /// A local view hint for skipping the neighboring collapsed group. Never persisted.
        adjacent_group_collapsed: bool,
    },
    MoveGroup {
        group_id: WorkbenchTabGroupId,
        direction: TabGroupMoveDirection,
    },
    /// One release-only drag, with a boundary in the original canonical order.
    /// The authority preserves complete groups and clamps to the pinned partition.
    MoveTabsByDrag {
        tab_ids: WorkbenchTabIdSet,
        insert_index: crate::WorkbenchTabInsertIndex,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        group_id: Option<WorkbenchTabGroupId>,
    },
}

impl TabConfiguration {
    #[must_use]
    pub const fn creates_group(&self) -> bool {
        matches!(
            self,
            Self::CreateGroup { .. } | Self::CreateGroupForTabs { .. }
        )
    }
}

/// Move past one adjacent group or ungrouped Tab, without wrapping or crossing pinned Tabs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabGroupMoveDirection {
    Previous,
    Next,
}

/// Returns the complete candidate, or `None` for an intentional no-op. The input is never mutated.
/// `created_group` must be a fresh authority-allocated identity exactly for group creation.
/// # Errors
/// Rejects missing members, an incompatible allocated identity, and the group capacity bound.
pub fn configure_tabs(
    context: &TerminalContext,
    operation: &TabConfiguration,
    created_group: Option<&WorkbenchTabGroupId>,
) -> Result<Option<TerminalContext>, WorkbenchRejectionReason> {
    use TabConfiguration::*;
    use WorkbenchRejectionReason as R;
    if operation.creates_group() != created_group.is_some() {
        return Err(R::TabGroupUnavailable);
    }
    let mut candidate = context.clone();
    match operation {
        CreateGroup { label, color, .. } | CreateGroupForTabs { label, color, .. } => {
            let id = created_group.ok_or(R::TabGroupUnavailable)?;
            if candidate.groups.iter().any(|group| &group.group_id == id) {
                return Err(R::TabGroupUnavailable);
            }
            candidate.groups.push(TabGroup {
                group_id: id.clone(),
                label: label.as_str().to_owned(),
                color: *color,
            });
            let ids = match operation {
                CreateGroup { tab_id, .. } => std::slice::from_ref(tab_id),
                CreateGroupForTabs { tab_ids, .. } => tab_ids.as_slice(),
                _ => unreachable!(),
            };
            assign_group(&mut candidate, ids, Some(id))?;
            if candidate.groups.len() > MAXIMUM_TAB_GROUPS_PER_CONTEXT {
                return Err(R::ResourceLimit);
            }
        }
        SetGroup { tab_id, group_id } => assign_group(
            &mut candidate,
            std::slice::from_ref(tab_id),
            group_id.as_ref(),
        )?,
        SetTabsGroup { tab_ids, group_id } => {
            assign_group(&mut candidate, tab_ids.as_slice(), group_id.as_ref())?
        }
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
            set_pinned(&mut candidate, std::slice::from_ref(tab_id), *pinned)?;
        }
        SetTabsPinned { tab_ids, pinned } => {
            set_pinned(&mut candidate, tab_ids.as_slice(), *pinned)?
        }
        MoveTabsByDrag {
            tab_ids,
            insert_index,
            group_id,
        } => {
            drag::move_tabs(
                &mut candidate,
                tab_ids.as_slice(),
                insert_index.value(),
                group_id.as_ref(),
            )?;
        }
        MoveTab {
            tab_id,
            direction,
            adjacent_group_collapsed,
        } => move_tab(
            &mut candidate,
            tab_id,
            *direction,
            *adjacent_group_collapsed,
        )?,
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
    ids: &[WorkbenchTabId],
    group: Option<&WorkbenchTabGroupId>,
) -> Result<(), WorkbenchRejectionReason> {
    if let Some(group) = group {
        require_group(context, group)?;
    }
    require_tabs(context, ids)?;
    let moving_indices: Vec<_> = context
        .tabs
        .iter()
        .enumerate()
        .filter(|(_, tab)| ids.contains(&tab.tab_id) && tab.group_id.as_ref() != group)
        .map(|(index, _)| index)
        .collect();
    let Some(&index) = moving_indices.first() else {
        return Ok(());
    };
    let sources: Vec<_> = moving_indices
        .iter()
        .filter_map(|index| context.tabs[*index].group_id.as_ref())
        .collect();
    let removes_whole_sources = group.is_none()
        && moving_indices
            .iter()
            .enumerate()
            .all(|(offset, current)| *current == index + offset)
        && moving_indices
            .iter()
            .all(|index| context.tabs[*index].group_id.is_some())
        && context.tabs.iter().all(|tab| {
            !tab.group_id
                .as_ref()
                .is_some_and(|id| sources.contains(&id))
                || ids.contains(&tab.tab_id)
        });
    let mut moving = Vec::with_capacity(moving_indices.len());
    let mut remaining = Vec::with_capacity(context.tabs.len() - moving_indices.len());
    for (index, mut tab) in std::mem::take(&mut context.tabs).into_iter().enumerate() {
        if moving_indices.contains(&index) {
            tab.group_id = group.cloned();
            if group.is_some() {
                tab.pinned = false;
            }
            moving.push(tab);
        } else {
            remaining.push(tab);
        }
    }
    context.tabs = remaining;
    let pinned = context.tabs.iter().take_while(|tab| tab.pinned).count();
    let insertion = if let Some(group) = group {
        context
            .tabs
            .iter()
            .rposition(|tab| tab.group_id.as_ref() == Some(group))
            .map_or(index.clamp(pinned, context.tabs.len()), |last| last + 1)
    } else if removes_whole_sources {
        index.clamp(pinned, context.tabs.len())
    } else {
        context.tabs.len()
    };
    // A fresh group cannot split a surviving source group.
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
    context.tabs.splice(insertion..insertion, moving);
    remove_empty_groups(context);
    Ok(())
}

fn require_tabs(
    context: &TerminalContext,
    ids: &[WorkbenchTabId],
) -> Result<(), WorkbenchRejectionReason> {
    for id in ids {
        tab_index(context, id)?;
    }
    Ok(())
}

fn set_pinned(
    context: &mut TerminalContext,
    ids: &[WorkbenchTabId],
    pinned: bool,
) -> Result<(), WorkbenchRejectionReason> {
    require_tabs(context, ids)?;
    let mut moving = Vec::new();
    let mut remaining = Vec::new();
    for mut tab in std::mem::take(&mut context.tabs) {
        if ids.contains(&tab.tab_id) && tab.pinned != pinned {
            tab.pinned = pinned;
            if pinned {
                tab.group_id = None;
            }
            moving.push(tab);
        } else {
            remaining.push(tab);
        }
    }
    let changed = !moving.is_empty();
    let boundary = remaining.iter().take_while(|tab| tab.pinned).count();
    remaining.splice(boundary..boundary, moving);
    context.tabs = remaining;
    if changed {
        remove_empty_groups(context);
    }
    Ok(())
}

fn move_tab(
    context: &mut TerminalContext,
    id: &WorkbenchTabId,
    direction: WorkbenchTabMoveDirection,
    adjacent_collapsed: bool,
) -> Result<(), WorkbenchRejectionReason> {
    use WorkbenchTabMoveDirection::*;
    let start = tab_index(context, id)?;
    if matches!(direction, First | Last) {
        let boundary = context.tabs.iter().take_while(|tab| tab.pinned).count();
        let end = match (direction, context.tabs[start].pinned) {
            (First, true) => 0,
            (First, false) => boundary,
            (Last, true) => boundary - 1,
            _ => context.tabs.len() - 1,
        };
        let mut tab = context.tabs.remove(start);
        tab.group_id = None;
        context.tabs.insert(end, tab);
        remove_empty_groups(context);
        return Ok(());
    }
    let next = if direction == Previous {
        start.checked_sub(1)
    } else {
        start
            .checked_add(1)
            .filter(|index| *index < context.tabs.len())
    };
    let old_group = context.tabs[start].group_id.clone();
    let next = next.filter(|index| context.tabs[*index].pinned == context.tabs[start].pinned);
    let Some(mut end) = next else {
        if old_group.is_some() {
            context.tabs[start].group_id = None;
            remove_empty_groups(context);
        }
        return Ok(());
    };
    let target_group = context.tabs[end].group_id.clone();
    if old_group != target_group {
        if old_group.is_some() {
            context.tabs[start].group_id = None;
            remove_empty_groups(context);
            return Ok(());
        }
        if target_group.is_some() {
            if !adjacent_collapsed {
                context.tabs[start].group_id = target_group;
                return Ok(());
            }
            if direction == Previous {
                while end > 0 && context.tabs[end - 1].group_id == target_group {
                    end -= 1;
                }
            } else {
                while end + 1 < context.tabs.len() && context.tabs[end + 1].group_id == target_group
                {
                    end += 1;
                }
            }
        }
    }
    let tab = context.tabs.remove(start);
    context.tabs.insert(end, tab);
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
