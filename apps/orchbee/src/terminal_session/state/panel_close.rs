use super::{
    AttachmentKey, NativeSessionState, PaneKey, PanelIntent, PanelOperation, PanelRequestKind,
    SessionNotice, SessionPhase,
};
use crate::terminal_session::workbench_projection::NativeTerminalBinding;
use orchbee_workbench_protocol::{
    SiteId, WorkbenchLabel, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PanelCloseConfirmationToken(u64);

/// The exact leaf a panel-close confirmation was armed against; the confirm path
/// refuses to execute against anything else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct PanelCloseTarget {
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
pub(in crate::terminal_session) struct PanelCloseSelection {
    target: PanelCloseTarget,
    desired: AttachmentKey,
    panel_label: WorkbenchLabel,
    tab_label: WorkbenchLabel,
    binding: NativeTerminalBinding,
    dissolves_tab: bool,
    base_revision: WorkbenchRevision,
}

impl PanelCloseSelection {
    const fn new(
        target: PanelCloseTarget,
        desired: AttachmentKey,
        panel_label: WorkbenchLabel,
        tab_label: WorkbenchLabel,
        binding: NativeTerminalBinding,
        dissolves_tab: bool,
        base_revision: WorkbenchRevision,
    ) -> Self {
        Self {
            target,
            desired,
            panel_label,
            tab_label,
            binding,
            dissolves_tab,
            base_revision,
        }
    }
}

struct PanelCloseConfirmation {
    token: PanelCloseConfirmationToken,
    selection: PanelCloseSelection,
    shown: bool,
}

#[derive(Clone, Copy)]
pub(in crate::terminal_session) struct PanelCloseConfirmationView<'a> {
    confirmation: &'a PanelCloseConfirmation,
}

impl<'a> PanelCloseConfirmationView<'a> {
    pub(in crate::terminal_session) const fn token(&self) -> PanelCloseConfirmationToken {
        self.confirmation.token
    }
    pub(in crate::terminal_session) fn panel_label(&self) -> &'a str {
        self.confirmation.selection.panel_label.as_str()
    }
    pub(in crate::terminal_session) fn tab_label(&self) -> &'a str {
        self.confirmation.selection.tab_label.as_str()
    }
    /// Whether closing this leaf removes its entire Tab for every attached Client.
    pub(in crate::terminal_session) const fn dissolves_tab(&self) -> bool {
        self.confirmation.selection.dissolves_tab
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum PanelCloseStateError {
    Busy,
    NotDisplayed,
    StaleConfirmation,
}

#[derive(Default)]
pub(in crate::terminal_session) struct PanelCloseState {
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

impl NativeSessionState {
    pub(in crate::terminal_session) fn open_panel_close_confirmation(
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
                SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
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
        let selection = PanelCloseSelection::new(
            PanelCloseTarget::new(
                self.cache.site_id().clone(),
                self.cache.terminal_context_id().clone(),
                tab.tab_id().clone(),
                panel.panel_id().clone(),
            ),
            self.desired
                .as_ref()
                .map(|key| key.anchor().clone())
                .ok_or(PanelCloseStateError::Busy)?,
            panel.presentation_label().clone(),
            tab.tab_label().clone(),
            panel.binding().clone(),
            tab.panel_count() == 1,
            self.cache.revision(),
        );
        self.panel_close.open_confirmation(selection)
    }

    pub(in crate::terminal_session) fn cancel_panel_close_confirmation(&mut self) {
        self.panel_close.cancel_confirmation();
    }

    pub(in crate::terminal_session) fn acknowledge_panel_close_draw(
        &mut self,
        prompt: Option<PanelCloseConfirmationToken>,
    ) {
        self.panel_close.acknowledge_draw(prompt);
    }

    pub(in crate::terminal_session) fn invalidate_panel_close_display(&mut self) {
        self.panel_close.invalidate_display();
    }

    /// Whether the frozen selection still matches live Session identity: same
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
            && self.cache.terminal_context_id() == &selection.target.terminal_context_id
            && self.cache.revision() == selection.base_revision
            && self.desired.as_ref().map(PaneKey::anchor) == Some(&selection.desired)
            && tab.tab_id() == &selection.target.tab_id
            && panel.panel_id() == &selection.target.panel_id
            && panel.binding() == &selection.binding
            && (tab.panel_count() == 1) == selection.dissolves_tab
    }

    /// Whether the armed confirmation's frozen leaf no longer matches live
    /// Session identity; the UI settles a stale armed capture as cancelled
    /// rather than leaving it armed-but-stale swallowing keys.
    pub(super) fn armed_panel_close_stale(&self) -> bool {
        self.panel_close
            .confirmation
            .as_ref()
            .is_some_and(|confirmation| {
                !self.panel_close_selection_matches(&confirmation.selection)
            })
    }

    /// A draw receipt reporting that the confirmation could not be presented
    /// cancels the capture with a notice instead of leaving it armed.
    pub(in crate::terminal_session) fn settle_panel_close_not_presented(&mut self) {
        if self.panel_close.confirmation.is_some() {
            self.panel_close.cancel_confirmation();
            self.set_notice(SessionNotice::ConfirmationCancelled);
        }
    }

    /// The admitted close always names the frozen leaf: the live Session must
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

    pub(in crate::terminal_session) fn plan_panel_close_confirmation(
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
    pub(in crate::terminal_session) fn begin_panel_close_exchange(
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

impl<'a> super::NativeSessionView<'a> {
    pub(in crate::terminal_session) fn panel_close_confirmation(
        &self,
    ) -> Option<PanelCloseConfirmationView<'a>> {
        self.state.panel_close.confirmation()
    }
}
