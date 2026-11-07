//! Opens the focused Panel's scrollback editor from a local action through the same popup lane,
//! admission, and refusal reasons as a remote `editor-open` request; the Server remains the
//! capture and editor owner.
use super::{ClientActionDisposition, WorkbenchClientNotice, WorkbenchClientUi};
use crate::workbench_client::client_control::random_request_id;
use orchbee_workbench_protocol::{
    ClientViewControlReason as Reason, ClientViewPhase, ScrollbackEditorSpec, WorkbenchRequestId,
};
use std::time::Duration;

impl WorkbenchClientUi {
    pub(super) fn request_scrollback_editor(&mut self) -> ClientActionDisposition {
        match self.begin_scrollback_editor() {
            Ok(id) => self.scrollback_editor = Some(id),
            Err(reason) => self.note_scrollback_editor_refusal(reason),
        }
        ClientActionDisposition::DismissNavigator
    }

    fn begin_scrollback_editor(&mut self) -> Result<WorkbenchRequestId, Reason> {
        if self.retirement_failure.is_some() {
            return Err(Reason::ProjectionUnavailable);
        }
        if self.popup_modal_open() {
            return Err(Reason::ModalOpen);
        }
        let (_, view) = self
            .control_observation(false)
            .ok_or(Reason::ProjectionUnavailable)?;
        if self.navigation_pending() || view.phase != ClientViewPhase::Ready {
            return Err(Reason::Busy);
        }
        // Only a bound displayed member has scrollback, as the remote request requires.
        let source = self
            .configured_command_source()
            .and_then(|(source, _)| source)
            .ok_or(Reason::TargetUnavailable)?;
        let snapshot = self
            .state
            .as_ref()
            .map(|state| state.cache().canonical_snapshot().clone())
            .ok_or(Reason::ProjectionUnavailable)?;
        let session = self.admit_popup_source(&source, view.workbench_revision, Some(&snapshot))?;
        let id = random_request_id().ok_or(Reason::Busy)?;
        self.popup
            .begin_editor(session, id.clone(), ScrollbackEditorSpec { source })?;
        Ok(id)
    }

    /// A refusal the popup lane reports after admission, such as an unavailable editor, answers
    /// the action that opened it; a failure already shows the popup's own notice.
    pub(super) fn settle_scrollback_editor(&mut self) -> bool {
        let Some(id) = self.scrollback_editor.as_ref() else {
            return false;
        };
        match self.popup.settled(id) {
            None => false,
            Some(refusal) => {
                self.scrollback_editor = None;
                if let Some(reason) = refusal {
                    self.note_scrollback_editor_refusal(reason);
                }
                true
            }
        }
    }

    fn note_scrollback_editor_refusal(&mut self, reason: Reason) {
        let notice = match reason {
            Reason::ModalOpen => WorkbenchClientNotice::ModalOwnsInput,
            Reason::Busy => WorkbenchClientNotice::LocalBusy,
            Reason::InsufficientSpace => WorkbenchClientNotice::ScrollbackEditorTooSmall,
            _ => WorkbenchClientNotice::ScrollbackEditorUnavailable,
        };
        self.command_notice.set(
            self.clock
                .now()
                .checked_add(Duration::from_secs(5))
                .map(|until| (notice, until)),
        );
    }
}
