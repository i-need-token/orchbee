//! Source capture derives physical identity only from this connection's retained viewer.
use super::{LocalTerminalConnection, OperationId, TargetLifecycle};
use orchbee_workbench_protocol::{
    TerminalEditorAction, TerminalEditorRequest, TerminalEditorResult,
};
use std::num::NonZeroU64;

impl LocalTerminalConnection<'_> {
    pub(crate) fn execute_editor(
        &mut self,
        owner: impl Into<crate::local_terminal_service::PopupOwner>,
        request: TerminalEditorRequest,
    ) -> TerminalEditorResult {
        let source = if let TerminalEditorAction::Open { capture, .. } = &request.action {
            self.viewers
                .iter()
                .flatten()
                .find(|viewer| viewer.attachment_id.as_str() == capture.attachment_id.as_str())
                .filter(|viewer| {
                    self.service.viewer_worker_is_current(&viewer.handle)
                        && self.service.target_lifecycle(viewer.target)
                            == Some(TargetLifecycle::Unfenced)
                        && viewer.cleanup.is_some()
                        && viewer.projection.capturable_version(
                            capture.generation.value(),
                            capture.revision.value(),
                        )
                })
                .and_then(|viewer| {
                    Some((
                        orchbee_capability_node_protocol::TerminalEditorSource {
                            attachment_id: OperationId::from_bytes(
                                *viewer.projection.attachment_id.as_bytes(),
                            )
                            .ok()?,
                            host_epoch: viewer.target.identity.host_epoch(),
                            terminal_id: viewer.target.identity.terminal_id(),
                            generation: NonZeroU64::new(capture.generation.value())?,
                            revision: capture.revision.value(),
                        },
                        viewer.target,
                    ))
                })
        } else {
            None
        };
        self.service
            .execute_editor_popup(owner.into(), request, source)
    }
}
