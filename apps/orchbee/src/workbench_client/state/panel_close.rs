use super::{
    AttachmentKey, ClientPhase, PaneKey, PanelIntent, PanelOperation, PanelRequestKind,
    WorkbenchClientState,
};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use crate::workbench_client::workbench_projection::NativeTerminalBinding;
use orchbee_workbench_protocol::{
    SiteId, WorkbenchLabel, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PanelCloseConfirmationToken(u64);

impl PanelCloseConfirmationToken {
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

/// The exact leaf a panel-close confirmation was armed against; the confirm path
/// refuses to execute against anything else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct PanelCloseTarget {
    site_id: SiteId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
}

impl PanelCloseTarget {
    const fn new(
        site_id: SiteId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
    ) -> Self {
        Self {
            site_id,
            terminal_context_id,
            tab_id,
            panel_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct PanelCloseSelection {
    target: PanelCloseTarget,
    desired: AttachmentKey,
    panel_label: String,
    tab_label: WorkbenchLabel,
    panel_position: usize,
    tab_position: usize,
    binding: Option<NativeTerminalBinding>,
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
pub(in crate::workbench_client) struct PanelCloseConfirmationView<'a> {
    confirmation: &'a PanelCloseConfirmation,
}

impl<'a> PanelCloseConfirmationView<'a> {
    pub(in crate::workbench_client) const fn token(&self) -> PanelCloseConfirmationToken {
        self.confirmation.token
    }
    pub(in crate::workbench_client) fn panel_label(&self) -> &'a str {
        self.confirmation.selection.panel_label.as_str()
    }
    pub(in crate::workbench_client) fn tab_label(&self) -> &'a str {
        self.confirmation.selection.tab_label.as_str()
    }
    pub(in crate::workbench_client) const fn panel_position(&self) -> usize {
        self.confirmation.selection.panel_position
    }
    pub(in crate::workbench_client) const fn tab_position(&self) -> usize {
        self.confirmation.selection.tab_position
    }
    /// Whether closing this leaf removes its entire Tab for every attached Client.
    pub(in crate::workbench_client) const fn dissolves_tab(&self) -> bool {
        self.confirmation.selection.dissolves_tab
    }
    pub(in crate::workbench_client) const fn interactive(&self) -> bool {
        self.confirmation.shown && self.confirmation.presentation.interactive()
    }
    pub(in crate::workbench_client) const fn presentation(&self) -> PresentationPhase {
        self.confirmation.presentation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum PanelCloseStateError {
    Busy,
    NotDisplayed,
    StaleConfirmation,
}

#[derive(Default)]
pub(in crate::workbench_client) struct PanelCloseState {
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
    pub(in crate::workbench_client) fn open_panel_close_confirmation(
        &mut self,
    ) -> Result<(), PanelCloseStateError> {
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
                self.cache.site_id().clone(),
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

    pub(in crate::workbench_client) fn cancel_panel_close_confirmation(&mut self) {
        self.panel_close.cancel_confirmation();
    }

    pub(in crate::workbench_client) fn acknowledge_panel_close_draw(
        &mut self,
        prompt: Option<PanelCloseConfirmationToken>,
    ) {
        self.panel_close.acknowledge_draw(prompt);
    }

    pub(in crate::workbench_client) fn invalidate_panel_close_display(&mut self) {
        self.panel_close.invalidate_display();
    }

    /// Whether the frozen selection still matches live Client selection identity: same
    /// site, context, revision, desired attachment, selected Tab and panel id,
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
        self.cache.site_id() == &selection.target.site_id
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
    pub(in crate::workbench_client) fn settle_panel_close_not_presented(
        &mut self,
    ) -> Option<GeometrySettlement> {
        let confirmation = self.panel_close.confirmation.as_mut()?;
        confirmation.shown = false;
        let settlement = confirmation.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.panel_close.cancel_confirmation();
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

    pub(in crate::workbench_client) fn plan_panel_close_confirmation(
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

    /// The confirmation can only ever begin a `PanelIntent::Close`; `begin_panel`
    /// re-plans the operation and refuses any drift since the frozen selection.
    pub(in crate::workbench_client) fn begin_panel_close_exchange(
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
    pub(in crate::workbench_client) fn panel_close_confirmation(
        &self,
    ) -> Option<PanelCloseConfirmationView<'a>> {
        self.state.panel_close.confirmation()
    }
}
