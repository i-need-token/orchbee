//! Client-owned Panel focus history behind Last Panel. It records each settled, presented Panel
//! focus across the Session Workbench, whatever moved it, and steps back to the most recent
//! previous Panel that still exists, switching Tab or Workspace through the ordinary selection
//! owners. A transient plugin overlay never enters the history; a step with no target does
//! nothing. The same history also picks where focus returns when a Tab closes that holds the
//! focused Panel, and that return stays within the closed Tab's Workspace.
use super::held_intent::StructureAction;
use super::{ClientActionDisposition, ClientPhaseKind, WorkbenchClientUi};
use crate::client_view::ClientAction;
use crate::workbench_projection::{ClientContextLocation, project_native_context_tabs};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};
use std::collections::VecDeque;

#[cfg(test)]
mod tests;

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

/// Where focus goes when the Tabs in `closed` close: if the focused Panel is in one of them, the
/// most recent `history` entry outside them that `is_open` accepts. `history` is most recent first.
/// None means focus stays where it is, or no entry qualifies, so the post-close landing applies.
pub(super) fn panel_focus_after_tab_close<'a, Entry>(
    closed: &[WorkbenchTabId],
    focused: Option<&Entry>,
    history: impl IntoIterator<Item = &'a Entry>,
    tab_of: impl Fn(&Entry) -> &WorkbenchTabId,
    is_open: impl Fn(&Entry) -> bool,
) -> Option<&'a Entry>
where
    Entry: 'a,
{
    let focused = focused?;
    if !closed.contains(tab_of(focused)) {
        return None;
    }
    history
        .into_iter()
        .find(|&entry| !closed.contains(tab_of(entry)) && is_open(entry))
}

/// The Panel a Tab close returns focus to, picked as `panel_focus_after_tab_close` picks it. The
/// closed Tabs are in the focused Panel's Workspace, so only Panels of that Workspace qualify:
/// focus never moves into another Workspace on a close.
fn focus_return_candidate<'a>(
    closed: &[WorkbenchTabId],
    focused: &Focus,
    history: impl IntoIterator<Item = &'a Focus>,
    is_open: impl Fn(&Focus) -> bool,
) -> Option<&'a Focus> {
    panel_focus_after_tab_close(
        closed,
        Some(focused),
        history,
        |entry| &entry.tab,
        |entry| entry.location == focused.location && is_open(entry),
    )
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

    /// The most recent earlier Panel that still exists, and how to reach it from `current`. The
    /// Last Panel row and the step both ask here, so an enabled row always has a Panel to return
    /// to.
    fn last_panel_target(&self, current: &Focus) -> Option<(Focus, Step)> {
        self.panel_focus_history
            .entries
            .iter()
            .filter(|entry| entry.panel != current.panel)
            .find_map(|entry| {
                self.last_panel_step(current, entry)
                    .map(|step| (entry.clone(), step))
            })
    }

    /// Whether Last Panel has an earlier Panel to return to, or `None` while focus has not settled
    /// and only the step, held until it settles, can tell.
    pub(super) fn last_panel_available(&self) -> Option<bool> {
        let current = self.current_panel_focus()?;
        Some(self.last_panel_target(&current).is_some())
    }

    /// Focuses the most recent previously focused Panel that still exists. Entries whose Panel
    /// is gone leave the history; without a target nothing happens.
    pub(super) fn focus_last_panel(&mut self) -> ClientActionDisposition {
        self.remember_panel_focus();
        let Some(current) = self.current_panel_focus() else {
            return ClientActionDisposition::KeepNavigator;
        };
        let target = self.last_panel_target(&current);
        let history = std::mem::take(&mut self.panel_focus_history.entries);
        let kept = history
            .into_iter()
            .filter(|entry| {
                entry.panel == current.panel || self.last_panel_step(&current, entry).is_some()
            })
            .collect();
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

    /// The Tab index focus returns to when the selected Tab's own close is accepted, with its
    /// Panel preferred for the switch that follows. Only a Panel of that Tab's Workspace is picked;
    /// without one, None leaves the post-close landing in place.
    pub(super) fn focus_return_after_tab_close(&mut self) -> Option<usize> {
        let state = self.state.as_ref()?;
        if !state.selected_tab_close_accepted() {
            return None;
        }
        let current = self.current_panel_focus()?;
        let closed: Vec<WorkbenchTabId> = state
            .cache()
            .tabs()
            .iter()
            .filter(|tab| state.tab_leaving(tab.tab_id()))
            .map(|tab| tab.tab_id().clone())
            .collect();
        let entry = focus_return_candidate(
            &closed,
            &current,
            &self.panel_focus_history.entries,
            |entry| self.last_panel_step(&current, entry).is_some(),
        )?
        .clone();
        let index = state.cache().tab_index(&entry.tab)?;
        self.state
            .as_mut()?
            .prefer_panel(&entry.location, &entry.tab, &entry.panel);
        Some(index)
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
