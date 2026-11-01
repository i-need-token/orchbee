//! Adapts Workspace and Project-group close actions, token-bound confirmation, and correlated
//! retained outcomes to Client state. It does not perform physical closure or authorize an
//! unpresented confirmation.

use super::held_intent::HeldIntent;
use super::{ModalPresentation, WorkbenchClientUi, WorkbenchClientView};
use crate::client_view::{ClientActionDisposition, ClientKey};
use crate::workbench_client::mailbox::{CommandMailbox, UiWaitKey};
#[cfg(test)]
pub(crate) use crate::workbench_state::workspace_close::WorkspaceCloseStatus;
pub use crate::workbench_state::workspace_close::WorkspaceCloseToken;
use crate::workbench_state::workspace_close::{
    SidebarDeleteReview, StaleCloseReview, WorkspaceCloseEvidence, WorkspaceCloseOperation,
    WorkspaceDeleteRequest,
};
pub(crate) use crate::workbench_state::workspace_close::{
    WorkspaceCloseConfirmationView, WorkspaceCloseStatusView,
};
use crate::workbench_state::{
    CarrierFailure, ClientNotice, OperationDeadlines, WorkbenchClientState,
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
        request: &WorkspaceDeleteRequest,
    ) -> ClientActionDisposition {
        if self.modal_owns_input() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let Some(state) = self.state.as_mut() else {
            return ClientActionDisposition::KeepNavigator;
        };
        // A held or retried request meets the newer projection its wait installed.
        let Some(request) = request.rebased(state) else {
            state.set_notice(ClientNotice::WorkspaceChanged);
            return ClientActionDisposition::KeepNavigator;
        };
        if state.background_refresh_only() {
            self.hold_intent(HeldIntent::WorkspaceDelete(Box::new(request)));
            return ClientActionDisposition::DismissNavigator;
        }
        let Some(mailbox) = self.mailbox.as_ref() else {
            return ClientActionDisposition::KeepNavigator;
        };
        if mailbox.try_enqueue_sidebar_workspace_delete_for(state, &request, deadlines, &self.clock)
        {
            ClientActionDisposition::DismissNavigator
        } else {
            // A refused admission sends nothing; say so instead of dropping the request.
            state.set_notice(ClientNotice::LocalBusy);
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
            WorkspaceDeleteRequest::capture(state, workspace_id, deadlines.total())
        }) else {
            if self.switch_landing() {
                self.hold_intent(HeldIntent::WorkspaceCloseAt(workspace_id.clone()));
                return ClientActionDisposition::DismissNavigator;
            }
            if let Some(state) = self.state.as_mut() {
                let notice = if state.close_scope_available() {
                    ClientNotice::WorkspaceCloseUnavailable
                } else {
                    ClientNotice::LocalBusy
                };
                state.set_notice(notice);
            }
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
        if self.modal_owns_input() || self.state.is_none() {
            return ClientActionDisposition::KeepNavigator;
        }
        // The key names where the landing switch is headed, never the Workspace being left: it
        // closes that destination once the switch landed there. A destination that a click the
        // Client could not resolve made uncertain is refused instead, and once more after the
        // landing, since the user may have pressed the key before seeing what the switch shows.
        if self.switch_landing() {
            if let Some(destination) = self.landing_destination() {
                self.hold_intent(HeldIntent::CurrentWorkspaceClose(destination));
                return ClientActionDisposition::DismissNavigator;
            }
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::WorkspaceChanged);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        if self.take_destination_uncertainty() {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::WorkspaceChanged);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(state) = self.state.as_ref() else {
            return ClientActionDisposition::KeepNavigator;
        };
        let crate::workbench_projection::ClientContextLocation::Workspace { workspace_id, .. } =
            state.cache().location()
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
            ClientKey::Enter => self.submit_workspace_close(token),
            ClientKey::Escape => {
                self.release_held_workspace_close(token);
                if let Some(state) = self.state.as_mut() {
                    state.cancel_workspace_close();
                }
            }
            _ => {}
        }
        true
    }
    /// Confirms the drawn prompt. A background read that owns the refresh slot holds the
    /// confirmation until it settles; any other refusal says that nothing was sent.
    fn submit_workspace_close(&mut self, token: WorkspaceCloseToken) {
        if self
            .state
            .as_ref()
            .is_some_and(WorkbenchClientState::background_refresh_only)
        {
            self.hold_intent(HeldIntent::WorkspaceClose(token));
            return;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return;
        };
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return;
        };
        if !mailbox.try_enqueue_workspace_close_for(state, token, deadlines, &self.clock)
            && state
                .workspace_close_confirmation()
                .is_some_and(|prompt| prompt.token() == token)
        {
            state.set_notice(ClientNotice::LocalBusy);
        }
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
        let review = self
            .state
            .as_mut()
            .map(|state| state.review_sidebar_workspace_delete(&evidence));
        if let Some(SidebarDeleteReview::Recheck(workspace_id)) = review {
            self.recheck_sidebar_workspace_delete(&workspace_id, &evidence);
        }
        match self
            .state
            .as_mut()
            .map(|state| state.review_stale_workspace_close(&evidence))
        {
            Some(StaleCloseReview::Retry) => self.retry_workspace_close(&evidence.operation),
            Some(StaleCloseReview::Recheck(workspace_id)) => {
                self.recheck_sidebar_workspace_delete(&workspace_id, &evidence);
            }
            Some(StaleCloseReview::NotApplicable | StaleCloseReview::Changed) | None => {}
        }
        changed
    }

    /// Submits a confirmed close once more after a newer projection kept the first request from
    /// admission. The user confirmed this exact scope, and the state owner proved it unchanged.
    fn retry_workspace_close(&mut self, prior: &WorkspaceCloseOperation) {
        let admitted = OperationDeadlines::from_sample(self.clock.now()).is_some_and(|deadlines| {
            let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
                return false;
            };
            mailbox.try_enqueue_workspace_close_retry_for(state, prior, deadlines, &self.clock)
        });
        if !admitted && let Some(state) = self.state.as_mut() {
            state.settle_sidebar_workspace_delete_changed();
        }
    }

    /// Re-asks the Server after `prior`'s reply: once after the target changed under its
    /// confirmation reply, or within the stale retry bound after a newer projection kept it from
    /// admission. The conditional request has no effect before confirmation, so asking again is
    /// safe.
    fn recheck_sidebar_workspace_delete(
        &mut self,
        workspace_id: &WorkbenchWorkspaceId,
        prior: &WorkspaceCloseEvidence,
    ) {
        let admitted = OperationDeadlines::from_sample(self.clock.now()).is_some_and(|deadlines| {
            let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
                return false;
            };
            WorkspaceDeleteRequest::recheck(state, workspace_id, prior, deadlines.total())
                .is_some_and(|request| {
                    mailbox.try_enqueue_sidebar_workspace_delete_for(
                        state,
                        &request,
                        deadlines,
                        &self.clock,
                    )
                })
        });
        if !admitted && let Some(state) = self.state.as_mut() {
            state.settle_sidebar_workspace_delete_changed();
        }
    }
}
