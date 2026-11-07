//! Client-owned Panel focus history behind Last Panel. It records each settled, presented Panel
//! focus across the Session Workbench, whatever moved it, and steps back to the most recent
//! previous Panel that still exists, switching Tab or Workspace through the ordinary selection
//! owners. A transient plugin overlay never enters the history; a step with no target does
//! nothing.
use super::held_intent::StructureAction;
use super::{ClientActionDisposition, ClientPhaseKind, WorkbenchClientUi};
use crate::client_view::ClientAction;
use crate::workbench_projection::{ClientContextLocation, project_native_context_tabs};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};
use std::collections::VecDeque;

/// Enough to step back through recent work; the oldest focus leaves first.
const CAPACITY: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Focus {
    location: ClientContextLocation,
    tab: WorkbenchTabId,
    panel: WorkbenchPanelId,
}

/// Most recent first, each Panel at most once.
#[derive(Default)]
pub(super) struct PanelFocusHistory {
    entries: VecDeque<Focus>,
}

impl PanelFocusHistory {
    fn record(&mut self, focus: Focus) {
        if self.entries.front() == Some(&focus) {
            return;
        }
        self.entries.retain(|entry| entry.panel != focus.panel);
        self.entries.push_front(focus);
        self.entries.truncate(CAPACITY);
    }
}

/// How the Client reaches a history entry from the current focus.
enum Step {
    /// Another Panel of the current Tab.
    Panel(WorkbenchPanelId),
    /// A Panel of another Tab here, by the Tab's shown position.
    Tab(u8),
    /// A Panel of another Workbench location, by its displayed position.
    Context(usize),
}

impl WorkbenchClientUi {
    /// The settled focus of this Client: the selected Panel of the current Tab once no switch is
    /// landing.
    fn current_panel_focus(&self) -> Option<Focus> {
        if self.retirement_failure.is_some() || self.navigation_pending() || self.switch_landing() {
            return None;
        }
        let state = self.state.as_ref()?;
        if !matches!(
            state.phase_kind(),
            ClientPhaseKind::Ready | ClientPhaseKind::SelectedWithoutAttachment
        ) {
            return None;
        }
        let active = state.active()?;
        let tab = state.cache().tabs().get(active.index())?;
        let panel = tab.panel(active.panel_index())?;
        Some(Focus {
            location: state.cache().location().clone(),
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
        })
    }

    /// Records the presented focus. Pointer, keyboard and requested focus changes all arrive
    /// here; a plugin overlay's Panel does not.
    pub(super) fn remember_panel_focus(&mut self) {
        let Some(focus) = self.current_panel_focus() else {
            return;
        };
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.plugin_overlay_panel(&focus.panel))
        {
            return;
        }
        self.panel_focus_history.record(focus);
    }

    /// Focuses the most recent previously focused Panel that still exists. Entries whose Panel
    /// is gone leave the history; without a target nothing happens.
    pub(super) fn focus_last_panel(&mut self) -> ClientActionDisposition {
        self.remember_panel_focus();
        let Some(current) = self.current_panel_focus() else {
            return ClientActionDisposition::KeepNavigator;
        };
        let mut target = None;
        let history = std::mem::take(&mut self.panel_focus_history.entries);
        let mut kept = VecDeque::with_capacity(history.len());
        for entry in history {
            if entry.panel != current.panel {
                let Some(step) = self.last_panel_step(&current, &entry) else {
                    continue;
                };
                if target.is_none() {
                    target = Some((entry.clone(), step));
                }
            }
            kept.push_back(entry);
        }
        self.panel_focus_history.entries = kept;
        let Some((entry, step)) = target else {
            return ClientActionDisposition::KeepNavigator;
        };
        match step {
            Step::Panel(panel) => self.structure_action(StructureAction::FocusPanel(panel)),
            Step::Tab(position) => {
                if let Some(state) = self.state.as_mut() {
                    state.prefer_panel(&entry.location, &entry.tab, &entry.panel);
                }
                self.structure_action(StructureAction::SelectTab(ClientAction::SelectTab(
                    position,
                )))
            }
            Step::Context(index) => {
                if let Some(state) = self.state.as_mut() {
                    state.prefer_panel(&entry.location, &entry.tab, &entry.panel);
                }
                self.select_context_at(index)
            }
        }
    }

    /// How to reach `entry`, or `None` when its Panel, Tab or Workspace is gone or leaving.
    fn last_panel_step(&self, current: &Focus, entry: &Focus) -> Option<Step> {
        let state = self.state.as_ref()?;
        if &entry.location == state.cache().location() {
            let index = state.cache().tab_index(&entry.tab)?;
            let tab = state.cache().tabs().get(index)?;
            if !state.tab_shown(index)
                || tab.panel_index(&entry.panel).is_none()
                || !state.panel_shown(&entry.tab, &entry.panel)
            {
                return None;
            }
            if entry.tab == current.tab {
                return Some(Step::Panel(entry.panel.clone()));
            }
            let position = (0..=index).filter(|index| state.tab_shown(*index)).count();
            return u8::try_from(position).ok().map(Step::Tab);
        }
        // An accepted Workspace close takes the Workspace out of navigation first.
        let index = state.display_position(&entry.location)?;
        let tabs = project_native_context_tabs(state.cache().canonical_snapshot(), &entry.location)
            .ok()?;
        let tab = tabs.tabs().get(tabs.tab_index(&entry.tab)?)?;
        tab.panel_index(&entry.panel)?;
        Some(Step::Context(index))
    }
}
