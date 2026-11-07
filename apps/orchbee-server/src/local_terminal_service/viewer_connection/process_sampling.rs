//! One temporary process/screen viewer; unsettled cleanup blocks the sampler's next attachment.
use super::{
    BindingViewerTarget, ClientTerminalId, ClientWorkerId, IssuedViewer, LocalTerminalService,
    ObcsSessionId, TargetLifecycle, WorkerOperationRequest, WorkerOperationResult, WorkerOutcome,
    WorkerRequest, WorkerRequestError, WorkerRequestHandle,
};
use crate::agent_screen::ScreenDetector;
use crate::session_coordinator::ProcessObservationTarget;
use std::sync::atomic::{AtomicBool, Ordering};
pub(in crate::local_terminal_service) mod screen;

pub(in crate::local_terminal_service) struct CapturedAgentExplanation {
    pub(in crate::local_terminal_service) explanation:
        orchbee_workbench_protocol::AgentScreenExplanation,
    pub(in crate::local_terminal_service) sample: orchbee_workbench_protocol::AgentDetectionSample,
}

/// Owned by the periodic sampler until exact Detach/Disposed, or matching Session shutdown proves Worker exit.
pub(in crate::local_terminal_service) struct PendingCleanup {
    handle: WorkerRequestHandle,
    target: BindingViewerTarget,
    template: WorkerRequest,
}
impl PendingCleanup {
    pub(in crate::local_terminal_service) fn matches_target(
        &self,
        target: BindingViewerTarget,
    ) -> bool {
        self.target == target
    }
    #[cfg(windows)]
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
    pub(in crate::local_terminal_service) fn sample_agent_explanation(
        &self,
        target: &ProcessObservationTarget,
        stop: &AtomicBool,
        detector: &ScreenDetector,
        provider: orchbee_agent_observation_protocol::AgentReportProvider,
    ) -> (Option<CapturedAgentExplanation>, Option<PendingCleanup>) {
        let mut explanation = None;
        let pending = self.with_observation_viewer(target, stop, |viewer, template| {
            explanation = screen::explain(viewer, template, stop, detector, provider);
        });
        (explanation, pending)
    }
    /// One live terminal's close evidence on a temporary viewer, each fact published as soon as
    /// it is read.
    pub(in crate::local_terminal_service) fn sample_close_facts(
        &self,
        target: &ProcessObservationTarget,
        stop: &AtomicBool,
        publish: &mut dyn FnMut(crate::local_terminal_service::tab_close::evidence::CloseFacts),
    ) -> Option<PendingCleanup> {
        self.with_observation_viewer(target, stop, |viewer, template| {
            screen::close_facts(viewer, template, stop, publish);
        })
    }
    pub(in crate::local_terminal_service) fn sample_process_target(
        &self,
        target: &ProcessObservationTarget,
        stop: &AtomicBool,
        detector: &ScreenDetector,
    ) -> Option<PendingCleanup> {
        self.with_observation_viewer(target, stop, |viewer, template| {
            screen::observe(viewer, template, stop, detector);
        })
    }

    /// One restored terminal's resume evidence on a temporary viewer.
    #[cfg(windows)]
    pub(in crate::local_terminal_service) fn sample_resume_facts(
        &self,
        target: &ProcessObservationTarget,
        stop: &AtomicBool,
    ) -> (
        Option<crate::local_terminal_service::agent_resume::ResumeFacts>,
        Option<PendingCleanup>,
    ) {
        let mut facts = None;
        let pending = self.with_observation_viewer(target, stop, |viewer, template| {
            facts = Some(screen::resume_facts(viewer, template, stop));
        });
        (facts, pending)
    }

    #[cfg(windows)]
    pub(in crate::local_terminal_service) fn sample_checkpoint_directory(
        &self,
        target: &ProcessObservationTarget,
    ) -> (
        Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
        Option<PendingCleanup>,
    ) {
        self.sample_current_directory(target, &AtomicBool::new(false))
    }

    /// The directory `target`'s shell reported, else its root process's. Only Windows Hosts bound
    /// a process sample, so other platforms read nothing.
    pub(in crate::local_terminal_service) fn sample_current_directory(
        &self,
        target: &ProcessObservationTarget,
        stop: &AtomicBool,
    ) -> (
        Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
        Option<PendingCleanup>,
    ) {
        #[cfg(windows)]
        {
            let mut directory = None;
            let pending = self.with_observation_viewer(target, stop, |viewer, template| {
                directory = screen::directory(viewer, template, stop);
            });
            (directory, pending)
        }
        #[cfg(not(windows))]
        {
            let _ = (target, stop);
            (None, None)
        }
    }

    pub(in crate::local_terminal_service) fn with_observation_viewer(
        &self,
        target: &ProcessObservationTarget,
        stop: &AtomicBool,
        observe: impl FnOnce(&mut super::ViewerAttachment<'_>, WorkerRequest),
    ) -> Option<PendingCleanup> {
        if stop.load(Ordering::Acquire) {
            return None;
        }
        let (Ok(session), Ok(worker), Ok(terminal)) = (
            ObcsSessionId::parse(target.session_id.as_str()),
            ClientWorkerId::parse(target.binding.worker_id.as_str()),
            ClientTerminalId::parse(target.binding.terminal_id.as_str()),
        ) else {
            return None;
        };
        let Ok(issued) = self.issue_binding_viewer(&session, &worker, &terminal) else {
            return None;
        };
        self.with_issued_observation_viewer(target, stop, issued, observe)
    }

    pub(in crate::local_terminal_service) fn with_reference_observation_viewer(
        &self,
        target: &ProcessObservationTarget,
        reference: &orchbee_client_server_protocol::RequestId,
        stop: &AtomicBool,
        observe: impl FnOnce(&mut super::ViewerAttachment<'_>, WorkerRequest),
    ) -> Option<PendingCleanup> {
        if stop.load(Ordering::Acquire) {
            return None;
        }
        let Ok(issued) = self.issue_viewer(reference) else {
            return None;
        };
        self.with_issued_observation_viewer(target, stop, issued, observe)
    }

    fn with_issued_observation_viewer(
        &self,
        target: &ProcessObservationTarget,
        stop: &AtomicBool,
        mut issued: IssuedViewer<'_>,
        observe: impl FnOnce(&mut super::ViewerAttachment<'_>, WorkerRequest),
    ) -> Option<PendingCleanup> {
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
            // Action/result admission belongs to the protocol; this step consumes only its requested viewer record or acknowledgment.
            _ => false,
        };
        if attached
            && !stop.load(Ordering::Acquire)
            && self.viewer_worker_is_current(&issued.viewer.handle)
            && self.target_lifecycle(issued.viewer.target) == Some(TargetLifecycle::Unfenced)
        {
            observe(&mut issued.viewer, template);
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
