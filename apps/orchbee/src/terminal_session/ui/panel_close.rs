use super::{ActionBuildError, ModalPresentation, NativeSessionView, NativeUi};
use crate::terminal_session::state::{
    NativeSessionState, OPERATION_TOTAL, PanelCloseConfirmationView,
};
use crate::{
    client_view::{ClientKey, SessionActionDisposition},
    terminal_session::mailbox::MoveAdmissionError,
};
use std::sync::Arc;

pub(crate) use crate::terminal_session::state::PanelCloseConfirmationToken as NativePanelCloseConfirmationToken;

#[derive(Clone, Copy)]
pub(crate) struct NativePanelCloseConfirmationView<'a> {
    inner: PanelCloseConfirmationView<'a>,
}

impl<'a> NativePanelCloseConfirmationView<'a> {
    pub(crate) const fn token(&self) -> NativePanelCloseConfirmationToken {
        self.inner.token()
    }
    pub(crate) fn panel_label(&self) -> &'a str {
        self.inner.panel_label()
    }
    pub(crate) fn tab_label(&self) -> &'a str {
        self.inner.tab_label()
    }
    pub(crate) const fn panel_position(&self) -> usize {
        self.inner.panel_position()
    }
    pub(crate) const fn tab_position(&self) -> usize {
        self.inner.tab_position()
    }
    pub(crate) const fn dissolves_tab(&self) -> bool {
        self.inner.dissolves_tab()
    }
}

impl<'a> NativeSessionView<'a> {
    pub(crate) fn panel_close_confirmation(&self) -> Option<NativePanelCloseConfirmationView<'a>> {
        self.state
            .as_ref()?
            .panel_close_confirmation()
            .map(|inner| NativePanelCloseConfirmationView { inner })
    }
}

impl NativeUi {
    pub(in crate::terminal_session) fn click_panel_close(
        &mut self,
        token: NativePanelCloseConfirmationToken,
        confirm: bool,
    ) {
        if self
            .state
            .as_ref()
            .and_then(|state| state.view().panel_close_confirmation())
            .is_some_and(|prompt| prompt.token() == token)
        {
            self.capture_panel_confirmation_key(if confirm {
                ClientKey::Plain('y')
            } else {
                ClientKey::Escape
            });
        }
    }
    pub(super) fn panel_close_confirmation_armed(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| state.view().panel_close_confirmation().is_some())
    }

    pub(in crate::terminal_session) fn request_panel_close_confirmation(
        &mut self,
    ) -> SessionActionDisposition {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, NativeSessionState::open_panel_close_confirmation)
            .is_ok()
        {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }

    pub(in crate::terminal_session) fn cancel_panel_close(&mut self) -> SessionActionDisposition {
        if let Some(state) = &mut self.state {
            state.cancel_panel_close_confirmation();
        }
        SessionActionDisposition::KeepNavigator
    }

    pub(in crate::terminal_session) fn confirm_panel_close(&mut self) -> SessionActionDisposition {
        let Some(deadline) = self.clock.now().checked_add(OPERATION_TOTAL) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
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
            Ok(_) => SessionActionDisposition::DismissNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                SessionActionDisposition::KeepNavigator
            }
            Err(_) => SessionActionDisposition::KeepNavigator,
        }
    }

    /// The armed panel-close confirmation owns input: only the exact lowercase `y`
    /// decides confirm and only `Esc` decides cancel; every other key is a
    /// swallowed non-decision. Returns `false` while no panel-close is armed.
    /// A confirmation whose frozen target drifted is settled as cancelled with
    /// a notice before any key is decided.
    pub(in crate::terminal_session) fn capture_panel_confirmation_key(
        &mut self,
        key: ClientKey,
    ) -> bool {
        if !self.panel_close_confirmation_armed() {
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
                self.confirm_panel_close();
            }
            ClientKey::Escape => {
                self.cancel_panel_close();
            }
            ClientKey::ControlB | ClientKey::Plain(_) | ClientKey::Other => {}
        }
        true
    }

    pub(in crate::terminal_session) fn after_panel_close_draw(
        &mut self,
        prompt: Option<ModalPresentation<NativePanelCloseConfirmationToken>>,
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
                state.settle_panel_close_not_presented();
            }
            None => state.acknowledge_panel_close_draw(None),
        }
    }

    pub(in crate::terminal_session) fn invalidate_panel_close_display(&mut self) {
        if let Some(state) = &mut self.state {
            state.invalidate_panel_close_display();
        }
    }
}
