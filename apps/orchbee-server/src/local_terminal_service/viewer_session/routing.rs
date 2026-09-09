use super::{
    Action, LocalTerminalSession, Outcome, Rejected, TargetLifecycle, TerminalViewerRequest,
    Unavailable, ViewerRejection, WorkerAction, WorkerOperationRequest, WorkerOperationResult,
    WorkerOutcome, WorkerRequest, WorkerRequestError, rejected, snapshot_reason, unavailable,
};

impl LocalTerminalSession<'_> {
    pub(super) fn execute_viewer(&mut self, request: &TerminalViewerRequest) -> Outcome {
        match request.action() {
            Action::Attach {
                terminal_create_request_id,
            } => {
                if !self.admit_viewer_slot() {
                    return unavailable(Unavailable::ServerBusy);
                }
                let issued = self.service.issue_viewer(terminal_create_request_id);
                return self.attach(issued);
            }
            Action::AttachBinding {
                site_id,
                worker_id,
                terminal_id,
            } => {
                if !self.admit_viewer_slot() {
                    return unavailable(Unavailable::ServerBusy);
                }
                let issued = self
                    .service
                    .issue_binding_viewer(site_id, worker_id, terminal_id);
                return self.attach(issued);
            }
            Action::Poll { .. }
            | Action::FrameApplied { .. }
            | Action::NeedSnapshot { .. }
            | Action::Detach { .. } => {}
        }
        let index = request.action().attachment_id().and_then(|attachment_id| {
            self.viewers.iter().position(|entry| {
                entry
                    .as_ref()
                    .is_some_and(|viewer| &viewer.attachment_id == attachment_id)
            })
        });
        let Some(index) = index else {
            return if matches!(request.action(), Action::Detach { .. }) {
                Outcome::Detached {}
            } else {
                rejected(Rejected::TerminalUnavailable)
            };
        };
        let Some(viewer) = self.viewers[index].as_mut() else {
            unreachable!("selected occupied viewer slot")
        };
        if self.service.target_lifecycle(viewer.target) == Some(TargetLifecycle::Disposed) {
            self.viewers[index].take();
            return if matches!(request.action(), Action::Detach { .. }) {
                Outcome::Detached {}
            } else {
                unavailable(Unavailable::TerminalRuntimeUnavailable)
            };
        }
        if !self.service.viewer_worker_is_current(&viewer.handle) {
            self.viewers[index].take();
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        if matches!(request.action(), Action::Poll { .. }) && viewer.projection.awaiting_feedback()
        {
            return Outcome::Empty {};
        }
        let feedback = matches!(
            request.action(),
            Action::FrameApplied { .. } | Action::NeedSnapshot { .. }
        );
        if feedback && !viewer.projection.accepts_feedback(request.action()) {
            return rejected(Rejected::FeedbackMismatch);
        }
        let Some(request_id) = self.service.next_viewer_operation() else {
            return unavailable(Unavailable::ServerBusy);
        };
        let Some(template) = viewer.cleanup else {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        };
        let action = match request.action() {
            Action::Poll { .. } => WorkerAction::Poll,
            Action::Detach { .. } => WorkerAction::Detach,
            Action::FrameApplied {
                sequence,
                generation,
                revision,
                ..
            } => WorkerAction::FrameApplied {
                sequence: *sequence,
                generation: *generation,
                revision: *revision,
            },
            Action::NeedSnapshot {
                sequence, reason, ..
            } => WorkerAction::NeedSnapshot {
                sequence: *sequence,
                reason: snapshot_reason(*reason),
            },
            Action::Attach { .. } | Action::AttachBinding { .. } => {
                unreachable!("attach is handled before existing-viewer operations")
            }
        };
        let completion = viewer
            .handle
            .request(WorkerOperationRequest::Viewer(WorkerRequest {
                request_id,
                action,
                ..template
            }));
        if !self.service.viewer_worker_is_current(&viewer.handle) {
            self.viewers[index].take();
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        match completion {
            Ok(WorkerOperationResult::Viewer(result)) => match result.outcome {
                WorkerOutcome::Empty => Outcome::Empty {},
                WorkerOutcome::Record(record) => {
                    if viewer.projection.receive(&record).is_ok() {
                        Outcome::Record(record)
                    } else {
                        self.viewers[index].take();
                        unavailable(Unavailable::TerminalRuntimeUnavailable)
                    }
                }
                WorkerOutcome::FeedbackAccepted => {
                    viewer.projection.complete_feedback(request.action());
                    Outcome::FeedbackAccepted {}
                }
                WorkerOutcome::Detached => {
                    viewer.cleanup = None;
                    self.viewers[index].take();
                    Outcome::Detached {}
                }
                WorkerOutcome::Rejected(ViewerRejection::FeedbackMismatch) => {
                    rejected(Rejected::FeedbackMismatch)
                }
                WorkerOutcome::Rejected(_) | WorkerOutcome::Attached => {
                    self.viewers[index].take();
                    unavailable(Unavailable::TerminalRuntimeUnavailable)
                }
            },
            Err(WorkerRequestError::Backpressure) => unavailable(Unavailable::ServerBusy),
            _ => {
                self.viewers[index].take();
                unavailable(Unavailable::TerminalRuntimeUnavailable)
            }
        }
    }
}
