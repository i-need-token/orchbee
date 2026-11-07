//! Retains bounded identity-based context, Tab, Panel, and presentation preferences and prunes
//! them against admitted membership. It chooses Client-local remembered selections without
//! changing canonical display order or creating a missing target.

use super::{ClientContextTabs, ClientPhase, ClientTab, SelectedTab, WorkbenchClientState};
use crate::workbench_projection::ClientContextLocation;
use orchbee_workbench_protocol::{TerminalPresentationId, WorkbenchPanelId, WorkbenchTabId};
use std::collections::BTreeMap;

/// Volatile choices owned by one Client. Membership, bindings and Create
/// permission remain with the canonical projection and provenance owners.
#[derive(Debug, Default)]
pub(super) struct NavigationMemory {
    contexts: Vec<ContextMemory>,
}

#[derive(Debug)]
struct ContextMemory {
    location: ClientContextLocation,
    last_tab: Option<WorkbenchTabId>,
    panels: BTreeMap<WorkbenchTabId, WorkbenchPanelId>,
    presentations: BTreeMap<WorkbenchPanelId, TerminalPresentationId>,
}

impl NavigationMemory {
    pub(super) fn remember(&mut self, cache: &ClientContextTabs, selected: &SelectedTab) {
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
                    presentations: BTreeMap::new(),
                });
                self.contexts.len() - 1
            });
        let entry = self.contexts.remove(index);
        self.contexts.push(entry);
        let entry = self.contexts.last_mut().expect("remembered context");
        entry.last_tab = Some(tab.tab_id().clone());
        entry
            .panels
            .insert(tab.tab_id().clone(), selected.key().panel().clone());
    }

    /// Names `panel` as the Panel a later switch to `tab` lands on, and `tab` as the Tab a later
    /// landing in `location` selects, for a step back to a Panel focused before.
    fn prefer(
        &mut self,
        location: &ClientContextLocation,
        tab: &WorkbenchTabId,
        panel: &WorkbenchPanelId,
    ) {
        let index = self
            .contexts
            .iter()
            .position(|entry| &entry.location == location)
            .unwrap_or_else(|| {
                // A preference is no visit, so it does not make the location recent.
                self.contexts.insert(
                    0,
                    ContextMemory {
                        location: location.clone(),
                        last_tab: None,
                        panels: BTreeMap::new(),
                        presentations: BTreeMap::new(),
                    },
                );
                0
            });
        let entry = &mut self.contexts[index];
        entry.last_tab = Some(tab.clone());
        entry.panels.insert(tab.clone(), panel.clone());
    }

    pub(super) fn recent_locations(&self) -> impl Iterator<Item = &ClientContextLocation> {
        self.contexts.iter().rev().map(|entry| &entry.location)
    }

    pub(super) fn remember_presentation(
        &mut self,
        cache: &ClientContextTabs,
        selected: &SelectedTab,
        id: TerminalPresentationId,
    ) {
        self.remember(cache, selected);
        if let Some(entry) = self
            .contexts
            .iter_mut()
            .find(|entry| &entry.location == cache.location())
        {
            entry
                .presentations
                .insert(selected.key().panel().clone(), id);
        }
    }

    pub(super) fn presentation<'a>(
        &self,
        location: &ClientContextLocation,
        panel: &'a crate::workbench_projection::ClientPanel,
    ) -> &'a crate::workbench_projection::ClientPresentation {
        self.contexts
            .iter()
            .find(|entry| &entry.location == location)
            .and_then(|entry| entry.presentations.get(panel.panel_id()))
            .and_then(|id| panel.presentation_index(id))
            .and_then(|index| panel.presentation(index))
            .unwrap_or_else(|| panel.primary())
    }

    pub(super) fn tab(&self, location: &ClientContextLocation) -> Option<&WorkbenchTabId> {
        self.contexts
            .iter()
            .find(|entry| &entry.location == location)?
            .last_tab
            .as_ref()
    }

    /// The Panel remembered for a Tab, in whichever context holds it.
    pub(super) fn panel(&self, tab: &WorkbenchTabId) -> Option<&WorkbenchPanelId> {
        self.contexts.iter().find_map(|entry| entry.panels.get(tab))
    }

    pub(super) fn overlay_focus(
        &self,
        location: &ClientContextLocation,
        tab: &WorkbenchTabId,
    ) -> Option<&WorkbenchPanelId> {
        self.contexts
            .iter()
            .find(|entry| &entry.location == location)?
            .panels
            .get(tab)
    }

    pub(super) fn restore_overlay_focus(
        &mut self,
        location: &ClientContextLocation,
        tab: &WorkbenchTabId,
        from: &WorkbenchPanelId,
        to: &orchbee_workbench_protocol::ClientOverlayMember,
    ) {
        if let Some(entry) = self
            .contexts
            .iter_mut()
            .find(|entry| &entry.location == location)
            && (entry.panels.get(tab) == Some(from) || entry.panels.get(tab) == Some(&to.panel_id))
        {
            entry.panels.insert(tab.clone(), to.panel_id.clone());
            entry
                .presentations
                .insert(to.panel_id.clone(), to.presentation_id.clone());
        }
    }

    pub(super) fn panel_index(&self, location: &ClientContextLocation, tab: &ClientTab) -> usize {
        self.contexts
            .iter()
            .find(|entry| &entry.location == location)
            .and_then(|entry| entry.panels.get(tab.tab_id()))
            .and_then(|panel_id| tab.panel_index(panel_id))
            .unwrap_or(0)
    }

    pub(super) fn prune(
        &mut self,
        cache: &ClientContextTabs,
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
            entry.presentations.retain(|panel, presentation| {
                tabs.iter().any(|tab| {
                    tab.panel_index(panel)
                        .and_then(|index| tab.panel(index))
                        .is_some_and(|panel| panel.presentation_index(presentation).is_some())
                })
            });
            entry.last_tab.is_some() || !entry.panels.is_empty()
        });
    }
}

impl WorkbenchClientState {
    /// The same stable-identity preference used by normal context reentry, without a visit.
    pub(crate) fn query_active_tab_id(
        &self,
        candidate: &ClientContextTabs,
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
    pub(crate) fn layout_focus_panel_id(
        &self,
        location: &ClientContextLocation,
        tab: &ClientTab,
    ) -> Option<WorkbenchPanelId> {
        tab.panel(self.navigation_memory.panel_index(location, tab))
            .map(|panel| panel.panel_id().clone())
    }

    /// Called after navigation or creation admission, before a pending phase
    /// is published. An unsuccessful later result remembers only the departure.
    pub(super) fn remember_current_navigation(&mut self) {
        let selected = match &self.phase {
            ClientPhase::Ready(ready) => ready.selected(),
            ClientPhase::SelectedWithoutAttachment(selected)
            | ClientPhase::RetryableAttachUnavailable(selected) => selected,
            _ => return,
        };
        self.navigation_memory.remember(&self.cache, selected);
        if let Some(entry) = self
            .navigation_memory
            .contexts
            .iter_mut()
            .find(|entry| &entry.location == self.cache.location())
        {
            let tab = &self.cache.tabs()[selected.index()];
            for record in &self.panes {
                if record.key().anchor() != selected.key().anchor() || record.departing {
                    continue;
                }
                if let Some(panel) = tab
                    .panel_index(record.key().panel())
                    .and_then(|index| tab.panel(index))
                    && let Some(member) = panel.bound_presentation(record.binding())
                {
                    entry
                        .presentations
                        .insert(panel.panel_id().clone(), member.presentation_id().clone());
                }
            }
        }
    }

    /// Last Panel's target: the next switch to `tab`, or landing in `location`, selects `panel`.
    /// Pruning still drops it once the Panel is gone.
    pub(crate) fn prefer_panel(
        &mut self,
        location: &ClientContextLocation,
        tab: &WorkbenchTabId,
        panel: &WorkbenchPanelId,
    ) {
        self.navigation_memory.prefer(location, tab, panel);
    }

    pub(super) fn prune_navigation_memory(&mut self) {
        self.navigation_memory
            .prune(&self.cache, &self.retained_contexts);
    }
}
