//! Adapts close confirmations, explicit resumes, retained status reads, and draw receipts to
//! Client state and mailbox admission. It observes Server close truth without painting the
//! prompt, executing physical closure, or acknowledging a merely prepared frame.

use super::{PanelCloseConfirmationToken, WorkbenchClientUi, WorkbenchClientView};
use crate::client_view::{ClientActionDisposition, ClientKey};
use crate::localization::CloseImpact;
use crate::workbench_client::mailbox::{
    CloseAdmissionError, CommandMailbox, MutationResult, UiReply, UiWaitKey,
};
use crate::workbench_state::{self, TabCloseResumePermit};
use crate::workbench_state::{
    CarrierFailure, CloseOperation, CloseResult, OPERATION_TOTAL, WorkbenchClientState,
};
use std::sync::Arc;

pub use crate::workbench_state::CloseConfirmationToken;
pub(crate) use crate::workbench_state::{CloseConfirmationKind, CloseStatusKind, CloseStatusToken};

/// The renderer's geometry verdict for one modal surface in a completed draw.
/// `Presented` proves the complete surface was written. `NotPresented` means
/// only that the renderer evaluated this surface and the offered geometry could
/// not present it; the owning UI state decides whether that is a first-draw
/// refusal or a temporary suspension. An absent receipt means the surface was
/// absent or covered and revokes draw authority without making a geometry
/// verdict. Rendering itself never mutates UI state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModalPresentation<T> {
    Presented(T),
    NotPresented,
}

impl<T> ModalPresentation<T> {
    pub(crate) fn into_presented(self) -> Option<T> {
        match self {
            Self::Presented(token) => Some(token),
            Self::NotPresented => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DrawReceipt {
    settings: Option<ModalPresentation<crate::native_settings::SettingsDraw>>,
    menu: Option<ModalPresentation<crate::action_menu::MenuDraw>>,
    position: Option<crate::startup_preference::Draw>,
    prompt: Option<ModalPresentation<CloseConfirmationToken>>,
    status: Option<CloseStatusToken>,
    rename: Option<ModalPresentation<super::RenameDraftToken>>,
    setup: Option<ModalPresentation<super::SetupToken>>,
    worktree: Option<ModalPresentation<super::WorktreeToken>>,
    panel_prompt: Option<ModalPresentation<PanelCloseConfirmationToken>>,
    workspace_prompt: Option<ModalPresentation<super::WorkspaceCloseToken>>,
    /// The Agent board's exact-episode draw verdict. `Presented` carries the
    /// successful viewport proof; `NotPresented` reports evaluated geometry.
    /// `None` revokes current proof because the board was absent or covered.
    agent_board: Option<super::AgentBoardDraw>,
    attention: Option<crate::workbench_client::agent_attention::AgentAttentionDraw>,
    graphics: Vec<crate::workbench_client::PaneGraphicsDraw>,
}

impl DrawReceipt {
    pub(crate) fn with_settings(
        mut self,
        draw: Option<ModalPresentation<crate::native_settings::SettingsDraw>>,
    ) -> Self {
        self.settings = draw;
        self
    }
    pub(crate) fn with_menu(
        mut self,
        draw: Option<ModalPresentation<crate::action_menu::MenuDraw>>,
    ) -> Self {
        self.menu = draw;
        self
    }
    pub(crate) fn position(&self) -> Option<&crate::startup_preference::Draw> {
        self.position.as_ref()
    }
    pub fn with_position(mut self, position: Option<crate::startup_preference::Draw>) -> Self {
        self.position = position;
        self
    }
    pub fn graphics_draws(&self) -> &[crate::workbench_client::PaneGraphicsDraw] {
        &self.graphics
    }
    pub fn with_graphics(
        mut self,
        graphics: Vec<crate::workbench_client::PaneGraphicsDraw>,
    ) -> Self {
        self.graphics = graphics;
        self
    }
    pub fn with_attention(
        mut self,
        attention: Option<crate::workbench_client::agent_attention::AgentAttentionDraw>,
    ) -> Self {
        self.attention = attention;
        self
    }
    pub(crate) fn with_agent_board(mut self, agent_board: Option<super::AgentBoardDraw>) -> Self {
        self.agent_board = agent_board;
        self
    }
    pub(crate) const fn with_worktree(
        mut self,
        worktree: Option<ModalPresentation<super::WorktreeToken>>,
    ) -> Self {
        self.worktree = worktree;
        self
    }

    pub(crate) const fn with_setup(
        mut self,
        setup: Option<ModalPresentation<super::SetupToken>>,
    ) -> Self {
        self.setup = setup;
        self
    }
    pub(crate) const fn new(
        prompt: Option<ModalPresentation<CloseConfirmationToken>>,
        status: Option<CloseStatusToken>,
    ) -> Self {
        Self {
            position: None,
            prompt,
            status,
            rename: None,
            setup: None,
            worktree: None,
            menu: None,
            settings: None,
            panel_prompt: None,
            workspace_prompt: None,
            agent_board: None,
            attention: None,
            graphics: Vec::new(),
        }
    }
    pub(crate) const fn with_rename(
        mut self,
        rename: Option<ModalPresentation<super::RenameDraftToken>>,
    ) -> Self {
        self.rename = rename;
        self
    }
    pub(crate) const fn with_panel_close(
        mut self,
        panel_prompt: Option<ModalPresentation<PanelCloseConfirmationToken>>,
    ) -> Self {
        self.panel_prompt = panel_prompt;
        self
    }
    pub(crate) const fn with_workspace_close(
        mut self,
        prompt: Option<ModalPresentation<super::WorkspaceCloseToken>>,
    ) -> Self {
        self.workspace_prompt = prompt;
        self
    }
    /// Whether this draw presented an editable text field and therefore placed
    /// the terminal cursor inside it — the exact assertion a renderer test can
    /// make without re-deriving modal layout.
    #[cfg(test)]
    pub(crate) fn editing_cursor_shown(&self) -> bool {
        matches!(self.rename, Some(ModalPresentation::Presented(_)))
            || matches!(self.setup, Some(ModalPresentation::Presented(_)))
    }
}

/// One Tab of a batch close: its captured position and name, and the live Tab if it still exists.
type SetEntry<'a> = (usize, &'a str, Option<super::ClientTabView<'a>>);

pub struct CloseConfirmationView<'a> {
    inner: workbench_state::CloseConfirmationView<'a>,
    impact: CloseImpact,
    tab: Option<super::ClientTabView<'a>>,
    set: Vec<SetEntry<'a>>,
    resume: Option<CloseStatusView<'a>>,
}

impl<'a> CloseConfirmationView<'a> {
    /// What closing ends, from the terminal facts this Client holds at this draw.
    pub(crate) const fn impact(&self) -> CloseImpact {
        self.impact
    }
    pub(crate) fn set_count(&self) -> Option<usize> {
        self.inner.set().map(|set| set.count())
    }
    pub(crate) fn set_context_label(&self) -> Option<&'a str> {
        self.inner.set().map(|set| set.context_label())
    }
    /// The batch's Tab names, each as the Tab strip names its live Tab.
    pub(crate) fn set_labels(
        &self,
        locale: crate::localization::Locale,
    ) -> Option<Vec<(usize, std::borrow::Cow<'a, str>)>> {
        self.inner.set()?;
        Some(
            self.set
                .iter()
                .map(|&(position, captured, tab)| {
                    (position, super::live_tab_label(tab, captured, locale))
                })
                .collect(),
        )
    }
    #[cfg(test)]
    pub(crate) fn captured_set_labels(&self) -> Option<&'a [(usize, String)]> {
        self.inner.set().map(|set| set.labels())
    }
    pub(crate) const fn token(&self) -> CloseConfirmationToken {
        self.inner.token()
    }
    pub(crate) fn target_label(
        &self,
        locale: crate::localization::Locale,
    ) -> std::borrow::Cow<'a, str> {
        super::live_tab_label(self.tab, self.inner.target_label(), locale)
    }
    pub(crate) const fn kind(&self) -> CloseConfirmationKind {
        self.inner.kind()
    }
    pub(crate) const fn interactive(&self) -> bool {
        self.inner.interactive()
    }
    pub(crate) const fn presentation(&self) -> crate::tui_presentation::PresentationPhase {
        self.inner.presentation()
    }
    pub(crate) fn resume_status(&self) -> Option<CloseStatusView<'a>> {
        self.resume
    }
    pub(crate) fn resume_set_status(&self) -> Option<crate::workbench_state::CloseSetStatus> {
        self.inner.resume_set_status()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CloseStatusView<'a> {
    inner: workbench_state::CloseStatusView<'a>,
    tab: Option<super::ClientTabView<'a>>,
}

impl<'a> CloseStatusView<'a> {
    pub(crate) const fn token(&self) -> CloseStatusToken {
        self.inner.token()
    }
    pub(crate) fn target_label(
        &self,
        locale: crate::localization::Locale,
    ) -> std::borrow::Cow<'a, str> {
        super::live_tab_label(self.tab, self.inner.target_label(), locale)
    }
    pub(crate) fn status(&self) -> CloseStatusKind {
        self.inner.status()
    }
    pub(crate) fn resumable(&self) -> bool {
        self.inner.resumable()
    }
    pub(crate) fn observation_paused(&self) -> bool {
        self.inner.observation_paused()
    }
    pub(crate) const fn unavailable(&self) -> bool {
        self.inner.unavailable()
    }
}

impl<'a> CloseStatusView<'a> {
    /// The status as text under an already resolved Tab name.
    pub(crate) fn text<'b>(&self, label: &'b str) -> crate::localization::CloseStatusText<'b> {
        crate::localization::CloseStatusText {
            label,
            kind: self.status(),
            resumable: self.resumable(),
            observation_paused: self.observation_paused(),
            unavailable: self.unavailable(),
        }
    }
}

#[cfg(test)]
impl<'a> From<&CloseStatusView<'a>> for crate::localization::CloseStatusText<'a> {
    fn from(status: &CloseStatusView<'a>) -> Self {
        Self {
            label: status.inner.target_label(),
            kind: status.status(),
            resumable: status.resumable(),
            observation_paused: status.observation_paused(),
            unavailable: status.unavailable(),
        }
    }
}

impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn close_set_status(&self) -> Option<crate::workbench_state::CloseSetStatus> {
        self.state.as_ref()?.close_set_status()
    }
    pub(crate) fn close_confirmation(&self) -> Option<CloseConfirmationView<'a>> {
        let state = self.state.as_ref()?;
        let inner = state.close_confirmation()?;
        let impact = state.close_impact(&inner, &|pane| self.pane_settled(pane));
        let tab = self.tab_by_id(inner.target_tab_id());
        let set = inner.set().map_or_else(Vec::new, |set| {
            set.entries()
                .map(|(position, label, id)| (position, label, self.tab_by_id(id)))
                .collect()
        });
        let resume = inner
            .resume_status()
            .map(|inner| self.close_status_view(inner));
        Some(CloseConfirmationView {
            inner,
            impact,
            tab,
            set,
            resume,
        })
    }
    pub(crate) fn close_status(&self) -> Option<CloseStatusView<'a>> {
        self.state
            .as_ref()?
            .close_status()
            .map(|inner| self.close_status_view(inner))
    }
    fn close_status_view(
        &self,
        inner: workbench_state::CloseStatusView<'a>,
    ) -> CloseStatusView<'a> {
        CloseStatusView {
            tab: self.tab_by_id(inner.target_tab_id()),
            inner,
        }
    }
}

impl WorkbenchClientUi {
    pub(super) fn request_close_set(
        &mut self,
        ids: &[orchbee_workbench_protocol::WorkbenchTabId],
    ) -> Option<()> {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return None;
        };
        mailbox
            .apply_local_for(state, |state| state.open_close_set(ids))
            .ok()?;
        Some(())
    }

    pub(super) fn schedule_close_set(&mut self, mailbox: &Arc<CommandMailbox>) -> bool {
        let Some(state) = self.state.as_mut() else {
            return false;
        };
        let changed = state.settle_close_set(self.clock.now());
        if !state.close_set_ready() {
            return changed;
        }
        let Some(deadline) = state.close_set_deadline() else {
            return changed;
        };
        let Some(operation_deadline) = self.clock.now().checked_add(OPERATION_TOTAL) else {
            return changed;
        };
        let deadline = deadline.min(operation_deadline);
        match mailbox.try_enqueue_close_for(
            state,
            deadline,
            &self.clock,
            |state, request, _| state.plan_next_close_set(request, deadline),
            WorkbenchClientState::begin_next_close_set,
        ) {
            Ok(_) => true,
            Err(CloseAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                true
            }
            Err(_) => changed,
        }
    }
    pub(in crate::workbench_client) fn close_worker_loss(
        &self,
    ) -> crate::workbench_state::CloseWorkerReturn {
        self.mailbox.as_ref().map_or(
            crate::workbench_state::CloseWorkerReturn::Missing {
                queued: None,
                deposited: None,
            },
            |mailbox| mailbox.close_worker_loss(),
        )
    }

    #[cfg(test)]
    pub(in crate::workbench_client) fn settled_close_identity(
        &self,
    ) -> Option<&crate::workbench_state::CloseFlightKey> {
        self.state.as_ref()?.settled_close_identity()
    }

    #[cfg(test)]
    pub(in crate::workbench_client) fn uncertain_close_identity(
        &self,
    ) -> Option<(
        &crate::workbench_state::CloseFlightKey,
        Option<&orchbee_workbench_protocol::WorkbenchRequestId>,
    )> {
        self.state.as_ref()?.uncertain_close_identity()
    }
    pub(in crate::workbench_client) fn begin_status_recovery(
        &mut self,
    ) -> Option<(
        Arc<CommandMailbox>,
        crate::local_client::LocalOperationClock,
    )> {
        if !self.retirement_finalized || self.mailbox.is_some() {
            return None;
        }
        let state = self.state.as_mut()?;
        let seed = self.retired_mailbox_seed?;
        if !state.begin_close_status_recovery() {
            return None;
        }
        let mailbox = Arc::new(CommandMailbox::for_status_recovery(
            seed,
            state.latest_geometry(),
        ));
        self.mailbox = Some(Arc::clone(&mailbox));
        self.retirement_failure = None;
        self.retirement_finalized = false;
        self.pending_draws.clear();
        self.ordinary_poll_serial = None;
        Some((mailbox, self.clock.clone()))
    }

    pub(in crate::workbench_client) fn click_tab_close(
        &mut self,
        token: CloseConfirmationToken,
        confirm: bool,
    ) {
        if self
            .state
            .as_ref()
            .and_then(|state| state.view().close_confirmation())
            .is_some_and(|prompt| prompt.token() == token && prompt.interactive())
        {
            self.capture_tab_confirmation_key(
                token,
                if confirm {
                    ClientKey::Enter
                } else {
                    ClientKey::Escape
                },
            );
        }
    }

    #[cfg(test)]
    pub(in crate::workbench_client) fn capture_confirmation_key(&mut self, key: ClientKey) -> bool {
        if let Some(token) = self
            .state
            .as_ref()
            .and_then(WorkbenchClientState::workspace_close_confirmation)
            .map(|prompt| prompt.token())
            && self.capture_workspace_close_key(token, key)
        {
            return true;
        }
        if let Some(token) = self
            .state
            .as_ref()
            .and_then(|state| state.view().panel_close_confirmation())
            .map(|prompt| prompt.token())
            && self.capture_panel_confirmation_key(token, key)
        {
            return true;
        }
        let Some(token) = self
            .state
            .as_ref()
            .and_then(|state| state.view().close_confirmation())
            .map(|prompt| prompt.token())
        else {
            return false;
        };
        self.capture_tab_confirmation_key(token, key)
    }

    pub(in crate::workbench_client) fn capture_tab_confirmation_key(
        &mut self,
        expected: CloseConfirmationToken,
        key: ClientKey,
    ) -> bool {
        let Some((actual, interactive)) = self
            .state
            .as_ref()
            .and_then(|state| state.view().close_confirmation())
            .map(|prompt| (prompt.token(), prompt.interactive()))
        else {
            return false;
        };
        if actual != expected {
            return false;
        }
        if self
            .state
            .as_mut()
            .is_some_and(|state| state.settle_stale_close_confirmations())
        {
            return true;
        }
        if !interactive {
            if key == ClientKey::Escape {
                self.apply_client_action(crate::client_view::ClientAction::CancelClose);
            }
            return true;
        }
        match key {
            ClientKey::Enter => {
                self.apply_client_action(crate::client_view::ClientAction::ConfirmClose);
            }
            ClientKey::Escape => {
                self.apply_client_action(crate::client_view::ClientAction::CancelClose);
            }
            ClientKey::ControlB | ClientKey::Plain(_) | ClientKey::Other => {}
        }
        true
    }

    pub(super) fn request_close_confirmation(&mut self) -> ClientActionDisposition {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, WorkbenchClientState::open_close_confirmation)
            .is_ok()
        {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    pub(super) fn request_tab_close_resume(
        &mut self,
        tab_id: &orchbee_workbench_protocol::WorkbenchTabId,
    ) -> ClientActionDisposition {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, |state| state.open_tab_close_resume(tab_id))
            .is_ok()
        {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    pub(super) fn request_tab_close_resume_with_permit(
        &mut self,
        tab_id: &orchbee_workbench_protocol::WorkbenchTabId,
        permit: &TabCloseResumePermit,
    ) -> ClientActionDisposition {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, |state| {
                state.open_tab_close_resume_with_permit(tab_id, permit)
            })
            .is_ok()
        {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    pub(super) fn cancel_close(&mut self) -> ClientActionDisposition {
        if let Some(state) = &mut self.state {
            state.cancel_close_confirmation();
        }
        ClientActionDisposition::KeepNavigator
    }

    pub(super) fn confirm_close(&mut self) -> ClientActionDisposition {
        let Some(deadline) = self.clock.now().checked_add(OPERATION_TOTAL) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_close_for(
            state,
            deadline,
            &self.clock,
            |state, request_id, _| state.plan_close_confirmation(request_id, deadline),
            WorkbenchClientState::begin_close_exchange,
        );
        match result {
            Ok(_) => ClientActionDisposition::DismissNavigator,
            Err(CloseAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                ClientActionDisposition::KeepNavigator
            }
            Err(_) => ClientActionDisposition::KeepNavigator,
        }
    }

    pub(super) fn schedule_close_read(&mut self, mailbox: &Arc<CommandMailbox>) {
        let sampled = self.clock.now();
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if !state.has_close_read_candidate(sampled) {
            return;
        }
        let Some(deadline) = sampled.checked_add(OPERATION_TOTAL) else {
            return;
        };
        let deadline = state
            .close_set_deadline()
            .map_or(deadline, |sequence| sequence.min(deadline));
        let result = mailbox.try_enqueue_close_for(
            state,
            deadline,
            &self.clock,
            |state, request_id, admitted_at| {
                state.plan_close_read(request_id, deadline, admitted_at)
            },
            WorkbenchClientState::begin_close_exchange,
        );
        if let Err(CloseAdmissionError::Closed(closed)) = result {
            self.retire(closed.failure());
        }
    }

    pub(super) fn apply_close_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait_key: UiWaitKey,
        operation: CloseOperation,
        outcome: CloseResult,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let result = mailbox.deposit_ui_reply_before(&wait_key, &self.clock, |_| {
            state
                .settle_close_exchange(&operation, &outcome)
                .map_err(|_| CarrierFailure::AuthorityDivergence)?;
            Ok(UiReply::Close(operation.key().clone()))
        });
        match result {
            Ok(MutationResult::Applied) => self.validate_state(),
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }

    pub(in crate::workbench_client) fn after_close_draw(&mut self, receipt: DrawReceipt) {
        self.after_action_menu_draw(receipt.menu);
        self.after_settings_draw(receipt.settings);
        self.attention.after_draw();
        self.after_graphics_draw(receipt.graphics);
        if let Some(state) = &mut self.state {
            state.acknowledge_close_draw(
                receipt.prompt.and_then(ModalPresentation::into_presented),
                receipt.status,
            );
            if matches!(receipt.prompt, Some(ModalPresentation::NotPresented)) {
                let _ = state.settle_close_confirmation_not_presented();
            }
        }
        self.after_panel_close_draw(receipt.panel_prompt);
        self.after_workspace_close_draw(receipt.workspace_prompt);
        self.after_rename_draw(receipt.rename);
        self.after_setup_draw(receipt.setup);
        self.after_worktree_draw(receipt.worktree);
        self.after_agent_board_draw(receipt.agent_board);
        if let Some(draw) = receipt.attention {
            self.after_attention_draw(draw);
        }
    }
    pub(in crate::workbench_client) fn invalidate_close_display(&mut self) {
        if let Some(state) = &mut self.state {
            state.invalidate_close_display();
            state.acknowledge_workspace_close_draw(None);
        }
        self.invalidate_panel_close_display();
    }
}
