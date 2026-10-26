use super::{ActionBuildError, ModalPresentation, WorkbenchClientUi, WorkbenchClientView};
use crate::workbench_client::state::{
    OPERATION_TOTAL, PanelCloseConfirmationView, WorkbenchClientState,
};
use crate::{
    client_view::{ClientActionDisposition, ClientKey},
    workbench_client::mailbox::MoveAdmissionError,
};
use std::sync::Arc;

pub(crate) use crate::workbench_client::state::PanelCloseConfirmationToken;

#[derive(Clone, Copy)]
pub(crate) struct PanelCloseConfirmationView<'a> {
    inner: PanelCloseConfirmationView<'a>,
}

impl<'a> PanelCloseConfirmationView<'a> {
    pub(crate) const fn token(&self) -> PanelCloseConfirmationToken {
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
    pub(crate) const fn interactive(&self) -> bool {
        self.inner.interactive()
    }
    pub(crate) const fn presentation(&self) -> crate::tui_presentation::PresentationPhase {
        self.inner.presentation()
    }
}

impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn panel_close_confirmation(&self) -> Option<PanelCloseConfirmationView<'a>> {
        self.state
            .as_ref()?
            .panel_close_confirmation()
            .map(|inner| PanelCloseConfirmationView { inner })
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
            .is_ok()
        {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    pub(in crate::workbench_client) fn cancel_panel_close(&mut self) -> ClientActionDisposition {
        if let Some(state) = &mut self.state {
            state.cancel_panel_close_confirmation();
        }
        ClientActionDisposition::KeepNavigator
    }

    pub(in crate::workbench_client) fn confirm_panel_close(&mut self) -> ClientActionDisposition {
        let Some(deadline) = self.clock.now().checked_add(OPERATION_TOTAL) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
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
            Ok(_) => ClientActionDisposition::DismissNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                ClientActionDisposition::KeepNavigator
            }
            Err(_) => ClientActionDisposition::KeepNavigator,
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
                self.confirm_panel_close();
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
