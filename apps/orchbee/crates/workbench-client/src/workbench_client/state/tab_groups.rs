//! Client-local group disclosure and Tab strip scrolling; canonical metadata remains in the
//! projection.
use super::{ClientContextTabs, WorkbenchClientState, WorkbenchClientView};
use crate::tui_pointer::TabStripStart;
use orchbee_workbench_protocol::{
    SessionId, WorkbenchTabGroupId, WorkbenchTabId, WorkbenchTerminalContextId,
};

pub(in crate::workbench_client) enum TabSelectionChange {
    Toggle,
    Range,
    All,
    Clear,
}

#[derive(Default)]
pub(super) struct GroupView {
    context: Option<(SessionId, WorkbenchTerminalContextId)>,
    collapsed: Vec<WorkbenchTabGroupId>,
    selected: Vec<WorkbenchTabId>,
    anchor: Option<WorkbenchTabId>,
    recent: Option<WorkbenchTabGroupId>,
    scroll: Option<StripScroll>,
}

/// A Tab strip window the person paged to with an edge marker. It lasts while the current Tab is
/// the one it was paged from, or a Tab the person then activated inside the painted window.
struct StripScroll {
    start: TabStripStart,
    /// Canonical Tab order when last settled; a closed start Tab yields to its next survivor.
    order: Vec<WorkbenchTabId>,
    current: WorkbenchTabId,
    followed: Option<WorkbenchTabId>,
}

impl GroupView {
    fn matches(&self, cache: &ClientContextTabs) -> bool {
        self.context.as_ref().is_some_and(|(session, context)| {
            session == cache.session_id() && Some(context) == cache.terminal_context_id()
        })
    }

    fn prepare(&mut self, cache: &ClientContextTabs) {
        if !self.matches(cache) {
            *self = Self {
                context: cache
                    .terminal_context_id()
                    .map(|context| (cache.session_id().clone(), context.clone())),
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
    pub(super) fn collapsed<'a>(&'a self, cache: &ClientContextTabs) -> &'a [WorkbenchTabGroupId] {
        if self.matches(cache) {
            &self.collapsed
        } else {
            &[]
        }
    }

    fn toggle(&mut self, cache: &ClientContextTabs, group: &WorkbenchTabGroupId) {
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

impl WorkbenchClientState {
    fn active_tab_id(&self) -> Option<&WorkbenchTabId> {
        self.active()
            .and_then(|active| self.cache.tabs().get(active.index()))
            .map(|tab| tab.tab_id())
    }

    /// Pages the strip to start at a painted token. Scrolling never changes the current Tab.
    pub(in crate::workbench_client) fn scroll_tab_strip(&mut self, start: TabStripStart) {
        let Some(current) = self.active_tab_id().cloned() else {
            return;
        };
        if self.cache.tab_index(&start.tab).is_none() {
            return;
        }
        self.tab_groups.prepare(&self.cache);
        self.tab_groups.scroll = Some(StripScroll {
            start,
            order: self
                .cache
                .tabs()
                .iter()
                .map(|tab| tab.tab_id().clone())
                .collect(),
            current,
            followed: None,
        });
    }

    /// A pointer activation of a Tab painted in the paged window keeps that window.
    pub(in crate::workbench_client) fn follow_tab_strip_scroll(&mut self, tab: &WorkbenchTabId) {
        if self.tab_groups.matches(&self.cache)
            && let Some(scroll) = self.tab_groups.scroll.as_mut()
        {
            scroll.followed = Some(tab.clone());
        }
    }

    /// Before each frame: any other change of the current Tab returns the strip to the window
    /// around it, and a closed start Tab yields to the next surviving Tab, then the previous one.
    pub(in crate::workbench_client) fn settle_tab_strip_scroll(&mut self) {
        if self.tab_groups.scroll.is_none() {
            return;
        }
        let active = self.active_tab_id().cloned();
        let cache = &self.cache;
        let matches = self.tab_groups.matches(cache);
        let Some(scroll) = self.tab_groups.scroll.as_mut().filter(|_| matches) else {
            self.tab_groups.scroll = None;
            return;
        };
        if active.as_ref() != Some(&scroll.current) {
            match scroll.followed.take() {
                Some(followed) if active.as_ref() == Some(&followed) => scroll.current = followed,
                _ => {
                    self.tab_groups.scroll = None;
                    return;
                }
            }
        }
        if cache.tab_index(&scroll.start.tab).is_none() {
            let position = scroll
                .order
                .iter()
                .position(|id| *id == scroll.start.tab)
                .unwrap_or(scroll.order.len());
            let survivor = scroll.order[position.saturating_add(1).min(scroll.order.len())..]
                .iter()
                .chain(scroll.order[..position].iter().rev())
                .find(|id| cache.tab_index(id).is_some())
                .cloned();
            let Some(tab) = survivor else {
                self.tab_groups.scroll = None;
                return;
            };
            scroll.start = TabStripStart { tab, header: false };
        }
        scroll.order = cache
            .tabs()
            .iter()
            .map(|tab| tab.tab_id().clone())
            .collect();
    }

    /// The paged window's start while it still applies to the current Tab.
    pub(in crate::workbench_client) fn tab_strip_start(&self) -> Option<&TabStripStart> {
        let scroll = self
            .tab_groups
            .scroll
            .as_ref()
            .filter(|_| self.tab_groups.matches(&self.cache))?;
        let active = self.active_tab_id()?;
        (*active == scroll.current || Some(active) == scroll.followed.as_ref())
            .then_some(&scroll.start)
            .filter(|start| self.cache.tab_index(&start.tab).is_some())
    }

    pub(in crate::workbench_client) fn recent_tab_group(
        &self,
    ) -> Option<&orchbee_workbench_protocol::TabGroup> {
        self.active()
            .and_then(|active| self.cache.tabs().get(active.index()))
            .and_then(|tab| tab.group())
            .or_else(|| {
                self.tab_groups
                    .recent
                    .as_ref()
                    .filter(|_| self.tab_groups.matches(&self.cache))
                    .and_then(|id| {
                        self.cache
                            .groups()
                            .iter()
                            .find(|group| &group.group_id == id)
                    })
            })
            .or_else(|| self.cache.groups().last())
    }

    pub(super) fn remember_tab_group(&mut self) {
        let group = self.recent_tab_group().map(|group| group.group_id.clone());
        self.tab_groups.prepare(&self.cache);
        self.tab_groups.recent = group;
    }
    /// Native marking keeps the current terminal attached. The active Tab is always included,
    /// and canonical order determines the selected set independently from marking order.
    pub(in crate::workbench_client) fn selected_tab_ids(&self) -> Vec<WorkbenchTabId> {
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

    pub(in crate::workbench_client) fn clear_tab_selection(&mut self) {
        self.remember_tab_group();
        self.cancel_duplicate_sequence();
        self.tab_groups.selected.clear();
        self.tab_groups.anchor = None;
    }

    pub(in crate::workbench_client) fn mark_tabs(
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

    pub(in crate::workbench_client) fn group_collapsed(&self, id: &WorkbenchTabGroupId) -> bool {
        self.tab_groups.collapsed(&self.cache).contains(id)
    }
    pub(in crate::workbench_client) fn toggle_tab_group(&mut self, id: &WorkbenchTabGroupId) {
        self.tab_groups.toggle(&self.cache, id);
    }
}
impl WorkbenchClientView<'_> {
    pub(in crate::workbench_client) fn selected_tab_ids(&self) -> Vec<WorkbenchTabId> {
        self.state.selected_tab_ids()
    }
    pub(in crate::workbench_client) fn tab_strip_start(&self) -> Option<&TabStripStart> {
        self.state.tab_strip_start()
    }
    pub(in crate::workbench_client) fn group_collapsed(&self, id: &WorkbenchTabGroupId) -> bool {
        self.state.group_collapsed(id)
    }
}
