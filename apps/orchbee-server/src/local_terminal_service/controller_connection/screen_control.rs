//! Clear and reset are admitted viewer effects, each with its own transient input capability.
use super::{
    LocalTerminalService, OperationId, Outcome, Rejected, TargetLifecycle, Unavailable,
    ViewerAttachment, WorkerAction, WorkerOperationRequest, WorkerOperationResult, WorkerOutcome,
    WorkerRequest, WorkerRequestError, placement, rejected, unavailable, worker_rejection,
};

/// Runs `action`, which is `Clear` or `Reset`; the carrier admits only that action's results.
pub(super) fn execute(
    service: &LocalTerminalService,
    viewer: &ViewerAttachment<'_>,
    action: WorkerAction,
) -> Outcome {
    let Some(template) = viewer.cleanup else {
        return rejected(Rejected::ViewerUnavailable);
    };
    let command = {
        let Ok(workers) = service.workers.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        let current = placement(workers.request_handle().as_ref());
        if !viewer.handle.is_active() || current != placement(Some(&viewer.handle)) {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        let Ok(mut coordinator) = service.coordinator.lock() else {
            return unavailable(Unavailable::ServerBusy);
        };
        if coordinator.target_lifecycle(viewer.target) != TargetLifecycle::Unfenced
            || !coordinator.binding_viewer_is_current(viewer.target, current)
        {
            return rejected(Rejected::ViewerUnavailable);
        }
        let mut allocate = || {
            coordinator
                .allocate_identity()
                .and_then(|bytes| OperationId::from_bytes(bytes).ok())
        };
        let (Some(request_id), Some(attachment_id)) = (allocate(), allocate()) else {
            return unavailable(Unavailable::ServerBusy);
        };
        WorkerRequest {
            request_id,
            viewer_id: template.attachment_id,
            controller_id: attachment_id,
            worker_id: template.worker_id,
            worker_generation: template.worker_generation,
            action,
        }
    };
    // Admission above fixes the old physical target; later binding changes never redirect it.
    match viewer
        .handle
        .request(WorkerOperationRequest::Control(command))
    {
        Ok(WorkerOperationResult::Control(result)) => match result.outcome {
            WorkerOutcome::Cleared {
                generation,
                revision,
                cleanup_confirmed,
            } => Outcome::Cleared {
                generation,
                revision,
                cleanup_confirmed,
            },
            WorkerOutcome::ResetAccepted {
                generation,
                revision,
                cleanup_confirmed,
            } => Outcome::ResetAccepted {
                generation,
                revision,
                cleanup_confirmed,
            },
            WorkerOutcome::ClearOutcomeUnknown | WorkerOutcome::ResetOutcomeUnknown => {
                Outcome::OutcomeUnknown {}
            }
            WorkerOutcome::Rejected(reason) => rejected(worker_rejection(reason)),
            WorkerOutcome::Acquired
            | WorkerOutcome::Released
            | WorkerOutcome::InputAccepted
            | WorkerOutcome::Resized(_)
            | WorkerOutcome::Pasted { .. }
            | WorkerOutcome::HostColorsAccepted => Outcome::OutcomeUnknown {},
        },
        Err(WorkerRequestError::Backpressure) => unavailable(Unavailable::ServerBusy),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Outcome::OutcomeUnknown {},
        // The carrier checks the operation kind; other transport failures mean no receipt.
        _ => unavailable(Unavailable::TerminalRuntimeUnavailable),
    }
}
