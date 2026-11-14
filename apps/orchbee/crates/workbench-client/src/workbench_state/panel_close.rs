//! Owns exact Panel close confirmation tokens, draw acknowledgement, stale-target rejection,
//! and the captured submission plan. It does not paint the prompt or execute a close before
//! the runtime consumes that plan.

use super::tab_close::CloseTarget;
use super::{
    AttachmentKey, ClientPhase, PaneKey, PanelIntent, PanelOperation, PanelRequestKind,
    WorkbenchClientState,
};
use crate::exchange_values::workbench::{
    WorkbenchTabCloseAccepted, panel_close::WorkbenchPanelCloseAccepted,
};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use crate::workbench_projection::{
    ClientContextTabs, ClientPanelTree, ClientTab, ClientTerminalBinding,
};
use orchbee_workbench_protocol::{
    SessionId, WorkbenchLabel, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PanelCloseConfirmationToken(u64);

impl PanelCloseConfirmationToken {
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

/// The exact leaf a panel-close confirmation was armed against; the confirm path
/// refuses to execute against anything else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PanelCloseTarget {
    session_id: SessionId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
}

impl PanelCloseTarget {
    const fn new(
        session_id: SessionId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    ) -> Self {
        Self {
            session_id,
            terminal_context_id,
            tab_id,
            panel_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PanelCloseSelection {
    target: PanelCloseTarget,
    desired: AttachmentKey,
    panel_label: String,
    tab_label: WorkbenchLabel,
    panel_position: usize,
    tab_position: usize,
    binding: Option<ClientTerminalBinding>,
    dissolves_tab: bool,
    base_revision: WorkbenchRevision,
}

struct PanelCloseConfirmation {
    token: PanelCloseConfirmationToken,
    selection: PanelCloseSelection,
    presentation: PresentationPhase,
    shown: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct PanelCloseConfirmationView<'a> {
    confirmation: &'a PanelCloseConfirmation,
}

impl<'a> PanelCloseConfirmationView<'a> {
    pub(crate) const fn token(&self) -> PanelCloseConfirmationToken {
        self.confirmation.token
    }
    pub(crate) fn panel_label(&self) -> &'a str {
        self.confirmation.selection.panel_label.as_str()
    }
    pub(crate) fn tab_label(&self) -> &'a str {
        self.confirmation.selection.tab_label.as_str()
    }
    pub(crate) fn tab_id(&self) -> &'a WorkbenchTabId {
        &self.confirmation.selection.target.tab_id
    }
    pub(crate) fn panel_id(&self) -> &'a WorkbenchPanelId {
        &self.confirmation.selection.target.panel_id
    }
    pub(crate) const fn panel_position(&self) -> usize {
        self.confirmation.selection.panel_position
    }
    #[cfg(test)]
    pub(crate) const fn tab_position(&self) -> usize {
        self.confirmation.selection.tab_position
    }
    /// Whether closing this leaf removes its entire Tab for every attached Client.
    pub(crate) const fn dissolves_tab(&self) -> bool {
        self.confirmation.selection.dissolves_tab
    }
    pub(crate) const fn interactive(&self) -> bool {
        self.confirmation.shown && self.confirmation.presentation.interactive()
    }
    pub(crate) const fn presentation(&self) -> PresentationPhase {
        self.confirmation.presentation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PanelCloseStateError {
    Busy,
    NotDisplayed,
    StaleConfirmation,
}

#[derive(Default)]
pub(crate) struct PanelCloseState {
    confirmation: Option<PanelCloseConfirmation>,
    next_display_token: u64,
}

impl PanelCloseState {
    fn token(&mut self) -> Option<u64> {
        self.next_display_token = self.next_display_token.checked_add(1)?;
        Some(self.next_display_token)
    }

    fn open_confirmation(
        &mut self,
        selection: PanelCloseSelection,
    ) -> Result<(), PanelCloseStateError> {
        if self.confirmation.is_some() {
            return Err(PanelCloseStateError::Busy);
        }
        let token = PanelCloseConfirmationToken(self.token().ok_or(PanelCloseStateError::Busy)?);
        self.confirmation = Some(PanelCloseConfirmation {
            token,
            selection,
            presentation: PresentationPhase::Pending,
            shown: false,
        });
        Ok(())
    }

    fn cancel_confirmation(&mut self) {
        self.confirmation = None;
    }

    pub(super) fn confirmation(&self) -> Option<PanelCloseConfirmationView<'_>> {
        self.confirmation
            .as_ref()
            .map(|confirmation| PanelCloseConfirmationView { confirmation })
    }

    fn acknowledge_draw(&mut self, prompt: Option<PanelCloseConfirmationToken>) {
        if let Some(confirmation) = &mut self.confirmation {
            confirmation.shown = prompt == Some(confirmation.token);
            if confirmation.shown {
                confirmation.presentation.presented();
            }
        }
    }

    pub(super) fn invalidate_display(&mut self) {
        if let Some(confirmation) = &mut self.confirmation {
            confirmation.shown = false;
        }
    }

    fn confirmed_selection(
        &self,
        token: PanelCloseConfirmationToken,
    ) -> Result<&PanelCloseSelection, PanelCloseStateError> {
        self.confirmation
            .as_ref()
            .filter(|prompt| prompt.token == token && prompt.shown)
            .map(|prompt| &prompt.selection)
            .ok_or(PanelCloseStateError::NotDisplayed)
    }
}

impl WorkbenchClientState {
    pub(crate) fn open_panel_close_confirmation(&mut self) -> Result<(), PanelCloseStateError> {
        if self.close.confirmation().is_some()
            || self.close_status_only()
            || self.pending_refresh.is_some()
            || self.rename_draft_blocks_structure()
            || self.mutation_blocks_structure()
            || !self.invariant_holds()
            || !matches!(
                self.phase,
                ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
        {
            return Err(PanelCloseStateError::Busy);
        }
        let selected = self.active().ok_or(PanelCloseStateError::Busy)?;
        let tab = self
            .cache
            .tabs()
            .get(selected.index())
            .ok_or(PanelCloseStateError::Busy)?;
        let panel = tab
            .panel(selected.panel_index())
            .ok_or(PanelCloseStateError::Busy)?;
        let selection = PanelCloseSelection {
            target: PanelCloseTarget::new(
                self.cache.session_id().clone(),
                self.cache
                    .terminal_context_id()
                    .ok_or(PanelCloseStateError::Busy)?
                    .clone(),
                tab.tab_id().clone(),
                panel.panel_id().clone(),
            ),
            desired: self
                .desired
                .as_ref()
                .map(|key| key.anchor().clone())
                .ok_or(PanelCloseStateError::Busy)?,
            panel_label: self.selected_presentation(panel).display_label().to_owned(),
            tab_label: WorkbenchLabel::parse(&tab.display_label(selected.index()))
                .map_err(|_| PanelCloseStateError::Busy)?,
            panel_position: selected.panel_index() + 1,
            tab_position: selected.index() + 1,
            binding: self.selected_presentation(panel).binding().cloned(),
            dissolves_tab: tab.panel_count() == 1,
            base_revision: self.cache.revision(),
        };
        self.panel_close.open_confirmation(selection)
    }

    pub(crate) fn cancel_panel_close_confirmation(&mut self) {
        self.panel_close.cancel_confirmation();
    }

    pub(crate) fn acknowledge_panel_close_draw(
        &mut self,
        prompt: Option<PanelCloseConfirmationToken>,
    ) {
        self.panel_close.acknowledge_draw(prompt);
    }

    pub(crate) fn invalidate_panel_close_display(&mut self) {
        self.panel_close.invalidate_display();
    }

    /// Whether the frozen selection still matches live Client selection identity: same
    /// session, context, revision, desired attachment, selected Tab and panel id,
    /// leaf binding, and last-leaf nature.
    fn panel_close_selection_matches(&self, selection: &PanelCloseSelection) -> bool {
        let Some(selected) = self.active() else {
            return false;
        };
        let Some(tab) = self.cache.tabs().get(selected.index()) else {
            return false;
        };
        let Some(panel) = tab.panel(selected.panel_index()) else {
            return false;
        };
        self.cache.session_id() == &selection.target.session_id
            && self.cache.terminal_context_id() == Some(&selection.target.terminal_context_id)
            && self.cache.revision() == selection.base_revision
            && self.desired.as_ref().map(PaneKey::anchor) == Some(&selection.desired)
            && tab.tab_id() == &selection.target.tab_id
            && panel.panel_id() == &selection.target.panel_id
            && self.selected_presentation(panel).binding() == selection.binding.as_ref()
            && (tab.panel_count() == 1) == selection.dissolves_tab
    }

    /// Whether the armed confirmation's frozen leaf no longer matches live
    /// Client selection identity; the UI settles a stale armed capture as cancelled
    /// rather than leaving it armed-but-stale swallowing keys.
    pub(super) fn armed_panel_close_stale(&self) -> bool {
        self.panel_close
            .confirmation
            .as_ref()
            .is_some_and(|confirmation| {
                !self.panel_close_selection_matches(&confirmation.selection)
            })
    }

    /// Refuses a new prompt whose first draw is geometry-blocked, or suspends
    /// an admitted prompt while preserving its frozen target and token.
    pub(crate) fn settle_panel_close_not_presented(&mut self) -> Option<GeometrySettlement> {
        let confirmation = self.panel_close.confirmation.as_mut()?;
        confirmation.shown = false;
        let settlement = confirmation.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.panel_close.cancel_confirmation();
            self.set_notice(super::ClientNotice::DialogRefused);
        }
        Some(settlement)
    }

    /// The admitted close always names the frozen leaf: the live Client must
    /// still select the same panel id in the same Tab at the same revision.
    fn validated_panel_close_selection(
        &self,
    ) -> Result<&PanelCloseSelection, PanelCloseStateError> {
        let prompt = self
            .panel_close
            .confirmation()
            .ok_or(PanelCloseStateError::NotDisplayed)?;
        let selection = self.panel_close.confirmed_selection(prompt.token())?;
        if !self.panel_close_selection_matches(selection) {
            return Err(PanelCloseStateError::StaleConfirmation);
        }
        Ok(selection)
    }

    pub(crate) fn plan_panel_close_confirmation(
        &self,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<PanelOperation, PanelCloseStateError> {
        if self.pending_panel().is_some() {
            return Err(PanelCloseStateError::Busy);
        }
        self.validated_panel_close_selection()?;
        self.plan_panel(request_id, PanelRequestKind::Close, deadline)
            .ok_or(PanelCloseStateError::Busy)
    }

    /// Plans the conditional close of the armed confirmation `token`, which stays hidden while the
    /// Server decides whether closing the Panel asks first.
    pub(crate) fn plan_panel_close_if_idle(
        &self,
        token: PanelCloseConfirmationToken,
        request_id: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<PanelOperation, PanelCloseStateError> {
        if self.pending_panel().is_some() {
            return Err(PanelCloseStateError::Busy);
        }
        let selection = self
            .panel_close
            .confirmation
            .as_ref()
            .filter(|prompt| prompt.token == token)
            .map(|prompt| &prompt.selection)
            .ok_or(PanelCloseStateError::StaleConfirmation)?;
        if !self.panel_close_selection_matches(selection) {
            return Err(PanelCloseStateError::StaleConfirmation);
        }
        self.plan_panel(request_id, PanelRequestKind::CloseIfIdle, deadline)
            .ok_or(PanelCloseStateError::Busy)
    }

    /// Begins the conditional close planned from the armed confirmation, which stays armed, and
    /// hidden, until the Server answers.
    pub(crate) fn begin_panel_close_if_idle(
        &mut self,
        operation: &PanelOperation,
    ) -> Result<(), PanelCloseStateError> {
        let selection = self
            .panel_close
            .confirmation
            .as_ref()
            .map(|prompt| &prompt.selection)
            .ok_or(PanelCloseStateError::StaleConfirmation)?;
        if !self.panel_close_selection_matches(selection)
            || !matches!(operation.intent(), PanelIntent::CloseIfIdle)
            || !self.begin_panel(operation.clone())
        {
            return Err(PanelCloseStateError::StaleConfirmation);
        }
        Ok(())
    }

    /// The confirmation can only ever begin a `PanelIntent::Close`; `begin_panel`
    /// re-plans the operation and refuses any drift since the frozen selection.
    pub(crate) fn begin_panel_close_exchange(
        &mut self,
        operation: &PanelOperation,
    ) -> Result<(), PanelCloseStateError> {
        self.validated_panel_close_selection()?;
        if !matches!(operation.intent(), PanelIntent::Close) || !self.begin_panel(operation.clone())
        {
            return Err(PanelCloseStateError::StaleConfirmation);
        }
        self.panel_close.cancel_confirmation();
        Ok(())
    }
}

impl<'a> super::WorkbenchClientView<'a> {
    pub(crate) fn panel_close_confirmation(&self) -> Option<PanelCloseConfirmationView<'a>> {
        self.state.panel_close.confirmation()
    }
    /// Whether the Server is deciding if the armed Panel close needs asking; its confirmation
    /// stays hidden until the answer.
    pub(crate) fn panel_close_deciding(&self) -> bool {
        self.state
            .pending_panel()
            .is_some_and(|operation| matches!(operation.intent(), PanelIntent::CloseIfIdle))
    }
    /// What the armed Panel close ends, from the attachments `settled` reports as Host-settled.
    pub(crate) fn panel_close_impact(
        &self,
        settled: &dyn Fn(&super::PaneRef) -> bool,
    ) -> Option<crate::localization::CloseImpact> {
        let target = &self
            .state
            .panel_close
            .confirmation
            .as_ref()?
            .selection
            .target;
        Some(
            self.state
                .panel_close_impact(&target.tab_id, &target.panel_id, settled),
        )
    }
}

/// The Panels of one Tab whose closes the Server accepted and that the Client has stopped drawing.
/// Each leaf leaves at its own acceptance, as a Tab does, and stays out until its own close
/// settles, whatever the other closes do: a stop shows it again, and the canonical publication that
/// removes it ends its place here. An install that still carries a leaving Panel keeps it, with the
/// tree re-derived from that install.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PanelsLeaving {
    pub(super) tab_id: WorkbenchTabId,
    /// The leaving Panels in acceptance order; never empty.
    pub(super) panel_ids: Vec<WorkbenchPanelId>,
    /// The Tab's tree with every leaving leaf collapsed onto its sibling. When every leaf leaves,
    /// the last one accepted stays in it as the Tab's last frame while the Tab itself leaves.
    pub(super) tree: ClientPanelTree,
}

/// The tree `tab` draws while the Panels `leaving` names leave with their accepted closes.
fn leaving_tree(tab: &ClientTab, leaving: &[WorkbenchPanelId]) -> ClientPanelTree {
    leaving
        .iter()
        .fold(tab.panel_tree().clone(), |tree, panel_id| {
            tab.panel_index(panel_id)
                .and_then(|index| tree.without_leaf(index))
                .unwrap_or(tree)
        })
}

impl WorkbenchClientState {
    /// The Panel tree a Tab draws. A cached Tab with leaving Panels draws its tree with those
    /// leaves collapsed; any other Tab, including a candidate Tab that shares the identifier, draws
    /// its own.
    pub(crate) fn display_panel_tree<'s>(&'s self, tab: &'s ClientTab) -> &'s ClientPanelTree {
        let cached = self
            .cache
            .tabs()
            .iter()
            .any(|cached| std::ptr::eq(cached, tab));
        self.panel_leaving
            .iter()
            .find(|leaving| cached && leaving.tab_id == *tab.tab_id())
            .map_or(tab.panel_tree(), |leaving| &leaving.tree)
    }

    /// Whether this Panel of the cached Tab is still in the Client's view, not leaving with its
    /// accepted close.
    pub(crate) fn panel_shown(&self, tab_id: &WorkbenchTabId, panel_id: &WorkbenchPanelId) -> bool {
        !self.panel_leaves(tab_id, panel_id)
    }

    /// Whether this Panel of the cached Tab is leaving with its accepted close.
    fn panel_leaves(&self, tab_id: &WorkbenchTabId, panel_id: &WorkbenchPanelId) -> bool {
        self.panel_leaving
            .iter()
            .any(|leaving| leaving.tab_id == *tab_id && leaving.panel_ids.contains(panel_id))
    }

    /// Whether every Panel the cached Tab holds is leaving with its accepted close. The Tab then
    /// leaves the Client's view with them, as with a close of its own and as in the GUI.
    pub(crate) fn every_panel_leaving(&self, tab_id: &WorkbenchTabId) -> bool {
        let Some(leaving) = self
            .panel_leaving
            .iter()
            .find(|leaving| leaving.tab_id == *tab_id)
        else {
            return false;
        };
        self.cache
            .tab_index(tab_id)
            .and_then(|index| self.cache.tabs().get(index))
            .is_some_and(|tab| {
                (0..tab.panel_count())
                    .filter_map(|index| tab.panel(index))
                    .all(|panel| leaving.panel_ids.contains(panel.panel_id()))
            })
    }

    /// The Panel a switch to the cached `tab` lands on: the remembered `index`, unless that Panel
    /// is leaving with its accepted close; then the first leaf the Tab still draws.
    pub(super) fn drawn_panel_index(&self, tab: &ClientTab, index: usize) -> usize {
        if tab
            .panel(index)
            .is_none_or(|panel| !self.panel_leaves(tab.tab_id(), panel.panel_id()))
        {
            return index;
        }
        self.display_panel_tree(tab)
            .leaf_panel_indices()
            .first()
            .copied()
            .unwrap_or(index)
    }

    /// Starts drawing what an accepted Panel close removes as gone. A Tab's last Panel takes its
    /// Tab, which is what the Server closes, so that close is retained and followed as the close of
    /// the Tab: the Tab leaves the strip at once and the selection follows it as it follows any
    /// accepted Tab close. Any other Panel leaves its Tab's split, and its close is retained and
    /// followed by its operation, so a close that stops shows the Panel again.
    pub(super) fn leave_accepted_panel_close(
        &mut self,
        operation: &PanelOperation,
        accepted: &WorkbenchPanelCloseAccepted,
    ) {
        let tab_id = operation.target().tab_id();
        let last_panel = self
            .cache
            .tab_index(tab_id)
            .and_then(|index| self.cache.tabs().get(index))
            .is_some_and(|tab| tab.panel_count() == 1);
        if last_panel {
            // A Tab that already holds a retained close keeps that close and its report; a close
            // that cannot be retained leaves the Tab drawn until publication removes it.
            let _ = self.retain_last_panel_close(
                &WorkbenchTabCloseAccepted {
                    close_request_id: accepted.close_request_id.clone(),
                    terminal_context_id: accepted.terminal_context_id.clone(),
                    tab_id: accepted.tab_id.clone(),
                    accepted_revision: accepted.accepted_revision,
                },
                operation.admitted_at(),
            );
            return;
        }
        // A Panel whose retained close is still on its way keeps that close and its report; a close
        // that cannot be retained leaves the Panel drawn until publication removes it. A Panel whose
        // retained close stopped stays drawn until a read finds the close on its way again.
        if self
            .retain_panel_close(accepted, operation.admitted_at())
            .is_ok()
            && let Some(terminal_context_id) = self.cache.terminal_context_id()
        {
            let target = CloseTarget::panel(
                self.cache.session_id().clone(),
                terminal_context_id.clone(),
                tab_id.clone(),
                operation.panel_id().clone(),
            );
            if self.close.panel_close_stopped(&target) == Some(false) {
                self.begin_panel_leaving(tab_id, operation.panel_id());
            }
        }
    }

    /// Shows each leaving Panel again whose retained close stopped, with the close's report, and
    /// keeps every other leaving Panel out. The focus stays where each acceptance moved it; only a
    /// focus left on a leaving Panel, because every Panel of its Tab was leaving, moves to a Panel
    /// the Tab draws again.
    pub(super) fn reveal_stopped_panel_closes(&mut self) {
        let Some(terminal_context_id) = self.cache.terminal_context_id() else {
            return;
        };
        let session_id = self.cache.session_id();
        let close = &self.close;
        let mut revealed = false;
        for leaving in &mut self.panel_leaving {
            let before = leaving.panel_ids.len();
            leaving.panel_ids.retain(|panel_id| {
                let target = CloseTarget::panel(
                    session_id.clone(),
                    terminal_context_id.clone(),
                    leaving.tab_id.clone(),
                    panel_id.clone(),
                );
                close.panel_close_stopped(&target) != Some(true)
            });
            revealed |= leaving.panel_ids.len() != before;
        }
        if revealed {
            self.panel_leaving
                .retain(|leaving| !leaving.panel_ids.is_empty());
            self.rederive_leaving_trees();
            self.move_focus_off_leaving_panel();
        }
    }

    /// Starts drawing a Panel as gone once the Server accepted its close, beside every other Panel
    /// still leaving with its own. When the focus is on that Panel it moves first to the survivor
    /// the canonical install would choose among the Panels the Tab still draws, so the publication
    /// confirms the leaf the user already sees. When the Tab draws no other Panel, the Tab leaves
    /// with this one, which keeps the focus as the Tab's last frame. Without a live survivor the
    /// leaf keeps drawing until the publication removes it.
    pub(super) fn begin_panel_leaving(
        &mut self,
        tab_id: &WorkbenchTabId,
        panel_id: &WorkbenchPanelId,
    ) {
        let Some(index) = self.cache.tab_index(tab_id) else {
            return;
        };
        let Some(panel_index) = self
            .cache
            .tabs()
            .get(index)
            .and_then(|tab| tab.panel_index(panel_id))
        else {
            return;
        };
        let focused_here = self.active().is_some_and(|active| active.index() == index)
            && self.focus_panel_id() == Some(panel_id);
        if focused_here && !self.move_focus_off_leaving(index, panel_index, panel_id) {
            return;
        }
        match self
            .panel_leaving
            .iter_mut()
            .find(|leaving| leaving.tab_id == *tab_id)
        {
            Some(leaving) if leaving.panel_ids.contains(panel_id) => return,
            Some(leaving) => leaving.panel_ids.push(panel_id.clone()),
            None => self.panel_leaving.push(PanelsLeaving {
                tab_id: tab_id.clone(),
                panel_ids: vec![panel_id.clone()],
                tree: ClientPanelTree::Panel { panel_index },
            }),
        }
        self.rederive_leaving_trees();
    }

    /// Moves the focus off the leaving Panel `panel_id` of the selected Tab at `index` onto its
    /// survivor among the Panels the Tab still draws, and drops a zoom on the leaving Panel. True
    /// when the focus may rest where it ends: on the survivor, or on the leaving Panel when the Tab
    /// draws no other. False when the survivor cannot take the focus now.
    fn move_focus_off_leaving(
        &mut self,
        index: usize,
        panel_index: usize,
        panel_id: &WorkbenchPanelId,
    ) -> bool {
        let Some(tab) = self.cache.tabs().get(index) else {
            return false;
        };
        let tab_id = tab.tab_id().clone();
        let Some(survivor) = self.leaving_survivor(tab, panel_index, panel_id) else {
            return true;
        };
        let Some(pane) = self.plan_panel_focus_local_at(&survivor) else {
            return false;
        };
        // Zoom follows the focused Panel, so it is read before the focus leaves that Panel.
        let zoomed_here = self
            .zoomed_panels()
            .iter()
            .any(|(zoomed_tab, zoomed_panel)| *zoomed_tab == tab_id && zoomed_panel == panel_id);
        if !self.move_panel_focus_local(pane) {
            return false;
        }
        if zoomed_here {
            self.zoomed_tabs.remove(&tab_id);
        }
        true
    }

    /// Moves a focus left on a leaving Panel, because every Panel of its Tab was leaving, to a
    /// Panel the Tab draws again.
    fn move_focus_off_leaving_panel(&mut self) {
        let Some(index) = self.active().map(super::SelectedTab::index) else {
            return;
        };
        let Some(focus) = self.focus_panel_id().cloned() else {
            return;
        };
        let Some((tab_id, panel_index)) = self
            .cache
            .tabs()
            .get(index)
            .and_then(|tab| Some((tab.tab_id().clone(), tab.panel_index(&focus)?)))
        else {
            return;
        };
        if self.panel_leaves(&tab_id, &focus) && !self.every_panel_leaving(&tab_id) {
            self.move_focus_off_leaving(index, panel_index, &focus);
        }
    }

    /// The leaf that takes the focus when the focused Panel leaves, among the Panels its Tab still
    /// draws: the overlay return Panel when one applies, else the vanished leaf's place clamped to
    /// those Panels. That is the stand-in the canonical install applies to a vanished selection.
    /// `None` when the Tab draws no other Panel.
    fn leaving_survivor(
        &self,
        tab: &ClientTab,
        removed_index: usize,
        removed: &WorkbenchPanelId,
    ) -> Option<WorkbenchPanelId> {
        let drawn = |index: &usize| {
            *index != removed_index
                && tab
                    .panel(*index)
                    .is_some_and(|panel| !self.panel_leaves(tab.tab_id(), panel.panel_id()))
        };
        let index = match self
            .overlay_return_index(tab, removed)
            .filter(|index| drawn(index))
        {
            Some(index) => index,
            None => {
                let surviving: Vec<usize> = (0..tab.panel_count())
                    .filter(|index| drawn(index))
                    .collect();
                let place = surviving
                    .iter()
                    .take_while(|index| **index < removed_index)
                    .count();
                *surviving.get(place.min(surviving.len().checked_sub(1)?))?
            }
        };
        tab.panel(index).map(|panel| panel.panel_id().clone())
    }

    /// Re-derives the drawn tree of each Tab with leaving Panels from the cached Tab.
    fn rederive_leaving_trees(&mut self) {
        let cache = &self.cache;
        for leaving in &mut self.panel_leaving {
            if let Some(tab) = cache
                .tab_index(&leaving.tab_id)
                .and_then(|index| cache.tabs().get(index))
            {
                leaving.tree = leaving_tree(tab, &leaving.panel_ids);
            }
        }
    }

    /// Keeps each leaving Panel only while the candidate still holds it in the same Context, and
    /// re-derives the drawn trees from the candidate. Once the publication removes a leaf, its
    /// place here ends, and so does the follow of every Panel close whose Panel it removed.
    pub(crate) fn refresh_panel_leaving(&mut self, candidate: &ClientContextTabs) {
        self.close
            .prune_departed_panel_closes(|target| super::tab_close::panel_drawn(candidate, target));
        let leaving = std::mem::take(&mut self.panel_leaving);
        if candidate.location() != self.cache.location() {
            return;
        }
        self.panel_leaving = leaving
            .into_iter()
            .filter_map(|mut leaving| {
                let tab = candidate
                    .tab_index(&leaving.tab_id)
                    .and_then(|index| candidate.tabs().get(index))?;
                leaving
                    .panel_ids
                    .retain(|panel_id| tab.panel_index(panel_id).is_some());
                leaving.tree = leaving_tree(tab, &leaving.panel_ids);
                (!leaving.panel_ids.is_empty()).then_some(leaving)
            })
            .collect();
    }
}
