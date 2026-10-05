use super::{NativePanelCloseConfirmationToken, NativeSessionView, NativeUi};
use crate::terminal_session::state::{CloseConfirmationView, CloseStatusView};
use crate::{
    client_view::{ClientKey, SessionActionDisposition},
    terminal_session::{
        mailbox::{CloseAdmissionError, CommandMailbox, MutationResult, UiReply, UiWaitKey},
        state::{CarrierFailure, CloseOperation, CloseResult, NativeSessionState, OPERATION_TOTAL},
    },
};
use std::sync::Arc;

pub(crate) use crate::terminal_session::state::{
    CloseConfirmationKind as NativeCloseConfirmationKind,
    CloseConfirmationToken as NativeCloseConfirmationToken,
    CloseStatusKind as NativeCloseStatusKind, CloseStatusToken as NativeCloseStatusToken,
};

/// The renderer's typed outcome for a modal surface inside the unified content
/// area: `Presented` when the complete content was drawn and carries the armed
/// token, `NotPresented` when the offered area could not present it. The UI
/// settles a `NotPresented` outcome after the draw — rendering never mutates
/// Session state itself.
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
pub(crate) struct NativeDrawReceipt {
    #[cfg(windows)]
    position: Option<crate::terminal_session::startup_preference::Draw>,
    prompt: Option<ModalPresentation<NativeCloseConfirmationToken>>,
    status: Option<NativeCloseStatusToken>,
    rename: Option<ModalPresentation<super::NativeRenameDraftToken>>,
    setup: Option<ModalPresentation<super::NativeSetupToken>>,
    #[cfg(windows)]
    worktree: Option<ModalPresentation<super::WorktreeToken>>,
    panel_prompt: Option<ModalPresentation<NativePanelCloseConfirmationToken>>,
    workspace_prompt: Option<ModalPresentation<super::WorkspaceCloseToken>>,
    /// The Agent board's painted verdict: `Some` while the board is open —
    /// `presented` gates its input capture and `viewport_rows` bounds its
    /// selection window. `None` means the board is closed.
    agent_board: Option<super::AgentBoardDraw>,
    /// The shortcut-help page the renderer actually presented, reported back
    /// to the ClientView through the draw receipt so the dispatcher's page
    /// state settles on the clamped value (render never mutates it).
    help_page: Option<usize>,
    #[cfg(windows)]
    attention: Option<crate::terminal_session::agent_attention::NativeAgentAttentionDraw>,
    #[cfg(windows)]
    graphics: Vec<crate::terminal_session::PaneGraphicsDraw>,
}

impl NativeDrawReceipt {
    #[cfg(windows)]
    pub(crate) fn position(&self) -> Option<&crate::terminal_session::startup_preference::Draw> {
        self.position.as_ref()
    }
    #[cfg(windows)]
    pub(crate) fn with_position(
        mut self,
        position: Option<crate::terminal_session::startup_preference::Draw>,
    ) -> Self {
        self.position = position;
        self
    }
    #[cfg(windows)]
    pub(crate) fn graphics_draws(&self) -> &[crate::terminal_session::PaneGraphicsDraw] {
        &self.graphics
    }
    #[cfg(windows)]
    pub(crate) fn with_graphics(
        mut self,
        graphics: Vec<crate::terminal_session::PaneGraphicsDraw>,
    ) -> Self {
        self.graphics = graphics;
        self
    }
    #[cfg(windows)]
    pub(crate) fn with_attention(
        mut self,
        attention: Option<crate::terminal_session::agent_attention::NativeAgentAttentionDraw>,
    ) -> Self {
        self.attention = attention;
        self
    }
    pub(crate) const fn help_page(&self) -> Option<usize> {
        self.help_page
    }
    pub(crate) const fn with_help_page(mut self, help_page: Option<usize>) -> Self {
        self.help_page = help_page;
        self
    }
    pub(crate) const fn with_agent_board(
        mut self,
        agent_board: Option<super::AgentBoardDraw>,
    ) -> Self {
        self.agent_board = agent_board;
        self
    }
    #[cfg(windows)]
    pub(crate) const fn with_worktree(
        mut self,
        worktree: Option<ModalPresentation<super::WorktreeToken>>,
    ) -> Self {
        self.worktree = worktree;
        self
    }

    pub(crate) const fn with_setup(
        mut self,
        setup: Option<ModalPresentation<super::NativeSetupToken>>,
    ) -> Self {
        self.setup = setup;
        self
    }
    pub(crate) const fn new(
        prompt: Option<ModalPresentation<NativeCloseConfirmationToken>>,
        status: Option<NativeCloseStatusToken>,
    ) -> Self {
        Self {
            #[cfg(windows)]
            position: None,
            prompt,
            status,
            rename: None,
            setup: None,
            #[cfg(windows)]
            worktree: None,
            panel_prompt: None,
            workspace_prompt: None,
            agent_board: None,
            help_page: None,
            #[cfg(windows)]
            attention: None,
            #[cfg(windows)]
            graphics: Vec::new(),
        }
    }
    pub(crate) const fn with_rename(
        mut self,
        rename: Option<ModalPresentation<super::NativeRenameDraftToken>>,
    ) -> Self {
        self.rename = rename;
        self
    }
    pub(crate) const fn with_panel_close(
        mut self,
        panel_prompt: Option<ModalPresentation<NativePanelCloseConfirmationToken>>,
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

pub(crate) struct NativeCloseConfirmationView<'a> {
    inner: CloseConfirmationView<'a>,
}

impl<'a> NativeCloseConfirmationView<'a> {
    pub(crate) const fn token(&self) -> NativeCloseConfirmationToken {
        self.inner.token()
    }
    pub(crate) fn target_label(&self) -> &'a str {
        self.inner.target_label()
    }
    pub(crate) const fn kind(&self) -> NativeCloseConfirmationKind {
        self.inner.kind()
    }
    pub(crate) fn resume_status(&self) -> Option<NativeCloseStatusView<'a>> {
        self.inner
            .resume_status()
            .map(|inner| NativeCloseStatusView { inner })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct NativeCloseStatusView<'a> {
    inner: CloseStatusView<'a>,
}

impl<'a> NativeCloseStatusView<'a> {
    pub(crate) const fn token(&self) -> NativeCloseStatusToken {
        self.inner.token()
    }
    pub(crate) fn target_label(&self) -> &'a str {
        self.inner.target_label()
    }
    pub(crate) fn status(&self) -> NativeCloseStatusKind {
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

impl<'a> NativeSessionView<'a> {
    pub(crate) fn close_confirmation(&self) -> Option<NativeCloseConfirmationView<'a>> {
        self.state
            .as_ref()?
            .close_confirmation()
            .map(|inner| NativeCloseConfirmationView { inner })
    }
    pub(crate) fn close_status(&self) -> Option<NativeCloseStatusView<'a>> {
        self.state
            .as_ref()?
            .close_status()
            .map(|inner| NativeCloseStatusView { inner })
    }
}

impl NativeUi {
    pub(in crate::terminal_session) fn close_worker_loss(
        &self,
    ) -> crate::terminal_session::state::CloseWorkerReturn {
        self.mailbox.as_ref().map_or(
            crate::terminal_session::state::CloseWorkerReturn::Missing {
                queued: None,
                deposited: None,
            },
            |mailbox| mailbox.close_worker_loss(),
        )
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn settled_close_identity(
        &self,
    ) -> Option<&crate::terminal_session::state::CloseFlightKey> {
        self.state.as_ref()?.settled_close_identity()
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn uncertain_close_identity(
        &self,
    ) -> Option<(
        &crate::terminal_session::state::CloseFlightKey,
        Option<&orchbee_workbench_protocol::WorkbenchRequestId>,
    )> {
        self.state.as_ref()?.uncertain_close_identity()
    }
    #[cfg(any(target_os = "linux", windows))]
    pub(in crate::terminal_session) fn begin_status_recovery(
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

    pub(in crate::terminal_session) fn capture_confirmation_key(&mut self, key: ClientKey) -> bool {
        if self.capture_workspace_close_key(key) {
            return true;
        }
        if self.capture_panel_confirmation_key(key) {
            return true;
        }
        if self
            .state
            .as_ref()
            .is_none_or(|state| state.view().close_confirmation().is_none())
        {
            return false;
        }
        if self
            .state
            .as_mut()
            .is_some_and(|state| state.settle_stale_close_confirmations())
        {
            return true;
        }
        match key {
            ClientKey::Plain('y') => {
                self.apply_client_action(crate::client_view::ClientAction::ConfirmClose);
            }
            ClientKey::Escape => {
                self.apply_client_action(crate::client_view::ClientAction::CancelClose);
            }
            ClientKey::ControlB | ClientKey::Plain(_) | ClientKey::Other => {}
        }
        true
    }

    pub(super) fn request_close_confirmation(&mut self) -> SessionActionDisposition {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, NativeSessionState::open_close_confirmation)
            .is_ok()
        {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }

    pub(super) fn cancel_close(&mut self) -> SessionActionDisposition {
        if let Some(state) = &mut self.state {
            state.cancel_close_confirmation();
        }
        SessionActionDisposition::KeepNavigator
    }

    pub(super) fn confirm_close(&mut self) -> SessionActionDisposition {
        let Some(deadline) = self.clock.now().checked_add(OPERATION_TOTAL) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_close_for(
            state,
            deadline,
            &self.clock,
            |state, request_id, _| state.plan_close_confirmation(request_id, deadline),
            NativeSessionState::begin_close_exchange,
        );
        match result {
            Ok(_) => SessionActionDisposition::DismissNavigator,
            Err(CloseAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                SessionActionDisposition::KeepNavigator
            }
            Err(_) => SessionActionDisposition::KeepNavigator,
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
        let result = mailbox.try_enqueue_close_for(
            state,
            deadline,
            &self.clock,
            |state, request_id, admitted_at| {
                state.plan_close_read(request_id, deadline, admitted_at)
            },
            NativeSessionState::begin_close_exchange,
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

    pub(in crate::terminal_session) fn after_close_draw(&mut self, receipt: NativeDrawReceipt) {
        #[cfg(windows)]
        self.attention.after_draw();
        #[cfg(windows)]
        self.after_graphics_draw(receipt.graphics);
        if let Some(state) = &mut self.state {
            state.acknowledge_close_draw(
                receipt.prompt.and_then(ModalPresentation::into_presented),
                receipt.status,
            );
            if matches!(receipt.prompt, Some(ModalPresentation::NotPresented)) {
                state.settle_close_confirmation_not_presented();
            }
        }
        self.after_panel_close_draw(receipt.panel_prompt);
        self.after_workspace_close_draw(receipt.workspace_prompt);
        self.after_rename_draw(receipt.rename);
        self.after_setup_draw(receipt.setup);
        #[cfg(windows)]
        self.after_worktree_draw(receipt.worktree);
        self.after_agent_board_draw(receipt.agent_board);
        #[cfg(windows)]
        if let Some(draw) = receipt.attention {
            self.after_attention_draw(draw);
        }
    }
    pub(in crate::terminal_session) fn invalidate_close_display(&mut self) {
        if let Some(state) = &mut self.state {
            state.invalidate_close_display();
            state.acknowledge_workspace_close_draw(None);
        }
        self.invalidate_panel_close_display();
    }
}
