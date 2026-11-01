//! Capture uses a read-only viewer on the same connection as the one editor Open.
use super::{
    AttachmentId, Duration, HostEpoch, LocalClient, Ordering, PopupOutcome, PopupRunner, Reason,
    ScrollbackEditorSpec, TerminalProjection, TerminalViewerAction, TerminalViewerOutcome,
    WorkbenchRequestId, WorkbenchTerminalGeometry, thread,
};
use orchbee_client_server_protocol::{SessionId as ObcsSessionId, TerminalId, WorkerId};
use orchbee_workbench_protocol::{EditorPopupCapture, TerminalEditorAction, WorkbenchRevision};

impl PopupRunner {
    pub(super) fn open_editor(
        &self,
        client: &mut LocalClient,
        spec: Box<ScrollbackEditorSpec>,
        geometry: WorkbenchTerminalGeometry,
    ) -> Result<PopupOutcome, Reason> {
        let binding = spec.source.member.binding().ok_or(Reason::StaleView)?;
        let TerminalViewerOutcome::Attached {
            attachment_id,
            host_epoch,
            host_terminal_id,
            ..
        } = self.viewer(
            client,
            TerminalViewerAction::AttachBinding {
                session_id: ObcsSessionId::parse(self.session.as_str())
                    .map_err(|_| Reason::StaleView)?,
                worker_id: WorkerId::parse(binding.worker_id.as_str())
                    .map_err(|_| Reason::StaleView)?,
                terminal_id: TerminalId::parse(binding.terminal_id.as_str())
                    .map_err(|_| Reason::StaleView)?,
            },
        )?
        else {
            return Ok(PopupOutcome::Refused(Reason::TargetUnavailable));
        };
        let mut projection = TerminalProjection::new(
            AttachmentId::parse(attachment_id.as_str())
                .map_err(|_| Reason::ProjectionUnavailable)?,
            HostEpoch::parse(host_epoch.as_str()).map_err(|_| Reason::ProjectionUnavailable)?,
        )
        .with_host_terminal(host_terminal_id);
        let deadline = self.clock.now() + Duration::from_secs(20);
        let capture = loop {
            if self.stop.load(Ordering::Acquire) {
                return Err(Reason::Interrupted);
            }
            if self.close.load(Ordering::Acquire) {
                break None;
            }
            if self.clock.now() >= deadline {
                return Err(Reason::Expired);
            }
            match self.viewer(
                client,
                TerminalViewerAction::Poll {
                    attachment_id: attachment_id.clone(),
                },
            )? {
                TerminalViewerOutcome::Record(record) => {
                    if let Some(feedback) = projection
                        .receive_for_geometry(&record, None)
                        .map_err(|_| Reason::ProjectionUnavailable)?
                        && !matches!(
                            self.viewer(client, feedback)?,
                            TerminalViewerOutcome::FeedbackAccepted {}
                        )
                    {
                        return Err(Reason::ProjectionUnavailable);
                    }
                    if let Some(frame) = projection.frame() {
                        break Some(EditorPopupCapture {
                            attachment_id: WorkbenchRequestId::parse(attachment_id.as_str())
                                .map_err(|_| Reason::ProjectionUnavailable)?,
                            generation: WorkbenchRevision::new(frame.route.generation.get()),
                            revision: WorkbenchRevision::new(frame.route.revision),
                        });
                    }
                }
                // The Host viewer has no output-ready event; this bounded capture samples frames.
                TerminalViewerOutcome::Empty {} => thread::sleep(Duration::from_millis(20)),
                TerminalViewerOutcome::Attached { .. }
                | TerminalViewerOutcome::FeedbackAccepted { .. }
                | TerminalViewerOutcome::Detached { .. }
                | TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::RowsRejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => {
                    return Err(Reason::ProjectionUnavailable);
                }
            }
        };
        let opened = if let Some(capture) = capture
            && !self.close.load(Ordering::Acquire)
        {
            self.editor(
                client,
                TerminalEditorAction::Open {
                    popup_id: self.id.clone(),
                    spec,
                    capture,
                    geometry,
                },
            )?
        } else {
            PopupOutcome::Closed
        };
        if !matches!(
            self.viewer(client, TerminalViewerAction::Detach { attachment_id })?,
            TerminalViewerOutcome::Detached {}
        ) {
            return Err(Reason::Interrupted);
        }
        Ok(opened)
    }
}
