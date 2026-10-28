//! Transient overlay return chains belong to the Client that owns focus and zoom.
use super::{
    ClientContextTabs, ClientTab, Instant, SessionId, WorkbenchClientState, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use orchbee_workbench_protocol::{
    ClientOverlayMember, ClientViewControlReason as Reason, ClientViewIntent, PresentationBinding,
};

const MAXIMUM_OVERLAYS: usize = 32;

#[derive(Clone, Debug)]
pub(super) struct Overlay {
    pub(super) context: WorkbenchTerminalContextId,
    pub(super) tab: WorkbenchTabId,
    pub(super) member: ClientOverlayMember,
    return_to: ClientOverlayMember,
    previously_zoomed: bool,
    pub(super) owns_zoom: bool,
    pending: Option<WorkbenchRequestId>,
    close: Option<WorkbenchRequestId>,
}

#[derive(Clone, Debug)]
pub(in crate::workbench_client) struct OverlayJob {
    pub(in crate::workbench_client) session: SessionId,
    pub(in crate::workbench_client) context: WorkbenchTerminalContextId,
    pub(in crate::workbench_client) tab: WorkbenchTabId,
    pub(in crate::workbench_client) member: ClientOverlayMember,
    pub(in crate::workbench_client) id: WorkbenchRequestId,
    pub(in crate::workbench_client) deadline: Instant,
}

#[derive(Debug)]
pub(in crate::workbench_client) enum OverlayResult {
    /// A read or a definitely refused close can be observed again.
    Waiting,
    /// Accepted or possibly submitted; no automatic resubmission is permitted.
    CloseOwned { operation_id: WorkbenchRequestId },
}

fn matches(panel: &super::super::ClientPanel, member: &ClientOverlayMember) -> bool {
    panel.panel_id() == &member.panel_id
        && panel
            .presentation_index(&member.presentation_id)
            .and_then(|i| panel.presentation(i))
            .is_some_and(|p| {
                p.binding().map(|b| PresentationBinding {
                    worker_id: b.worker_id().clone(),
                    terminal_id: b.terminal_id().clone(),
                }) == member.binding
            })
}

impl WorkbenchClientState {
    pub(in crate::workbench_client) fn register_plugin_overlay(
        &mut self,
        intent: &ClientViewIntent,
    ) -> Result<(), Reason> {
        let ClientViewIntent::Overlay {
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            expected_binding,
            return_to,
            previously_zoomed,
        } = intent
        else {
            return Err(Reason::Unsupported);
        };
        let active = self.active().ok_or(Reason::TargetUnavailable)?;
        let tab = self
            .cache
            .tabs()
            .get(active.index())
            .ok_or(Reason::TargetUnavailable)?;
        if self.cache.terminal_context_id() != Some(terminal_context_id)
            || tab.tab_id() != tab_id
            || tab.panel_count() < 2
            || self.focus_panel_id() != Some(&return_to.panel_id)
            || self.zoom_active() != *previously_zoomed
            || self.plugin_overlays.len() >= MAXIMUM_OVERLAYS
            || self
                .plugin_overlays
                .iter()
                .any(|o| o.member.panel_id == *panel_id)
        {
            return Err(Reason::StaleView);
        }
        let source = tab
            .panel(active.panel_index())
            .ok_or(Reason::TargetUnavailable)?;
        if !matches(source, return_to)
            || self.selected_presentation(source).presentation_id() != &return_to.presentation_id
        {
            return Err(Reason::TargetUnavailable);
        }
        let member = ClientOverlayMember {
            panel_id: panel_id.clone(),
            presentation_id: presentation_id.clone(),
            binding: Some(expected_binding.clone()),
        };
        if tab
            .panel_index(panel_id)
            .and_then(|i| tab.panel(i))
            .is_none_or(|p| !matches(p, &member) || p.presentation_id() != presentation_id)
        {
            return Err(Reason::TargetUnavailable);
        }
        self.plugin_overlays.push(Overlay {
            context: terminal_context_id.clone(),
            tab: tab_id.clone(),
            member,
            return_to: return_to.clone(),
            previously_zoomed: *previously_zoomed,
            owns_zoom: true,
            pending: None,
            close: None,
        });
        Ok(())
    }

    pub(in crate::workbench_client) fn arm_overlay_zoom(&mut self, id: &WorkbenchPanelId) {
        if let Some(overlay) = self
            .plugin_overlays
            .iter_mut()
            .find(|o| &o.member.panel_id == id)
        {
            overlay.owns_zoom = true;
        }
    }

    pub(super) fn overlay_return(
        &self,
        tab: &ClientTab,
        removed: &WorkbenchPanelId,
    ) -> Option<(ClientOverlayMember, bool)> {
        let overlay = self
            .plugin_overlays
            .iter()
            .find(|o| &o.member.panel_id == removed && &o.tab == tab.tab_id())?;
        let mut member = overlay.return_to.clone();
        let mut zoomed = overlay.previously_zoomed;
        for _ in 0..MAXIMUM_OVERLAYS {
            if tab
                .panel_index(&member.panel_id)
                .and_then(|i| tab.panel(i))
                .is_some_and(|p| matches(p, &member))
            {
                return Some((member, zoomed));
            }
            let Some(parent) = self.plugin_overlays.iter().find(|o| {
                o.member == member && o.tab == overlay.tab && o.context == overlay.context
            }) else {
                // A removed source still has a prior zoom choice. Normal membership
                // repair chooses a surviving leaf; it must not erase that choice.
                return Some((member, zoomed));
            };
            member = parent.return_to.clone();
            zoomed = parent.previously_zoomed;
        }
        None
    }

    pub(super) fn overlay_return_index(
        &self,
        tab: &ClientTab,
        removed: &WorkbenchPanelId,
    ) -> Option<usize> {
        self.overlay_return(tab, removed).and_then(|(member, _)| {
            tab.panel_index(&member.panel_id)
                .filter(|i| tab.panel(*i).is_some_and(|p| matches(p, &member)))
        })
    }

    pub(super) fn overlay_return_presentation<'a>(
        &self,
        tab: &ClientTab,
        removed: &WorkbenchPanelId,
        panel: &'a super::super::ClientPanel,
    ) -> Option<&'a super::super::ClientPresentation> {
        let (member, _) = self.overlay_return(tab, removed)?;
        if !matches(panel, &member) {
            return None;
        }
        panel
            .presentation_index(&member.presentation_id)
            .and_then(|i| panel.presentation(i))
    }

    pub(super) fn prune_plugin_overlays(&mut self, previous_focus: Option<&WorkbenchPanelId>) {
        let cache = self.cache.clone();
        self.prune_plugin_overlays_in(&cache, previous_focus);
    }

    pub(in crate::workbench_client) fn observe_overlay_snapshot(
        &mut self,
        snapshot: &orchbee_workbench_protocol::Snapshot,
    ) {
        use crate::workbench_client::workbench_projection::{
            project_native_context_tabs, project_preferred_workspace,
        };
        if snapshot.session_id != *self.cache.session_id()
            || snapshot.revision < self.cache.revision()
        {
            return;
        }
        let Ok(navigation) = project_preferred_workspace(snapshot) else {
            return;
        };
        let contexts: std::collections::BTreeSet<_> = self
            .plugin_overlays
            .iter()
            .map(|o| o.context.clone())
            .collect();
        for context in contexts {
            if Some(&context) == self.cache.terminal_context_id() {
                continue;
            }
            if let Some(location) = navigation
                .navigation()
                .iter()
                .find(|n| n.location.terminal_context_id() == Some(&context))
                .map(|n| &n.location)
                && let Ok(cache) = project_native_context_tabs(snapshot, location)
            {
                self.prune_plugin_overlays_in(&cache, None);
            }
        }
        self.plugin_overlays.retain(|o| {
            if navigation
                .navigation()
                .iter()
                .any(|n| n.location.terminal_context_id() == Some(&o.context))
            {
                true
            } else {
                self.zoomed_tabs.remove(&o.tab);
                false
            }
        });
        self.prune_zoomed_tabs();
    }

    fn prune_plugin_overlays_in(
        &mut self,
        cache: &ClientContextTabs,
        previous_focus: Option<&WorkbenchPanelId>,
    ) {
        let records = std::mem::take(&mut self.plugin_overlays);
        // Retain the complete old chain until every invalid member's return has been resolved.
        self.plugin_overlays = records.clone();
        for overlay in records.iter().rev() {
            if Some(&overlay.context) != cache.terminal_context_id() {
                if !cache
                    .navigation()
                    .iter()
                    .any(|n| n.location.terminal_context_id() == Some(&overlay.context))
                {
                    self.plugin_overlays.retain(|o| o.member != overlay.member);
                }
                continue;
            }
            let tab = cache.tabs().iter().find(|t| t.tab_id() == &overlay.tab);
            if tab.is_some_and(|t| {
                t.panel_index(&overlay.member.panel_id)
                    .and_then(|i| t.panel(i))
                    .is_some_and(|p| matches(p, &overlay.member))
            }) {
                continue;
            }
            let removed = tab.is_some_and(|t| t.panel_index(&overlay.member.panel_id).is_none());
            if tab.is_none_or(|t| t.panel_count() <= 1) {
                self.zoomed_tabs.remove(&overlay.tab);
            }
            let current_tab = cache.location() == self.cache.location()
                && self
                    .active()
                    .is_some_and(|a| a.key().anchor().tab_id() == &overlay.tab);
            let owned_focus = if current_tab {
                previous_focus == Some(&overlay.member.panel_id)
            } else {
                self.navigation_memory
                    .overlay_focus(cache.location(), &overlay.tab)
                    == Some(&overlay.member.panel_id)
            };
            if let Some(tab) = tab
                && let Some((return_to, zoomed)) =
                    self.overlay_return(tab, &overlay.member.panel_id)
            {
                if removed && owned_focus {
                    if tab
                        .panel_index(&return_to.panel_id)
                        .and_then(|i| tab.panel(i))
                        .is_some_and(|p| matches(p, &return_to))
                    {
                        self.navigation_memory.restore_overlay_focus(
                            cache.location(),
                            &overlay.tab,
                            &overlay.member.panel_id,
                            &return_to,
                        );
                    }
                    if overlay.owns_zoom {
                        if zoomed && tab.panel_count() > 1 {
                            self.zoomed_tabs.insert(overlay.tab.clone());
                        } else {
                            self.zoomed_tabs.remove(&overlay.tab);
                        }
                    }
                }
                for child in &mut self.plugin_overlays {
                    if child.context == overlay.context
                        && child.tab == overlay.tab
                        && child.return_to == overlay.member
                    {
                        child.return_to = return_to.clone();
                        child.previously_zoomed = zoomed;
                    }
                }
            }
        }
        self.plugin_overlays.retain(|o| {
            Some(&o.context) != cache.terminal_context_id()
                && cache
                    .navigation()
                    .iter()
                    .any(|n| n.location.terminal_context_id() == Some(&o.context))
                || Some(&o.context) == cache.terminal_context_id()
                    && cache
                        .tabs()
                        .iter()
                        .find(|t| t.tab_id() == &o.tab)
                        .and_then(|t| t.panel_index(&o.member.panel_id).and_then(|i| t.panel(i)))
                        .is_some_and(|p| matches(p, &o.member))
        });
    }

    pub(in crate::workbench_client) fn begin_overlay_job(
        &mut self,
        id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Option<OverlayJob> {
        let overlay = self
            .plugin_overlays
            .iter_mut()
            .find(|o| o.pending.is_none() && o.close.is_none())?;
        overlay.pending = Some(id.clone());
        Some(OverlayJob {
            session: self.cache.session_id().clone(),
            context: overlay.context.clone(),
            tab: overlay.tab.clone(),
            member: overlay.member.clone(),
            id,
            deadline,
        })
    }

    pub(in crate::workbench_client) fn finish_overlay_job(
        &mut self,
        job: &OverlayJob,
        result: OverlayResult,
    ) {
        if let Some(overlay) = self.plugin_overlays.iter_mut().find(|o| {
            o.member == job.member
                && o.context == job.context
                && o.tab == job.tab
                && o.pending.as_ref() == Some(&job.id)
        }) {
            overlay.pending = None;
            if let OverlayResult::CloseOwned { operation_id } = result {
                overlay.close = Some(operation_id);
            } else {
                // Fairness: a permanently unavailable overlay must not starve other records.
                let index = self
                    .plugin_overlays
                    .iter()
                    .position(|o| o.member == job.member)
                    .expect("matched overlay");
                let overlay = self.plugin_overlays.remove(index);
                self.plugin_overlays.push(overlay);
            }
        }
    }
}
