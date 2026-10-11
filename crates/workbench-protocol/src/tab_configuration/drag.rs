//! Atomic counterpart of the GUI TabModel's drag insertion. Input is the source
//! order, never a sequence of speculative intermediate moves.
use crate::{TerminalContext, WorkbenchRejectionReason, WorkbenchTabGroupId, WorkbenchTabId};

pub(super) fn move_tabs(
    context: &mut TerminalContext,
    ids: &[WorkbenchTabId],
    boundary: usize,
    group: Option<&WorkbenchTabGroupId>,
) -> Result<(), WorkbenchRejectionReason> {
    use WorkbenchRejectionReason as R;
    super::require_tabs(context, ids)?;
    if boundary > context.tabs.len() {
        return Err(R::TabMoveIndexOutOfBounds);
    }
    if let Some(group) = group {
        super::require_group(context, group)?;
    }
    let indices: Vec<_> = context
        .tabs
        .iter()
        .enumerate()
        .filter_map(|(index, tab)| ids.contains(&tab.tab_id).then_some(index))
        .collect();
    let Some(&first) = indices.first() else {
        return Err(R::TabUnavailable);
    };
    let pinned = context.tabs[first].pinned;
    if indices
        .iter()
        .any(|index| context.tabs[*index].pinned != pinned)
        || (pinned && group.is_some())
    {
        return Err(R::TabMoveOrderConflict);
    }
    let partition = context.tabs.iter().take_while(|tab| tab.pinned).count();
    let boundary = if pinned {
        boundary.min(partition)
    } else {
        boundary.max(partition)
    };
    let mut insertion = boundary - indices.iter().filter(|index| **index < boundary).count();
    // GUI no-op releases return before pruning unrelated empty groups.
    if insertion == first
        && indices.windows(2).all(|pair| pair[1] == pair[0] + 1)
        && indices
            .iter()
            .all(|index| context.tabs[*index].group_id.as_ref() == group)
    {
        return Ok(());
    }
    let full_groups: Vec<_> = context
        .groups
        .iter()
        .filter(|candidate| {
            let members: Vec<_> = context
                .tabs
                .iter()
                .filter(|tab| tab.group_id.as_ref() == Some(&candidate.group_id))
                .collect();
            !members.is_empty() && members.iter().all(|tab| ids.contains(&tab.tab_id))
        })
        .map(|candidate| candidate.group_id.clone())
        .collect();
    // A carried header is a collection, not a member of another collection.
    if group.is_some_and(|group| full_groups.iter().any(|full| full != group)) {
        return Err(R::TabMoveOrderConflict);
    }
    let mut moving = Vec::with_capacity(indices.len());
    let mut remaining = Vec::with_capacity(context.tabs.len() - indices.len());
    for mut tab in std::mem::take(&mut context.tabs) {
        if ids.contains(&tab.tab_id) {
            if !tab
                .group_id
                .as_ref()
                .is_some_and(|id| full_groups.contains(id))
            {
                tab.group_id = group.cloned();
            }
            moving.push(tab);
        } else {
            remaining.push(tab);
        }
    }
    if let Some(group) = group
        && let Some(start) = remaining
            .iter()
            .position(|tab| tab.group_id.as_ref() == Some(group))
    {
        let end = start
            + remaining[start..]
                .iter()
                .take_while(|tab| tab.group_id.as_ref() == Some(group))
                .count();
        insertion = insertion.clamp(start, end);
    }
    let left = insertion
        .checked_sub(1)
        .and_then(|index| remaining.get(index))
        .and_then(|tab| tab.group_id.as_ref());
    let right = remaining
        .get(insertion)
        .and_then(|tab| tab.group_id.as_ref());
    if let Some(adjacent) = left.filter(|left| Some(*left) == right && Some(*left) != group) {
        if boundary <= first {
            while insertion > 0 && remaining[insertion - 1].group_id.as_ref() == Some(adjacent) {
                insertion -= 1;
            }
        } else {
            while insertion < remaining.len()
                && remaining[insertion].group_id.as_ref() == Some(adjacent)
            {
                insertion += 1;
            }
        }
    }
    remaining.splice(insertion..insertion, moving);
    context.tabs = remaining;
    super::remove_empty_groups(context);
    Ok(())
}
