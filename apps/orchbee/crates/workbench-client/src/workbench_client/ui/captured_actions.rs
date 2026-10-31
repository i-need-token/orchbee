//! Executes the original immutable menu capture after the common Client action admission and
//! cleanup sequence. The view reducer and terminal lease have already admitted its plain intent.
use super::WorkbenchClientUi;
use crate::client_view::ClientActionDisposition;
use crate::workbench_client::input::{CapturedClientAction, CapturedRequest};

impl WorkbenchClientUi {
    pub(super) fn apply_captured_action(
        &mut self,
        captured: CapturedClientAction,
    ) -> ClientActionDisposition {
        match captured.into_request() {
            CapturedRequest::WorkspaceAtDirectory(request) => {
                if self.request_quick_workspace_with_review(Some(*request), true) {
                    ClientActionDisposition::LayoutChanged
                } else {
                    ClientActionDisposition::KeepNavigator
                }
            }
            CapturedRequest::ProjectQuickWorkspace(request) => {
                if self.request_quick_workspace_for(Some(*request)) {
                    return ClientActionDisposition::LayoutChanged;
                } else if let Some(state) = self.state.as_mut() {
                    state.set_notice(crate::workbench_state::ClientNotice::DraftCancelled);
                }
                ClientActionDisposition::KeepNavigator
            }
            CapturedRequest::TargetedProjectRename(request) => {
                if self.request_project_rename(request).is_none()
                    && let Some(state) = self.state.as_mut()
                {
                    state.set_notice(crate::workbench_state::ClientNotice::DraftCancelled);
                }
                ClientActionDisposition::KeepNavigator
            }
            CapturedRequest::WorkspaceOrganization(request) => {
                if !self.request_workspace_organization(request)
                    && let Some(state) = self.state.as_mut()
                {
                    state.set_notice(crate::workbench_state::ClientNotice::DraftCancelled);
                }
                ClientActionDisposition::KeepNavigator
            }
            CapturedRequest::SidebarWorkspaceDelete(request) => {
                self.request_sidebar_workspace_delete(&request)
            }
        }
    }
}
