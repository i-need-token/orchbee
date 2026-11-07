//! Client-owned Panel focus history behind Last Panel. It records each settled, presented Panel
//! focus across the Session Workbench, whatever moved it, and steps back to the most recent
//! previous Panel that still exists, switching Tab or Workspace through the ordinary selection
//! owners. A transient plugin overlay never enters the history; a step with no target does
//! nothing. The same history also picks where focus returns when a Tab closes that holds the
//! focused Panel; a Panel in another Workspace is reached once that close completes, unless the
//! person acts first.
use super::held_intent::StructureAction;
use super::{ClientActionDisposition, ClientPhaseKind, WorkbenchClientUi};
use crate::client_view::ClientAction;
use crate::workbench_projection::{ClientContextLocation, project_native_context_tabs};
use crate::workbench_state::WorkbenchClientState;
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

/// The Panel in another Workspace that focus returns to once the Tab close that picked it settles.
/// Context selection is refused while that close is open, so the return waits for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingFocusReturn {
    /// The Workspace the close happened in; leaving it ends the return.
    origin: ClientContextLocation,
    /// The Tab the close takes, whose state says whether the close is open, stopped or over.
    closing: WorkbenchTabId,
    /// The Panel focus returns to.
    target: Focus,
    /// The input epoch the close request recorded; input after it ends the return.
    epoch: u64,
}

/// Where the Tab of a pending return stands on a tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClosingTab {
    /// Its close is still on its way, so the Tab stays out of view.
    Leaving,
    /// Its close stopped, so the Tab shows again with its report.
    Stopped,
    /// It left the Client's view and its close is over.
    Gone,
}

/// What a pending return does on one tick.
#[derive(Debug, Eq, PartialEq)]
enum FocusReturnStep<'a> {
    /// The close is open, or context selection does not admit a switch yet.
    Wait,
    /// The return ends, and the post-close landing stays.
    Drop,
    /// Prefers the target Panel, then switches to its Workspace at the displayed position `index`.
    Switch { index: usize, target: &'a Focus },
}

/// Holds the Panel a Tab close returns focus to when that Panel is in another Workspace than the
/// close's own. The slot keeps one return, so a close observed again keeps its first pick.
fn capture_focus_return(
    slot: &mut Option<PendingFocusReturn>,
    origin: &ClientContextLocation,
    closing: &WorkbenchTabId,
    picked: &Focus,
    epoch: u64,
) {
    if picked.location == *origin || slot.as_ref().is_some_and(|held| held.closing == *closing) {
        return;
    }
    *slot = Some(PendingFocusReturn {
        origin: origin.clone(),
        closing: closing.clone(),
        target: picked.clone(),
        epoch,
    });
}

/// Records a Tab close request. A new close ends any pending return from an earlier one, and the
/// input epoch the request leaves behind is the one its pick is held against.
fn record_tab_close_request(
    slot: &mut Option<PendingFocusReturn>,
    tab_close_epoch: &mut u64,
    input_epoch: u64,
) {
    *slot = None;
    *tab_close_epoch = input_epoch;
}

/// Whether input has moved since the close request that picked `pending`. A key press, paste or
/// mouse button press after that request has already moved the keyboard, so the return ends.
fn input_drops_focus_return(pending: &PendingFocusReturn, input_epoch: u64) -> bool {
    pending.epoch != input_epoch
}

/// One tick of a pending return. It waits while its close is open or a switch is not admissible,
/// then switches at `index` once the close is over, or ends when the close stopped or the Panel is
/// gone.
fn settle_focus_return(
    pending: &PendingFocusReturn,
    closing: ClosingTab,
    admissible: bool,
    index: Option<usize>,
) -> FocusReturnStep<'_> {
    match closing {
        ClosingTab::Leaving => FocusReturnStep::Wait,
        ClosingTab::Stopped => FocusReturnStep::Drop,
        ClosingTab::Gone if !admissible => FocusReturnStep::Wait,
        ClosingTab::Gone => index.map_or(FocusReturnStep::Drop, |index| FocusReturnStep::Switch {
            index,
            target: &pending.target,
        }),
    }
}

/// The displayed position of `entry`'s Workspace, when its Tab and Panel are still open there.
fn context_index(state: &WorkbenchClientState, entry: &Focus) -> Option<usize> {
    // An accepted Workspace close takes the Workspace out of navigation first.
    let index = state.display_position(&entry.location)?;
    let tabs =
        project_native_context_tabs(state.cache().canonical_snapshot(), &entry.location).ok()?;
    let tab = tabs.tabs().get(tabs.tab_index(&entry.tab)?)?;
    tab.panel_index(&entry.panel)?;
    Some(index)
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

    /// Records a Tab close the person requested or confirmed; see `record_tab_close_request`.
    pub(super) fn note_tab_close_request(&mut self) {
        record_tab_close_request(
            &mut self.pending_focus_return,
            &mut self.tab_close_epoch,
            self.input_epoch,
        );
    }

    /// The Tab index focus returns to when the selected Tab's own close is accepted, with its
    /// Panel preferred for the switch that follows. A pick in another Workspace is held as the
    /// pending return instead, because context selection is refused while the close is open; the
    /// post-close landing stays until that return runs.
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
        let entry = panel_focus_after_tab_close(
            &closed,
            Some(&current),
            &self.panel_focus_history.entries,
            |entry| &entry.tab,
            |entry| self.last_panel_step(&current, entry).is_some(),
        )?
        .clone();
        if &entry.location != state.cache().location() {
            capture_focus_return(
                &mut self.pending_focus_return,
                state.cache().location(),
                &current.tab,
                &entry,
                self.tab_close_epoch,
            );
            return None;
        }
        let index = state.cache().tab_index(&entry.tab)?;
        self.state
            .as_mut()?
            .prefer_panel(&entry.location, &entry.tab, &entry.panel);
        Some(index)
    }

    /// Runs the pending return once its Tab close is over, or ends it. It waits while the close is
    /// open, and ends when the person acted since the close request, the Client left the origin
    /// Workspace, the close stopped, or the Panel is gone; the landing then stays where the close
    /// put it.
    pub(super) fn follow_pending_focus_return(&mut self) -> bool {
        let Some(pending) = self.pending_focus_return.clone() else {
            return false;
        };
        let step = match self.state.as_ref() {
            _ if input_drops_focus_return(&pending, self.input_epoch) => FocusReturnStep::Drop,
            Some(state) if state.cache().location() == &pending.origin => {
                let closing = if state.tab_leaving(&pending.closing) {
                    ClosingTab::Leaving
                } else if state.cache().tab_index(&pending.closing).is_some() {
                    ClosingTab::Stopped
                } else {
                    ClosingTab::Gone
                };
                settle_focus_return(
                    &pending,
                    closing,
                    state.context_selection_available(),
                    context_index(state, &pending.target),
                )
            }
            _ => FocusReturnStep::Drop,
        };
        let (index, target) = match step {
            FocusReturnStep::Wait => return false,
            FocusReturnStep::Drop => {
                self.pending_focus_return = None;
                return false;
            }
            FocusReturnStep::Switch { index, target } => (index, target.clone()),
        };
        // The same steps as Last Panel's jump to another Workspace: prefer the Panel, then switch.
        self.pending_focus_return = None;
        if let Some(state) = self.state.as_mut() {
            state.prefer_panel(&target.location, &target.tab, &target.panel);
        }
        self.select_context_at_generation(index).is_some()
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
        Some(Step::Context(context_index(state, entry)?))
    }
}
