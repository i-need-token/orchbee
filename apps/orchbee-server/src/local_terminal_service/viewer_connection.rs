use super::{LocalTerminalService, placement, projection::ViewerProjection};
use crate::session_coordinator::{
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
    ClientMessage, HostEpoch, RequestId, ServerMessage, SessionId as ObcsSessionId,
    TerminalId as ClientTerminalId, TerminalViewerAction as Action,
    TerminalViewerOutcome as Outcome, TerminalViewerRequest, TerminalViewerResult, ViewerId,
    ViewerRejectedReason as Rejected, ViewerUnavailableReason as Unavailable,
    WorkerId as ClientWorkerId,
};

#[cfg(windows)]
mod editor;
#[cfg(windows)]
pub(in crate::local_terminal_service) mod process_sampling;
mod routing;
#[cfg(windows)]
pub(super) use process_sampling::{CapturedAgentExplanation, PendingCleanup};

const MAXIMUM_CONNECTION_VIEWERS: usize = 8;

pub(crate) struct LocalTerminalConnection<'service> {
    service: &'service LocalTerminalService,
    viewers: [Option<ViewerAttachment<'service>>; MAXIMUM_CONNECTION_VIEWERS],
}

pub(super) struct ViewerAttachment<'service> {
    pub(super) attachment_id: ViewerId,
    service: &'service LocalTerminalService,
    pub(super) handle: WorkerRequestHandle,
    pub(super) target: BindingViewerTarget,
    pub(super) controller: Option<super::controller_connection::ControllerLease<'service>>,
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

impl<'service> LocalTerminalConnection<'service> {
    pub(super) const fn new(service: &'service LocalTerminalService) -> Self {
        Self {
            service,
            viewers: [const { None }; MAXIMUM_CONNECTION_VIEWERS],
        }
    }

    pub(crate) fn execute(&mut self, request: ClientMessage) -> ServerMessage {
        let request = match request {
            ClientMessage::TerminalControl(request) => {
                let outcome = super::controller_connection::execute(
                    self.service,
                    self.viewers
                        .iter_mut()
                        .flatten()
                        .find(|viewer| &viewer.attachment_id == request.action().attachment_id()),
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

    fn admit_viewer_slot(&mut self) -> bool {
        for entry in &mut self.viewers {
            let disposed = entry.as_ref().is_some_and(|viewer| {
                self.service.target_lifecycle(viewer.target) == Some(TargetLifecycle::Disposed)
            });
            // The exact monotonic proof was observed above; its gate is no longer held.
            if disposed && let Some(mut viewer) = entry.take() {
                viewer.cleanup = None;
                if let Some(controller) = viewer.controller.as_mut() {
                    controller.disarm_known_disposed();
                }
                drop(viewer);
            }
        }
        self.viewers.iter().any(Option::is_none)
    }
    fn attach(&mut self, issued: Result<IssuedViewer<'service>, Outcome>) -> Outcome {
        let IssuedViewer {
            action,
            request_id,
            mut viewer,
        } = match issued {
            Ok(issued) => issued,
            Err(outcome) => return outcome,
        };
        let Some(cleanup) = viewer.cleanup else {
            return unavailable(Unavailable::ServerBusy);
        };
        if self
            .viewers
            .iter()
            .flatten()
            .any(|existing| existing.target == viewer.target)
        {
            // This candidate never reached Worker Attach and owns no viewer cleanup.
            viewer.cleanup = None;
            return rejected(Rejected::AlreadyAttached);
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
                WorkerOutcome::Empty
                | WorkerOutcome::Record(..)
                | WorkerOutcome::FeedbackAccepted
                | WorkerOutcome::LifecycleObserved { .. }
                | WorkerOutcome::Detached
                | WorkerOutcome::RowsRejected(..) => {
                    unavailable(Unavailable::TerminalRuntimeUnavailable)
                }
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
        let Some(slot) = self.viewers.iter_mut().find(|entry| entry.is_none()) else {
            return unavailable(Unavailable::ServerBusy);
        };
        let Ok(host_terminal_id) = orchbee_client_server_protocol::HostTerminalId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(viewer.target.identity.terminal_id().as_bytes())
        )) else {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        };
        *slot = candidate.take();
        Outcome::Attached {
            attachment_id,
            host_epoch,
            host_terminal_id,
        }
    }
}

impl LocalTerminalService {
    pub(crate) const fn client_connection(&self) -> LocalTerminalConnection<'_> {
        LocalTerminalConnection::new(self)
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
        session_id: &ObcsSessionId,
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
            .prepare_binding_viewer(
                session_id,
                worker_id,
                terminal_id,
                placement(handle.as_ref()),
            )
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
                attachment_id: ViewerId::parse(&format!(
                    "{:032x}",
                    u128::from_be_bytes(identities.attachment.as_bytes())
                ))
                .map_err(|_| unavailable(Unavailable::ServerBusy))?,
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
