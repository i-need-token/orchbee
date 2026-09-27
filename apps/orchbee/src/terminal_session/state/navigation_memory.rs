use super::{NativeContextTabs, NativeSessionState, NativeSimpleTab, SelectedTab, SessionPhase};
use crate::terminal_session::workbench_projection::NativeContextLocation;
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};
use std::collections::BTreeMap;

/// Volatile choices owned by one Session. Membership, bindings and Create
/// permission remain with the canonical projection and provenance owners.
#[derive(Debug, Default)]
pub(super) struct NavigationMemory {
    contexts: Vec<ContextMemory>,
}

#[derive(Debug)]
struct ContextMemory {
    location: NativeContextLocation,
    last_tab: Option<WorkbenchTabId>,
    panels: BTreeMap<WorkbenchTabId, WorkbenchPanelId>,
}

impl NavigationMemory {
    pub(super) fn remember(&mut self, cache: &NativeContextTabs, selected: &SelectedTab) {
        let Some(tab) = cache.tabs().get(selected.index()) else {
            return;
        };
        if tab.tab_id() != selected.key().anchor().tab_id()
            || tab
                .panel(selected.panel_index())
                .is_none_or(|panel| panel.panel_id() != selected.key().panel())
        {
            return;
        }
        let index = self
            .contexts
            .iter()
            .position(|entry| &entry.location == cache.location())
            .unwrap_or_else(|| {
                self.contexts.push(ContextMemory {
                    location: cache.location().clone(),
                    last_tab: None,
                    panels: BTreeMap::new(),
                });
                self.contexts.len() - 1
            });
        let entry = &mut self.contexts[index];
        entry.last_tab = Some(tab.tab_id().clone());
        entry
            .panels
            .insert(tab.tab_id().clone(), selected.key().panel().clone());
    }

    pub(super) fn tab(&self, location: &NativeContextLocation) -> Option<&WorkbenchTabId> {
        self.contexts
            .iter()
            .find(|entry| &entry.location == location)?
            .last_tab
            .as_ref()
    }

    pub(super) fn panel_index(
        &self,
        location: &NativeContextLocation,
        tab: &NativeSimpleTab,
    ) -> usize {
        self.contexts
            .iter()
            .find(|entry| &entry.location == location)
            .and_then(|entry| entry.panels.get(tab.tab_id()))
            .and_then(|panel_id| tab.panel_index(panel_id))
            .unwrap_or(0)
    }

    pub(super) fn prune(
        &mut self,
        cache: &NativeContextTabs,
        retained: &[super::context_selection::RetainedContext],
    ) {
        self.contexts.retain_mut(|entry| {
            if !cache
                .navigation()
                .iter()
                .any(|context| context.location == entry.location)
            {
                return false;
            }
            // Current membership is complete, including absence. Never revive
            // a removed member from an older retained copy of this location.
            let tabs = if &entry.location == cache.location() {
                Some(cache.tabs())
            } else {
                retained
                    .iter()
                    .find(|context| context.location() == &entry.location)
                    .map(super::context_selection::RetainedContext::tabs)
            };
            let Some(tabs) = tabs else {
                return false;
            };
            if entry
                .last_tab
                .as_ref()
                .is_some_and(|id| !tabs.iter().any(|tab| tab.tab_id() == id))
            {
                entry.last_tab = None;
            }
            entry.panels.retain(|tab_id, panel_id| {
                tabs.iter()
                    .any(|tab| tab.tab_id() == tab_id && tab.panel_index(panel_id).is_some())
            });
            entry.last_tab.is_some() || !entry.panels.is_empty()
        });
    }
}

impl NativeSessionState {
    /// The same stable-identity preference used by normal context reentry, without a visit.
    pub(in crate::terminal_session) fn query_active_tab_id(
        &self,
        candidate: &NativeContextTabs,
    ) -> Result<Option<WorkbenchTabId>, ()> {
        if candidate.location() == self.cache.location() {
            let selected = self.active().map(|active| active.key().anchor().tab_id());
            if selected.is_some_and(|id| candidate.tab_index(id).is_none())
                || (selected.is_none() && !candidate.tabs().is_empty())
            {
                return Err(());
            }
            return Ok(selected.cloned());
        }
        Ok(self
            .navigation_memory
            .tab(candidate.location())
            .filter(|id| candidate.tab_index(id).is_some())
            .cloned()
            .or_else(|| candidate.tabs().first().map(|tab| tab.tab_id().clone())))
    }

    /// Resolve a queried inactive Tab without recording a visit or changing selection.
    pub(in crate::terminal_session) fn layout_focus_panel_id(
        &self,
        location: &NativeContextLocation,
        tab: &NativeSimpleTab,
    ) -> Option<WorkbenchPanelId> {
        tab.panel(self.navigation_memory.panel_index(location, tab))
            .map(|panel| panel.panel_id().clone())
    }

    /// Called after navigation or creation admission, before a pending phase
    /// is published. An unsuccessful later result remembers only the departure.
    pub(super) fn remember_current_navigation(&mut self) {
        let selected = match &self.phase {
            SessionPhase::Ready(ready) => ready.selected(),
            SessionPhase::SelectedWithoutAttachment(selected)
            | SessionPhase::RetryableAttachUnavailable(selected) => selected,
            _ => return,
        };
        self.navigation_memory.remember(&self.cache, selected);
    }

    pub(super) fn prune_navigation_memory(&mut self) {
        self.navigation_memory
            .prune(&self.cache, &self.retained_contexts);
    }
}
