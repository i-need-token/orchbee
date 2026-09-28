//! One temporary process-only viewer; unsettled cleanup blocks the sampler's next attachment.
use super::{
    BindingViewerTarget, ClientSiteId, ClientTerminalId, ClientWorkerId, LocalTerminalService,
    TargetLifecycle, WorkerAction, WorkerOperationRequest, WorkerOperationResult, WorkerOutcome,
    WorkerRequest, WorkerRequestError, WorkerRequestHandle, routing,
};
use crate::site_coordinator::ProcessObservationTarget;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Owned by the periodic sampler until exact Detach/Disposed, or matching Site shutdown proves Worker exit.
pub(in crate::local_terminal_service) struct PendingCleanup {
    handle: WorkerRequestHandle,
    target: BindingViewerTarget,
    template: WorkerRequest,
}
impl PendingCleanup {
    pub(in crate::local_terminal_service) fn matches_worker(
        &self,
        handle: &WorkerRequestHandle,
    ) -> bool {
        self.handle.worker_id() == handle.worker_id()
            && self.handle.generation() == handle.generation()
    }
    pub(in crate::local_terminal_service) fn settle(&self, service: &LocalTerminalService) -> bool {
        if service.target_lifecycle(self.target) == Some(TargetLifecycle::Disposed) {
            return true;
        }
        if !service.viewer_worker_is_current(&self.handle) {
            return false;
        }
        let Some(request_id) = service.next_viewer_operation() else {
            return false;
        };
        // Exact-viewer Detach is idempotent. Every attempt gets a new correlation and consumes its completion.
        matches!(self.handle.request(WorkerOperationRequest::Viewer(WorkerRequest{request_id,..self.template})),
            Ok(WorkerOperationResult::Viewer(result)) if result.outcome==WorkerOutcome::Detached)
    }
}
impl LocalTerminalService {
    pub(in crate::local_terminal_service) fn sample_process_target(
        &self,
        target: &ProcessObservationTarget,
        stop: &AtomicBool,
    ) -> Option<PendingCleanup> {
        if stop.load(Ordering::Acquire) {
            return None;
        }
        let (Ok(site), Ok(worker), Ok(terminal)) = (
            ClientSiteId::parse(target.site_id.as_str()),
            ClientWorkerId::parse(target.binding.worker_id.as_str()),
            ClientTerminalId::parse(target.binding.terminal_id.as_str()),
        ) else {
            return None;
        };
        let Ok(mut issued) = self.issue_binding_viewer(&site, &worker, &terminal) else {
            return None;
        };
        if issued.viewer.target != target.target || stop.load(Ordering::Acquire) {
            // Nothing was submitted; this candidate never owned an attachment.
            issued.viewer.cleanup = None;
            return None;
        }
        let template = issued.viewer.cleanup.expect("issued viewer cleanup");
        let result = issued
            .viewer
            .handle
            .request(WorkerOperationRequest::Viewer(WorkerRequest {
                request_id: issued.request_id,
                action: issued.action,
                ..template
            }));
        let attached = match result {
            Ok(WorkerOperationResult::Viewer(result))
                if result.outcome == WorkerOutcome::Attached =>
            {
                true
            }
            Ok(WorkerOperationResult::Viewer(result))
                if matches!(result.outcome, WorkerOutcome::Rejected(_)) =>
            {
                issued.viewer.cleanup = None;
                return None;
            }
            Err(
                WorkerRequestError::Backpressure
                | WorkerRequestError::Unavailable
                | WorkerRequestError::InvalidRequest(_)
                | WorkerRequestError::WrongWorker
                | WorkerRequestError::WrongGeneration
                | WorkerRequestError::NotDelivered(_),
            ) => {
                issued.viewer.cleanup = None;
                return None;
            }
            _ => false,
        };
        if attached
            && !stop.load(Ordering::Acquire)
            && self.viewer_worker_is_current(&issued.viewer.handle)
            && self.target_lifecycle(issued.viewer.target) == Some(TargetLifecycle::Unfenced)
            && let Some(request_id) = self.next_viewer_operation()
        {
            let started = Instant::now();
            let result =
                issued
                    .viewer
                    .handle
                    .request(WorkerOperationRequest::Viewer(WorkerRequest {
                        request_id,
                        action: WorkerAction::ReadProcessInfo,
                        ..template
                    }));
            // Cancellation still consumes the required response, but no longer admits it as fresh state.
            if !stop.load(Ordering::Acquire)
                && let Ok(WorkerOperationResult::Viewer(result)) = result
                && let WorkerOutcome::Record(record) = result.outcome
            {
                routing::valid_process_record(&issued.viewer, request_id, &record, started);
            }
        }
        let pending = PendingCleanup {
            handle: issued.viewer.handle.clone(),
            target: issued.viewer.target,
            template: issued
                .viewer
                .cleanup
                .take()
                .expect("transfer owned cleanup"),
        };
        // The Attach attempt has settled before this cleanup; no Required receiver is abandoned.
        (!pending.settle(self)).then_some(pending)
    }
}
