use super::{ModalPresentation, NativeSessionView, NativeUi};
pub(crate) use crate::terminal_session::state::workspace_close::{
    WorkspaceCloseConfirmationView, WorkspaceCloseStatus, WorkspaceCloseStatusView,
    WorkspaceCloseToken,
};
use crate::{
    client_view::{ClientKey, SessionActionDisposition},
    terminal_session::{
        mailbox::{CommandMailbox, UiWaitKey},
        state::{
            CarrierFailure, NativeSessionState, OperationDeadlines,
            workspace_close::WorkspaceCloseEvidence,
        },
    },
};
use orchbee_workbench_protocol::{
    WorkbenchProjectId, WorkbenchSnapshotOutcome, WorkbenchWorkspaceId,
};
use std::sync::Arc;

impl<'a> NativeSessionView<'a> {
    pub(crate) fn workspace_close_confirmation(
        &self,
    ) -> Option<WorkspaceCloseConfirmationView<'a>> {
        self.state.as_ref()?.workspace_close_confirmation()
    }
    pub(crate) fn workspace_close_status(&self) -> Option<WorkspaceCloseStatusView<'a>> {
        self.state.as_ref()?.workspace_close_status()
    }
}
impl NativeUi {
    pub(super) fn request_sidebar_workspace_delete(
        &mut self,
        request: &crate::terminal_session::WorkspaceDeleteRequest,
    ) -> SessionActionDisposition {
        if self.modal_owns_input() {
            return SessionActionDisposition::KeepNavigator;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if mailbox.try_enqueue_sidebar_workspace_delete_for(state, request, deadlines, &self.clock)
        {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }
    /// Opens Close for the addressed Workspace without navigating away from the current terminal.
    pub(in crate::terminal_session) fn request_workspace_close_at(
        &mut self,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> SessionActionDisposition {
        if self.modal_owns_input() {
            return SessionActionDisposition::KeepNavigator;
        }
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, |state| {
                state.open_workspace_close_at(workspace_id).ok_or(())
            })
            .is_ok()
        {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }

    /// Opens Project close-all for the explicitly named menu target without changing focus.
    pub(in crate::terminal_session) fn request_project_close_all(
        &mut self,
        project_id: &WorkbenchProjectId,
    ) -> SessionActionDisposition {
        if self.modal_owns_input() {
            return SessionActionDisposition::KeepNavigator;
        }
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, |state| {
                state.open_project_close_all(project_id).ok_or(())
            })
            .is_ok()
        {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }

    pub(super) fn request_workspace_close(&mut self) -> SessionActionDisposition {
        if self.modal_owns_input() {
            return SessionActionDisposition::KeepNavigator;
        }
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, |state| state.open_workspace_close().ok_or(()))
            .is_ok()
        {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }
    pub(in crate::terminal_session) fn capture_workspace_close_key(
        &mut self,
        expected: WorkspaceCloseToken,
        key: ClientKey,
    ) -> bool {
        let Some((token, interactive)) = self
            .state
            .as_ref()
            .and_then(NativeSessionState::workspace_close_confirmation)
            .map(|prompt| (prompt.token(), prompt.interactive()))
        else {
            return false;
        };
        if token != expected {
            return false;
        }
        if self
            .state
            .as_mut()
            .is_some_and(NativeSessionState::settle_stale_workspace_close)
        {
            return true;
        }
        if !interactive {
            if key == ClientKey::Escape
                && let Some(state) = self.state.as_mut()
            {
                state.cancel_workspace_close();
            }
            return true;
        }
        match key {
            ClientKey::Enter => {
                if let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now())
                    && let (Some(mailbox), Some(state)) =
                        (self.mailbox.as_ref(), self.state.as_mut())
                {
                    mailbox.try_enqueue_workspace_close_for(state, token, deadlines, &self.clock);
                }
            }
            ClientKey::Escape => {
                if let Some(state) = self.state.as_mut() {
                    state.cancel_workspace_close();
                }
            }
            _ => {}
        }
        true
    }
    /// A pointer gesture must still name this exact confirmation at release time.
    pub(crate) fn click_workspace_close(&mut self, token: WorkspaceCloseToken, confirm: bool) {
        if self
            .state
            .as_ref()
            .and_then(NativeSessionState::workspace_close_confirmation)
            .is_some_and(|prompt| prompt.token() == token && prompt.interactive())
        {
            self.capture_workspace_close_key(
                token,
                if confirm {
                    ClientKey::Enter
                } else {
                    ClientKey::Escape
                },
            );
        }
    }
    pub(super) fn after_workspace_close_draw(
        &mut self,
        prompt: Option<ModalPresentation<WorkspaceCloseToken>>,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match prompt {
            Some(ModalPresentation::Presented(token)) => {
                state.acknowledge_workspace_close_draw(Some(token));
            }
            Some(ModalPresentation::NotPresented) => {
                state.acknowledge_workspace_close_draw(None);
                let _ = state.settle_workspace_close_not_presented();
            }
            None => state.acknowledge_workspace_close_draw(None),
        }
    }
    pub(in crate::terminal_session) fn workspace_close_worker_loss(
        &self,
    ) -> Option<WorkspaceCloseEvidence> {
        self.mailbox
            .as_ref()
            .and_then(|mailbox| mailbox.workspace_close_worker_loss())
    }
    pub(in crate::terminal_session) fn finalize_workspace_close_worker(
        &mut self,
        evidence: Option<WorkspaceCloseEvidence>,
    ) -> bool {
        evidence.is_none_or(|evidence| {
            self.state
                .as_mut()
                .is_some_and(|state| state.finish_workspace_close(&evidence))
        })
    }
    pub(super) fn apply_workspace_close_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: UiWaitKey,
        evidence: WorkspaceCloseEvidence,
        outcome: WorkbenchSnapshotOutcome,
    ) -> bool {
        if !self
            .state
            .as_mut()
            .is_some_and(|state| state.finish_workspace_close(&evidence))
        {
            self.retire(CarrierFailure::Protocol);
            return true;
        }
        let changed =
            self.apply_refresh_event(mailbox, wait, evidence.operation.generation, outcome);
        if let Some(state) = self.state.as_mut() {
            state.review_sidebar_workspace_delete(&evidence);
        }
        changed
    }
}
