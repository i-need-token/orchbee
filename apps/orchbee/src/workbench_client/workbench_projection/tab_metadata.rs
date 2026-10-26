//! Selected-context group metadata validation and exact mutation receipt guards.
use super::{
    NativeContextTabs, NativeProjectionDivergence, NativeProjectionError, WorkbenchRevision,
};
use orchbee_workbench_protocol::{
    MAXIMUM_TAB_GROUPS_PER_CONTEXT, TerminalContext, WorkbenchTabGroupLabel,
};
use std::collections::{HashMap, HashSet};

pub(super) fn validate(context: &TerminalContext) -> Result<(), NativeProjectionError> {
    if context.groups.len() > MAXIMUM_TAB_GROUPS_PER_CONTEXT {
        return Err(NativeProjectionError);
    }
    let mut groups = HashSet::new();
    for group in &context.groups {
        if !groups.insert(&group.group_id) || WorkbenchTabGroupLabel::parse(&group.label).is_err() {
            return Err(NativeProjectionError);
        }
    }
    let mut unpinned = false;
    let mut positions = HashMap::new();
    for (index, tab) in context.tabs.iter().enumerate() {
        if tab.pinned && (unpinned || tab.group_id.is_some()) {
            return Err(NativeProjectionError);
        }
        unpinned |= !tab.pinned;
        if let Some(group) = &tab.group_id
            && (!groups.contains(group)
                || positions
                    .insert(group, index)
                    .is_some_and(|previous| previous + 1 != index))
        {
            return Err(NativeProjectionError);
        }
    }
    Ok(())
}

pub(super) fn validate_receipt(
    baseline: &NativeContextTabs,
    candidate: &NativeContextTabs,
    committed_revision: WorkbenchRevision,
) -> Result<(), NativeProjectionDivergence> {
    if candidate.revision == committed_revision
        && (candidate.groups() != baseline.groups()
            || candidate.tabs.iter().any(|next| {
                baseline
                    .tabs
                    .iter()
                    .find(|prior| prior.tab_id == next.tab_id)
                    .is_some_and(|prior| prior.pinned != next.pinned || prior.group != next.group)
            }))
    {
        return Err(NativeProjectionDivergence);
    }
    Ok(())
}
