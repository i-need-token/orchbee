//! Client-local group disclosure; canonical metadata remains in the projection.
use super::{NativeContextTabs, NativeSessionState, NativeSessionView};
use orchbee_workbench_protocol::{SiteId, WorkbenchTabGroupId, WorkbenchTerminalContextId};

#[derive(Default)]
pub(super) struct GroupView {
    context: Option<(SiteId, WorkbenchTerminalContextId)>,
    collapsed: Vec<WorkbenchTabGroupId>,
}

impl GroupView {
    pub(super) fn collapsed<'a>(&'a self, cache: &NativeContextTabs) -> &'a [WorkbenchTabGroupId] {
        if self.context.as_ref().is_some_and(|(site, context)| {
            site == cache.site_id() && context == cache.terminal_context_id()
        }) {
            &self.collapsed
        } else {
            &[]
        }
    }

    fn toggle(&mut self, cache: &NativeContextTabs, group: &WorkbenchTabGroupId) {
        if !cache
            .groups()
            .iter()
            .any(|candidate| &candidate.group_id == group)
        {
            return;
        }
        let context = (cache.site_id().clone(), cache.terminal_context_id().clone());
        if self.context.as_ref() != Some(&context) {
            self.collapsed.clear();
            self.context = Some(context);
        }
        self.collapsed
            .retain(|id| cache.groups().iter().any(|group| &group.group_id == id));
        if let Some(index) = self.collapsed.iter().position(|id| id == group) {
            self.collapsed.remove(index);
        } else {
            self.collapsed.push(group.clone());
        }
    }
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn group_collapsed(&self, id: &WorkbenchTabGroupId) -> bool {
        self.tab_groups.collapsed(&self.cache).contains(id)
    }
    pub(in crate::terminal_session) fn toggle_tab_group(&mut self, id: &WorkbenchTabGroupId) {
        self.tab_groups.toggle(&self.cache, id);
    }
}
impl NativeSessionView<'_> {
    pub(in crate::terminal_session) fn group_collapsed(&self, id: &WorkbenchTabGroupId) -> bool {
        self.state.group_collapsed(id)
    }
}
