//! Client-local group disclosure; canonical metadata remains in the projection.
use super::{NativeContextTabs, NativeSessionState, NativeSessionView};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchTabGroupId, WorkbenchTabId, WorkbenchTerminalContextId,
};

pub(in crate::terminal_session) enum TabSelectionChange {
    Toggle,
    Range,
    All,
    Clear,
}

#[derive(Default)]
pub(super) struct GroupView {
    context: Option<(SiteId, WorkbenchTerminalContextId)>,
    collapsed: Vec<WorkbenchTabGroupId>,
    selected: Vec<WorkbenchTabId>,
    anchor: Option<WorkbenchTabId>,
}

impl GroupView {
    fn matches(&self, cache: &NativeContextTabs) -> bool {
        self.context.as_ref().is_some_and(|(site, context)| {
            site == cache.site_id() && context == cache.terminal_context_id()
        })
    }

    fn prepare(&mut self, cache: &NativeContextTabs) {
        if !self.matches(cache) {
            *self = Self {
                context: Some((cache.site_id().clone(), cache.terminal_context_id().clone())),
                ..Self::default()
            };
        }
        self.collapsed
            .retain(|id| cache.groups().iter().any(|group| &group.group_id == id));
        self.selected.retain(|id| cache.tab_index(id).is_some());
        if self
            .anchor
            .as_ref()
            .is_some_and(|id| cache.tab_index(id).is_none())
        {
            self.anchor = None;
        }
    }
    pub(super) fn collapsed<'a>(&'a self, cache: &NativeContextTabs) -> &'a [WorkbenchTabGroupId] {
        if self.matches(cache) {
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
        self.prepare(cache);
        if let Some(index) = self.collapsed.iter().position(|id| id == group) {
            self.collapsed.remove(index);
        } else {
            self.collapsed.push(group.clone());
        }
    }
}

impl NativeSessionState {
    /// Native marking keeps the current terminal attached. The active Tab is always included,
    /// and canonical order determines the selected set independently from marking order.
    pub(in crate::terminal_session) fn selected_tab_ids(&self) -> Vec<WorkbenchTabId> {
        let active = self
            .active()
            .and_then(|active| self.cache.tabs().get(active.index()))
            .map(|tab| tab.tab_id());
        self.cache
            .tabs()
            .iter()
            .filter(|tab| {
                Some(tab.tab_id()) == active
                    || (self.tab_groups.matches(&self.cache)
                        && self.tab_groups.selected.contains(tab.tab_id()))
            })
            .map(|tab| tab.tab_id().clone())
            .collect()
    }

    pub(in crate::terminal_session) fn clear_tab_selection(&mut self) {
        self.cancel_duplicate_sequence();
        self.tab_groups.selected.clear();
        self.tab_groups.anchor = None;
    }

    pub(in crate::terminal_session) fn mark_tabs(
        &mut self,
        target: &WorkbenchTabId,
        change: TabSelectionChange,
    ) -> Option<()> {
        self.cancel_duplicate_sequence();
        if self.setup_blocks_structure() || !self.setup_authority_available() {
            return None;
        }
        let index = self.cache.tab_index(target)?;
        let active = self
            .active()
            .and_then(|active| self.cache.tabs().get(active.index()))
            .map(|tab| tab.tab_id().clone());
        let selected = self.selected_tab_ids();
        self.tab_groups.prepare(&self.cache);
        self.tab_groups.selected = selected;
        self.tab_groups.anchor = self.tab_groups.anchor.clone().or(active.clone());
        match change {
            TabSelectionChange::All => {
                self.tab_groups.selected = self
                    .cache
                    .tabs()
                    .iter()
                    .map(|tab| tab.tab_id().clone())
                    .collect()
            }
            TabSelectionChange::Clear => self.clear_tab_selection(),
            TabSelectionChange::Range => {
                let anchor = self
                    .tab_groups
                    .anchor
                    .as_ref()
                    .and_then(|id| self.cache.tab_index(id))
                    .unwrap_or(index);
                self.tab_groups.selected = self.cache.tabs()[anchor.min(index)..=anchor.max(index)]
                    .iter()
                    .map(|tab| tab.tab_id().clone())
                    .collect();
            }
            TabSelectionChange::Toggle => {
                if let Some(index) = self.tab_groups.selected.iter().position(|id| id == target) {
                    if active.as_ref() != Some(target) {
                        self.tab_groups.selected.remove(index);
                    }
                } else {
                    self.tab_groups.selected.push(target.clone());
                }
            }
        }
        Some(())
    }

    pub(in crate::terminal_session) fn group_collapsed(&self, id: &WorkbenchTabGroupId) -> bool {
        self.tab_groups.collapsed(&self.cache).contains(id)
    }
    pub(in crate::terminal_session) fn toggle_tab_group(&mut self, id: &WorkbenchTabGroupId) {
        self.tab_groups.toggle(&self.cache, id);
    }
}
impl NativeSessionView<'_> {
    pub(in crate::terminal_session) fn selected_tab_ids(&self) -> Vec<WorkbenchTabId> {
        self.state.selected_tab_ids()
    }
    pub(in crate::terminal_session) fn group_collapsed(&self, id: &WorkbenchTabGroupId) -> bool {
        self.state.group_collapsed(id)
    }
}
