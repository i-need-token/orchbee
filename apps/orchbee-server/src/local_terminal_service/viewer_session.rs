use super::{LocalTerminalService, placement, projection::ViewerProjection};
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
};
use orchbee_capability_node_protocol::{
    OperationId, TerminalViewerAction as WorkerAction, TerminalViewerOutcome as WorkerOutcome,
    TerminalViewerRequest as WorkerRequest, ViewerRejection,
};
use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, RequestId, ServerMessage, TerminalViewerAction as Action,
    TerminalViewerOutcome as Outcome, TerminalViewerRequest, TerminalViewerResult, ViewerId,
    ViewerRejectedReason as Rejected, ViewerUnavailableReason as Unavailable,
};

pub(crate) struct LocalTerminalSession<'service> {
    service: &'service LocalTerminalService,
    viewer: Option<ViewerAttachment<'service>>,
}

pub(super) struct ViewerAttachment<'service> {
    pub(super) handle: WorkerRequestHandle,
    pub(super) target: super::controller_session::ControlTarget,
    pub(super) controller: Option<super::controller_session::ControllerLease<'service>>,
    pub(super) cleanup: Option<WorkerRequest>,
    projection: ViewerProjection,
}
impl Drop for ViewerAttachment<'_> {
    fn drop(&mut self) {
        self.controller.take();
        if let Some(cleanup) = self.cleanup.take() {
            self.handle.enqueue_detach(cleanup);
        }
    }
}

impl<'service> LocalTerminalSession<'service> {
    pub(super) const fn new(service: &'service LocalTerminalService) -> Self {
        Self {
            service,
            viewer: None,
        }
    }

    pub(crate) fn execute(&mut self, request: ClientMessage) -> ServerMessage {
        let request = match request {
            ClientMessage::TerminalControl(request) => {
                let outcome = super::controller_session::execute(
                    self.service,
                    self.viewer.as_mut(),
                    &request,
                );
                return ServerMessage::TerminalControlResult(
                    orchbee_client_server_protocol::TerminalControlResult::new(
                        request.request_id().clone(),
                        outcome,
                    ),
                );
            }
            request => request,
        };
        let ClientMessage::TerminalViewer(request) = request else {
            return self.service.execute(request);
        };
        let outcome = self.execute_viewer(&request);
        ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        ))
    }

    fn execute_viewer(&mut self, request: &TerminalViewerRequest) -> Outcome {
        if let Action::Attach {
            terminal_create_request_id,
        } = request.action()
        {
            if self.viewer.is_some() {
                return rejected(Rejected::AlreadyAttached);
            }
            return self.attach(terminal_create_request_id);
        }
        let Some(viewer) = self.viewer.as_mut() else {
            return if matches!(request.action(), Action::Detach {}) {
                Outcome::Detached {}
            } else {
                rejected(Rejected::TerminalUnavailable)
            };
        };
        if !self.service.viewer_worker_is_current(&viewer.handle) {
            self.viewer.take();
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        if matches!(request.action(), Action::Poll {}) && viewer.projection.awaiting_feedback() {
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
            Action::Poll {} => WorkerAction::Poll,
            Action::Detach {} => WorkerAction::Detach,
            Action::FrameApplied {
                sequence,
                generation,
                revision,
            } => WorkerAction::FrameApplied {
                sequence: *sequence,
                generation: *generation,
                revision: *revision,
            },
            Action::NeedSnapshot { sequence, reason } => WorkerAction::NeedSnapshot {
                sequence: *sequence,
                reason: snapshot_reason(*reason),
            },
            Action::Attach { .. } => {
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
            self.viewer.take();
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        match completion {
            Ok(WorkerOperationResult::Viewer(result)) => match result.outcome {
                WorkerOutcome::Empty => Outcome::Empty {},
                WorkerOutcome::Record(record) => {
                    if viewer.projection.receive(&record).is_ok() {
                        Outcome::Record(record)
                    } else {
                        self.viewer.take();
                        unavailable(Unavailable::TerminalRuntimeUnavailable)
                    }
                }
                WorkerOutcome::FeedbackAccepted => {
                    viewer.projection.complete_feedback(request.action());
                    Outcome::FeedbackAccepted {}
                }
                WorkerOutcome::Detached => {
                    viewer.cleanup = None;
                    self.viewer.take();
                    Outcome::Detached {}
                }
                WorkerOutcome::Rejected(ViewerRejection::FeedbackMismatch) => {
                    rejected(Rejected::FeedbackMismatch)
                }
                WorkerOutcome::Rejected(_) | WorkerOutcome::Attached => {
                    self.viewer.take();
                    unavailable(Unavailable::TerminalRuntimeUnavailable)
                }
            },
            Err(WorkerRequestError::Backpressure) => unavailable(Unavailable::ServerBusy),
            _ => {
                self.viewer.take();
                unavailable(Unavailable::TerminalRuntimeUnavailable)
            }
        }
    }

    fn attach(&mut self, reference: &RequestId) -> Outcome {
        let issued = self.service.issue_viewer(reference);
        let (mut viewer, request_id, action) = match issued {
            Ok(issued) => issued,
            Err(outcome) => return outcome,
        };
        let Some(cleanup) = viewer.cleanup else {
            return unavailable(Unavailable::ServerBusy);
        };
        let completion = viewer
            .handle
            .request(WorkerOperationRequest::Viewer(WorkerRequest {
                request_id,
                action,
                ..cleanup
            }));
        if !self.service.viewer_worker_is_current(&viewer.handle) {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        match completion {
            Ok(WorkerOperationResult::Viewer(result)) => match result.outcome {
                WorkerOutcome::Attached => {
                    let attachment_id = ViewerId::parse(&viewer.projection.attachment_id.to_text());
                    let host_epoch = HostEpoch::parse(&viewer.projection.host_epoch.to_text());
                    let (Ok(attachment_id), Ok(host_epoch)) = (attachment_id, host_epoch) else {
                        return unavailable(Unavailable::TerminalRuntimeUnavailable);
                    };
                    self.viewer = Some(viewer);
                    Outcome::Attached {
                        attachment_id,
                        host_epoch,
                    }
                }
                WorkerOutcome::Rejected(reason) => {
                    // A rejected Attach did not grant ownership, even on an identity collision.
                    viewer.cleanup = None;
                    if reason == ViewerRejection::CapacityExceeded {
                        unavailable(Unavailable::ServerBusy)
                    } else {
                        rejected(Rejected::TerminalUnavailable)
                    }
                }
                _ => unavailable(Unavailable::TerminalRuntimeUnavailable),
            },
            Err(WorkerRequestError::Backpressure) => {
                viewer.cleanup = None;
                unavailable(Unavailable::ServerBusy)
            }
            Err(
                WorkerRequestError::NotDelivered(_)
                | WorkerRequestError::Unavailable
                | WorkerRequestError::InvalidRequest(_)
                | WorkerRequestError::WrongWorker
                | WorkerRequestError::WrongGeneration,
            ) => {
                viewer.cleanup = None;
                unavailable(Unavailable::TerminalRuntimeUnavailable)
            }
            _ => unavailable(Unavailable::TerminalRuntimeUnavailable),
        }
    }
}

impl LocalTerminalService {
    pub(crate) const fn session(&self) -> LocalTerminalSession<'_> {
        LocalTerminalSession::new(self)
    }

    pub(super) fn next_viewer_operation(&self) -> Option<OperationId> {
        OperationId::from_bytes(self.coordinator.lock().ok()?.allocate_identity()?).ok()
    }

    pub(super) fn viewer_worker_is_current(&self, handle: &WorkerRequestHandle) -> bool {
        let Ok(workers) = self.workers.lock() else {
            return false;
        };
        handle.is_active()
            && placement(workers.request_handle().as_ref()) == placement(Some(handle))
    }

    fn issue_viewer(
        &self,
        reference: &RequestId,
    ) -> Result<(ViewerAttachment<'_>, OperationId, WorkerAction), Outcome> {
        let workers = self
            .workers
            .lock()
            .map_err(|_| unavailable(Unavailable::ServerBusy))?;
        let handle = workers
            .request_handle()
            .ok_or_else(|| unavailable(Unavailable::TerminalRuntimeUnavailable))?;
        let worker = placement(Some(&handle))
            .ok_or_else(|| unavailable(Unavailable::TerminalRuntimeUnavailable))?;
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_| unavailable(Unavailable::ServerBusy))?;
        let target = coordinator
            .viewer_target(reference, worker)
            .ok_or_else(|| rejected(Rejected::TerminalUnavailable))?;
        let mut next = || {
            coordinator
                .allocate_identity()
                .and_then(|bytes| OperationId::from_bytes(bytes).ok())
                .ok_or_else(|| unavailable(Unavailable::ServerBusy))
        };
        let attachment_id = next()?;
        let request_id = next()?;
        let cleanup_id = next()?;
        let projection = ViewerProjection::new(
            orchbee_terminal_protocol::AttachmentId::from_bytes(attachment_id.as_bytes())
                .map_err(|_| unavailable(Unavailable::ServerBusy))?,
            orchbee_terminal_protocol::HostEpoch::from_bytes(target.host_epoch().as_bytes())
                .map_err(|_| unavailable(Unavailable::ServerBusy))?,
        );
        Ok((
            ViewerAttachment {
                target: super::controller_session::ControlTarget {
                    worker: worker.worker_id.as_bytes(),
                    generation: worker.generation.get(),
                    host_epoch: target.host_epoch().as_bytes(),
                    terminal: target.terminal_id().as_bytes(),
                },
                controller: None,
                handle,
                projection,
                cleanup: Some(WorkerRequest {
                    request_id: cleanup_id,
                    worker_id: worker.worker_id,
                    worker_generation: worker.generation,
                    attachment_id,
                    action: WorkerAction::Detach,
                }),
            },
            request_id,
            WorkerAction::Attach {
                host_epoch: target.host_epoch(),
                terminal_id: target.terminal_id(),
            },
        ))
    }
}

fn rejected(reason: Rejected) -> Outcome {
    Outcome::Rejected { reason }
}
fn unavailable(reason: Unavailable) -> Outcome {
    Outcome::Unavailable { reason }
}
fn snapshot_reason(
    reason: orchbee_client_server_protocol::ViewerSnapshotReason,
) -> orchbee_capability_node_protocol::ViewerSnapshotReason {
    use orchbee_capability_node_protocol::ViewerSnapshotReason as Worker;
    use orchbee_client_server_protocol::ViewerSnapshotReason as Client;
    match reason {
        Client::ActivityStale => Worker::ActivityStale,
        Client::BaseMismatch => Worker::BaseMismatch,
        Client::DecoderRejected => Worker::DecoderRejected,
        Client::DimensionsMismatch => Worker::DimensionsMismatch,
        Client::GenerationMismatch => Worker::GenerationMismatch,
        Client::IdentityMismatch => Worker::IdentityMismatch,
        Client::SequenceGap => Worker::SequenceGap,
        Client::StoreReset => Worker::StoreReset,
    }
}
