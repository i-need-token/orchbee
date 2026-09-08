use super::{LocalTerminalService, placement, projection::ViewerProjection};
use crate::site_coordinator::{
    BindingViewerPreparationError, PreparedBindingViewer, TargetLifecycle,
};
use crate::terminal_coordinator::{BindingViewerTarget, ViewerOperationIdentities};
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
};
use orchbee_capability_node_protocol::{
    OperationId, TerminalViewerAction as WorkerAction, TerminalViewerOutcome as WorkerOutcome,
    TerminalViewerRequest as WorkerRequest, ViewerRejection,
};
use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, RequestId, ServerMessage, SiteId as ClientSiteId,
    TerminalId as ClientTerminalId, TerminalViewerAction as Action,
    TerminalViewerOutcome as Outcome, TerminalViewerRequest, TerminalViewerResult, ViewerId,
    ViewerRejectedReason as Rejected, ViewerUnavailableReason as Unavailable,
    WorkerId as ClientWorkerId,
};

pub(crate) struct LocalTerminalSession<'service> {
    service: &'service LocalTerminalService,
    viewer: Option<ViewerAttachment<'service>>,
}

pub(super) struct ViewerAttachment<'service> {
    service: &'service LocalTerminalService,
    pub(super) handle: WorkerRequestHandle,
    pub(super) target: BindingViewerTarget,
    pub(super) controller: Option<super::controller_session::ControllerLease<'service>>,
    pub(super) cleanup: Option<WorkerRequest>,
    projection: ViewerProjection,
}

struct IssuedViewer<'service> {
    action: WorkerAction,
    request_id: OperationId,
    viewer: ViewerAttachment<'service>,
}

impl Drop for ViewerAttachment<'_> {
    fn drop(&mut self) {
        self.controller.take();
        if self.cleanup.is_some()
            && self.service.target_lifecycle(self.target) == Some(TargetLifecycle::Disposed)
        {
            self.cleanup = None;
        }
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
        match request.action() {
            Action::Attach {
                terminal_create_request_id,
            } => {
                if self.viewer_prevents_fresh_attach() {
                    return rejected(Rejected::AlreadyAttached);
                }
                let issued = self.service.issue_viewer(terminal_create_request_id);
                return self.attach(issued);
            }
            Action::AttachBinding {
                site_id,
                worker_id,
                terminal_id,
            } => {
                if self.viewer_prevents_fresh_attach() {
                    return rejected(Rejected::AlreadyAttached);
                }
                let issued = self
                    .service
                    .issue_binding_viewer(site_id, worker_id, terminal_id);
                return self.attach(issued);
            }
            Action::Poll {}
            | Action::FrameApplied { .. }
            | Action::NeedSnapshot { .. }
            | Action::Detach {} => {}
        }
        let Some(viewer) = self.viewer.as_mut() else {
            return if matches!(request.action(), Action::Detach {}) {
                Outcome::Detached {}
            } else {
                rejected(Rejected::TerminalUnavailable)
            };
        };
        if self.service.target_lifecycle(viewer.target) == Some(TargetLifecycle::Disposed) {
            self.viewer.take();
            return if matches!(request.action(), Action::Detach {}) {
                Outcome::Detached {}
            } else {
                unavailable(Unavailable::TerminalRuntimeUnavailable)
            };
        }
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

    fn viewer_prevents_fresh_attach(&self) -> bool {
        self.viewer.as_ref().is_some_and(|viewer| {
            self.service.target_lifecycle(viewer.target) != Some(TargetLifecycle::Disposed)
        })
    }

    fn attach(&mut self, issued: Result<IssuedViewer<'service>, Outcome>) -> Outcome {
        let IssuedViewer {
            action,
            request_id,
            viewer,
        } = match issued {
            Ok(issued) => issued,
            Err(outcome) => return outcome,
        };
        let Some(cleanup) = viewer.cleanup else {
            return unavailable(Unavailable::ServerBusy);
        };
        // Fresh preparation succeeded. The old exact disposal proof is monotonic, and no
        // owner gate survives issue_viewer/issue_binding_viewer into this owned settlement.
        if let Some(mut disposed) = self.viewer.take() {
            disposed.cleanup = None;
            if let Some(controller) = disposed.controller.as_mut() {
                controller.disarm_known_disposed();
            }
            drop(disposed);
        }
        let completion = viewer
            .handle
            .request(WorkerOperationRequest::Viewer(WorkerRequest {
                request_id,
                action,
                ..cleanup
            }));
        self.publish_attach(viewer, completion)
    }

    fn publish_attach(
        &mut self,
        mut viewer: ViewerAttachment<'service>,
        completion: Result<WorkerOperationResult, WorkerRequestError>,
    ) -> Outcome {
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
                    let mut candidate = Some(viewer);
                    self.install_viewer(&mut candidate, attachment_id, host_epoch)
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

    fn install_viewer(
        &mut self,
        candidate: &mut Option<ViewerAttachment<'service>>,
        attachment_id: ViewerId,
        host_epoch: HostEpoch,
    ) -> Outcome {
        let service = self.service;
        let Ok(workers) = service.workers.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        let Some(viewer) = candidate.as_ref() else {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        };
        if !viewer.handle.is_active()
            || placement(workers.request_handle().as_ref()) != placement(Some(&viewer.handle))
        {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        let Ok(coordinator) = service.coordinator.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        if coordinator.target_lifecycle(viewer.target) != TargetLifecycle::Unfenced {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        self.viewer = candidate.take();
        Outcome::Attached {
            attachment_id,
            host_epoch,
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

    fn issue_viewer(&self, reference: &RequestId) -> Result<IssuedViewer<'_>, Outcome> {
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
        drop(coordinator);
        drop(workers);
        self.build_issued_viewer(
            handle,
            target,
            ViewerOperationIdentities {
                attachment: attachment_id,
                request: request_id,
                cleanup: cleanup_id,
            },
        )
    }

    fn issue_binding_viewer(
        &self,
        site_id: &ClientSiteId,
        worker_id: &ClientWorkerId,
        terminal_id: &ClientTerminalId,
    ) -> Result<IssuedViewer<'_>, Outcome> {
        let workers = self
            .workers
            .lock()
            .map_err(|_| unavailable(Unavailable::ServerBusy))?;
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_| unavailable(Unavailable::ServerBusy))?;
        // Resolve samples the carrier only after both admission locks are held.
        let handle = workers.request_handle();
        let prepared = coordinator
            .prepare_binding_viewer(site_id, worker_id, terminal_id, placement(handle.as_ref()))
            .map_err(binding_preparation_outcome)?;
        let handle = handle.ok_or_else(|| unavailable(Unavailable::TerminalRuntimeUnavailable))?;
        drop(coordinator);
        drop(workers);
        let PreparedBindingViewer { target, identities } = prepared;
        self.build_issued_viewer(handle, target, identities)
    }

    fn build_issued_viewer(
        &self,
        handle: WorkerRequestHandle,
        target: BindingViewerTarget,
        identities: ViewerOperationIdentities,
    ) -> Result<IssuedViewer<'_>, Outcome> {
        let projection = ViewerProjection::new(
            orchbee_terminal_protocol::AttachmentId::from_bytes(identities.attachment.as_bytes())
                .map_err(|_| unavailable(Unavailable::ServerBusy))?,
            orchbee_terminal_protocol::HostEpoch::from_bytes(
                target.identity.host_epoch().as_bytes(),
            )
            .map_err(|_| unavailable(Unavailable::ServerBusy))?,
        );
        Ok(IssuedViewer {
            viewer: ViewerAttachment {
                service: self,
                target,
                controller: None,
                handle,
                projection,
                cleanup: Some(WorkerRequest {
                    request_id: identities.cleanup,
                    worker_id: target.worker.worker_id,
                    worker_generation: target.worker.generation,
                    attachment_id: identities.attachment,
                    action: WorkerAction::Detach,
                }),
            },
            request_id: identities.request,
            action: WorkerAction::Attach {
                host_epoch: target.identity.host_epoch(),
                terminal_id: target.identity.terminal_id(),
            },
        })
    }
}

fn binding_preparation_outcome(error: BindingViewerPreparationError) -> Outcome {
    match error {
        BindingViewerPreparationError::ServerBusy => unavailable(Unavailable::ServerBusy),
        BindingViewerPreparationError::TerminalRuntimeUnavailable => {
            unavailable(Unavailable::TerminalRuntimeUnavailable)
        }
        BindingViewerPreparationError::TerminalUnavailable => {
            rejected(Rejected::TerminalUnavailable)
        }
        BindingViewerPreparationError::WorkbenchUnavailable => {
            unavailable(Unavailable::WorkbenchUnavailable)
        }
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

#[cfg(test)]
mod tests;
