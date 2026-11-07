//! Adapts Panel close input and draw receipts to the exact confirmation token before queueing
//! a captured close plan; a confirmed close that finds its slot taken waits for it as a held
//! intent. It does not paint the prompt or grant closure from an unpresented or stale
//! confirmation.

use super::held_intent::{CloseSubmission, ConfirmedClose};
use super::{ActionBuildError, ModalPresentation, WorkbenchClientUi, WorkbenchClientView};
use crate::workbench_state::{self, OPERATION_TOTAL, WorkbenchClientState};
use crate::{
    client_view::{ClientActionDisposition, ClientKey},
    workbench_client::mailbox::MoveAdmissionError,
};
use std::sync::Arc;

pub(crate) use crate::workbench_state::PanelCloseConfirmationToken;

#[derive(Clone, Copy)]
pub(crate) struct PanelCloseConfirmationView<'a> {
    inner: workbench_state::PanelCloseConfirmationView<'a>,
    impact: crate::localization::CloseImpact,
    tab: Option<super::ClientTabView<'a>>,
    panel_name: Option<crate::terminal_title::ChromeName<'a>>,
}

impl<'a> PanelCloseConfirmationView<'a> {
    /// What closing ends, from the terminal facts this Client holds at this draw.
    pub(crate) const fn impact(&self) -> crate::localization::CloseImpact {
        self.impact
    }
    pub(crate) const fn token(&self) -> PanelCloseConfirmationToken {
        self.inner.token()
    }
    pub(crate) fn panel_label(
        &self,
        locale: crate::localization::Locale,
    ) -> std::borrow::Cow<'a, str> {
        if self.tab.is_some() {
            super::panel_label(self.panel_name, self.inner.panel_position(), locale)
        } else {
            self.inner.panel_label().into()
        }
    }
    pub(crate) fn tab_label(
        &self,
        locale: crate::localization::Locale,
    ) -> std::borrow::Cow<'a, str> {
        super::live_tab_label(self.tab, self.inner.tab_label(), locale)
    }
    pub(crate) const fn panel_position(&self) -> usize {
        self.inner.panel_position()
    }
    pub(crate) const fn dissolves_tab(&self) -> bool {
        self.inner.dissolves_tab()
    }
    pub(crate) const fn interactive(&self) -> bool {
        self.inner.interactive()
    }
    pub(crate) const fn presentation(&self) -> crate::tui_presentation::PresentationPhase {
        self.inner.presentation()
    }
}

impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn panel_close_confirmation(&self) -> Option<PanelCloseConfirmationView<'a>> {
        let state = self.state.as_ref()?;
        if state.panel_close_deciding() {
            return None;
        }
        let inner = state.panel_close_confirmation()?;
        let impact = state.panel_close_impact(&|pane| self.pane_settled(pane))?;
        let tab = self.tab_by_id(inner.tab_id());
        let panel_name = tab.as_ref().and_then(|tab| {
            let panel = (0..tab.panel_count())
                .filter_map(|index| tab.panel(index))
                .find(|panel| panel.panel_id() == inner.panel_id())?;
            self.panel_name(&tab.inner, panel)
        });
        Some(PanelCloseConfirmationView {
            inner,
            impact,
            tab,
            panel_name,
        })
    }
}

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn click_panel_close(
        &mut self,
        token: PanelCloseConfirmationToken,
        confirm: bool,
    ) {
        if self
            .state
            .as_ref()
            .and_then(|state| state.view().panel_close_confirmation())
            .is_some_and(|prompt| prompt.token() == token && prompt.interactive())
        {
            self.capture_panel_confirmation_key(
                token,
                if confirm {
                    ClientKey::Enter
                } else {
                    ClientKey::Escape
                },
            );
        }
    }
    pub(super) fn panel_close_confirmation_armed(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| state.view().panel_close_confirmation().is_some())
    }

    pub(in crate::workbench_client) fn request_panel_close_confirmation(
        &mut self,
    ) -> ClientActionDisposition {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, WorkbenchClientState::open_panel_close_confirmation)
            .is_err()
        {
            return ClientActionDisposition::KeepNavigator;
        }
        // The Server decides whether closing the Panel asks first.
        if let Some(token) = state
            .view()
            .panel_close_confirmation()
            .map(|confirmation| confirmation.token())
        {
            self.begin_idle_close(super::idle_close::Target::Panel(token));
        }
        ClientActionDisposition::DismissNavigator
    }

    pub(in crate::workbench_client) fn cancel_panel_close(&mut self) -> ClientActionDisposition {
        self.release_held_close();
        if let Some(state) = &mut self.state {
            state.cancel_panel_close_confirmation();
        }
        ClientActionDisposition::KeepNavigator
    }

    /// Confirms the drawn prompt `token`. Another close's status read or exchange, or a refresh,
    /// can hold the slot the close needs; the close then waits for it with its prompt open.
    fn confirm_panel_close(&mut self, token: PanelCloseConfirmationToken) {
        let _disposition = self.confirm_close_decision(ConfirmedClose::Panel(token));
    }

    /// Submits the armed confirmation's close once. Any refusal but a closed mailbox is the slot
    /// being taken: planning refuses a confirmation that is not drawn, and the caller settles a
    /// drifted one before submitting.
    pub(super) fn submit_panel_close(&mut self) -> CloseSubmission {
        let Some(deadline) = self.clock.now().checked_add(OPERATION_TOTAL) else {
            return CloseSubmission::Ended;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return CloseSubmission::Ended;
        };
        let result = mailbox.try_enqueue_panel_for(
            state,
            deadline,
            &self.clock,
            |state, request_id| {
                state
                    .plan_panel_close_confirmation(request_id, deadline)
                    .map_err(|_| ActionBuildError::Busy)
            },
            |state, operation| {
                state
                    .begin_panel_close_exchange(operation)
                    .map_err(|_| ActionBuildError::Busy)
            },
        );
        match result {
            Ok(_) => CloseSubmission::Submitted,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                CloseSubmission::Ended
            }
            Err(_) => CloseSubmission::Waiting,
        }
    }

    /// The armed panel-close confirmation owns input: only unmodified Enter
    /// decides confirm and only `Esc` decides cancel; every other key is a
    /// swallowed non-decision. Returns `false` while no panel-close is armed.
    /// A confirmation whose frozen target drifted is settled as cancelled with
    /// a notice before any key is decided.
    pub(in crate::workbench_client) fn capture_panel_confirmation_key(
        &mut self,
        expected: PanelCloseConfirmationToken,
        key: ClientKey,
    ) -> bool {
        let Some((actual, interactive)) = self
            .state
            .as_ref()
            .and_then(|state| state.view().panel_close_confirmation())
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
                self.cancel_panel_close();
            }
            return true;
        }
        match key {
            ClientKey::Enter => {
                self.confirm_panel_close(expected);
            }
            ClientKey::Escape => {
                self.cancel_panel_close();
            }
            ClientKey::ControlB | ClientKey::Plain(_) | ClientKey::Other => {}
        }
        true
    }

    pub(in crate::workbench_client) fn after_panel_close_draw(
        &mut self,
        prompt: Option<ModalPresentation<PanelCloseConfirmationToken>>,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };
        match prompt {
            Some(ModalPresentation::Presented(token)) => {
                state.acknowledge_panel_close_draw(Some(token));
            }
            Some(ModalPresentation::NotPresented) => {
                state.acknowledge_panel_close_draw(None);
                let _ = state.settle_panel_close_not_presented();
            }
            None => state.acknowledge_panel_close_draw(None),
        }
    }

    pub(in crate::workbench_client) fn invalidate_panel_close_display(&mut self) {
        if let Some(state) = &mut self.state {
            state.invalidate_panel_close_display();
        }
    }
}
