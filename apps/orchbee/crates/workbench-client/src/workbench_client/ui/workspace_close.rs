//! Adapts Workspace and Project-group close actions, token-bound confirmation, and correlated
//! retained outcomes to Client state. It does not perform physical closure or authorize an
//! unpresented confirmation.

use super::{ModalPresentation, WorkbenchClientUi, WorkbenchClientView};
#[cfg(test)]
pub(crate) use crate::workbench_client::state::workspace_close::WorkspaceCloseStatus;
pub use crate::workbench_client::state::workspace_close::WorkspaceCloseToken;
pub(crate) use crate::workbench_client::state::workspace_close::{
    WorkspaceCloseConfirmationView, WorkspaceCloseStatusView,
};
use crate::{
    client_view::{ClientActionDisposition, ClientKey},
    workbench_client::{
        mailbox::{CommandMailbox, UiWaitKey},
        state::{
            CarrierFailure, OperationDeadlines, WorkbenchClientState,
            workspace_close::WorkspaceCloseEvidence,
        },
    },
};
use orchbee_workbench_protocol::{
    WorkbenchProjectId, WorkbenchSnapshotOutcome, WorkbenchWorkspaceId,
};
use std::sync::Arc;

impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn workspace_close_confirmation(
        &self,
    ) -> Option<WorkspaceCloseConfirmationView<'a>> {
        self.state.as_ref()?.workspace_close_confirmation()
    }
    pub(crate) fn workspace_close_status(&self) -> Option<WorkspaceCloseStatusView<'a>> {
        self.state.as_ref()?.workspace_close_status()
    }
}
impl WorkbenchClientUi {
    pub(super) fn request_sidebar_workspace_delete(
        &mut self,
        request: &crate::workbench_client::WorkspaceDeleteRequest,
    ) -> ClientActionDisposition {
        if self.modal_owns_input() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if mailbox.try_enqueue_sidebar_workspace_delete_for(state, request, deadlines, &self.clock)
        {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }
    /// Checks the addressed Workspace through the same conditional Server owner as sidebar Delete.
    pub(in crate::workbench_client) fn request_workspace_close_at(
        &mut self,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> ClientActionDisposition {
        if self.modal_owns_input() {
            return ClientActionDisposition::KeepNavigator;
        }
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.can_confirm_guarded_workspace_close(workspace_id))
        {
            let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
                return ClientActionDisposition::KeepNavigator;
            };
            return if mailbox
                .apply_local_for(state, |state| {
                    state.open_workspace_close_at(workspace_id).ok_or(())
                })
                .is_ok()
            {
                ClientActionDisposition::DismissNavigator
            } else {
                ClientActionDisposition::KeepNavigator
            };
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let Some(request) = self.state.as_ref().and_then(|state| {
            crate::workbench_client::WorkspaceDeleteRequest::capture(
                state,
                workspace_id,
                deadlines.total(),
            )
        }) else {
            return ClientActionDisposition::KeepNavigator;
        };
        self.request_sidebar_workspace_delete(&request)
    }

    /// Opens Project close-all for the explicitly named menu target without changing focus.
    pub(in crate::workbench_client) fn request_project_close_all(
        &mut self,
        project_id: &WorkbenchProjectId,
    ) -> ClientActionDisposition {
        if self.modal_owns_input() {
            return ClientActionDisposition::KeepNavigator;
        }
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if mailbox
            .apply_local_for(state, |state| {
                state.open_project_close_all(project_id).ok_or(())
            })
            .is_ok()
        {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    pub(super) fn request_workspace_close(&mut self) -> ClientActionDisposition {
        if self.modal_owns_input() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(state) = self.state.as_ref() else {
            return ClientActionDisposition::KeepNavigator;
        };
        let crate::workbench_client::workbench_projection::ClientContextLocation::Workspace {
            workspace_id,
            ..
        } = state.cache().location()
        else {
            if let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) {
                let _ =
                    mailbox.apply_local_for(state, |state| state.open_workspace_close().ok_or(()));
            }
            return ClientActionDisposition::KeepNavigator;
        };
        self.request_workspace_close_at(&workspace_id.clone())
    }
    pub(in crate::workbench_client) fn capture_workspace_close_key(
        &mut self,
        expected: WorkspaceCloseToken,
        key: ClientKey,
    ) -> bool {
        let Some((token, interactive)) = self
            .state
            .as_ref()
            .and_then(WorkbenchClientState::workspace_close_confirmation)
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
            .is_some_and(WorkbenchClientState::settle_stale_workspace_close)
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
            .and_then(WorkbenchClientState::workspace_close_confirmation)
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
    pub(in crate::workbench_client) fn workspace_close_worker_loss(
        &self,
    ) -> Option<WorkspaceCloseEvidence> {
        self.mailbox
            .as_ref()
            .and_then(|mailbox| mailbox.workspace_close_worker_loss())
    }
    pub(in crate::workbench_client) fn finalize_workspace_close_worker(
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
