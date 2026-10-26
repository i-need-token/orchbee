//! Admits retained close lifecycle work into the Worker carrier and wakes its owner when the
//! exact result settles. It preserves an outstanding physical request after local wait
//! cancellation without advancing the close state machine itself.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::Instant,
};

use orchbee_capability_node_protocol::{
    TerminalLifecycleRequest, TerminalLifecycleResult, encode_message,
};

use super::{
    AdmitOutcome, CompletionDelivery, PendingRequest, RequestLane, WorkerOperationRequest,
    WorkerOperationResult, WorkerRequestError, WorkerRequestHandle, WorkerResponseFailure,
};

#[must_use = "an admitted lifecycle request must be settled even after its attempt deadline"]
pub(crate) struct PendingLifecycleResult {
    result: Receiver<Result<WorkerOperationResult, WorkerRequestError>>,
    complete_write: Arc<AtomicBool>,
    admitted_observed_at: Instant,
}

impl PendingLifecycleResult {
    pub(crate) const fn admitted_observed_at(&self) -> Instant {
        self.admitted_observed_at
    }

    pub(crate) fn wait(self) -> Result<TerminalLifecycleResult, WorkerRequestError> {
        match self.result.recv() {
            Ok(Ok(WorkerOperationResult::Lifecycle(result))) => Ok(result),
            Ok(Ok(_)) => Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
                WorkerResponseFailure::WrongResultKind,
            )),
            Ok(Err(error)) => Err(error),
            Err(_) if self.complete_write.load(Ordering::Acquire) => {
                Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
                    WorkerResponseFailure::CarrierStopped,
                ))
            }
            Err(_) => Err(WorkerRequestError::Unavailable),
        }
    }
}

impl WorkerRequestHandle {
    pub(crate) fn submit_lifecycle(
        &self,
        request: TerminalLifecycleRequest,
    ) -> Result<super::PendingLifecycleResult, WorkerRequestError> {
        if !self.is_active() {
            return Err(WorkerRequestError::Unavailable);
        }
        if request.authorization.worker_id.as_bytes() != self.worker_id.as_bytes() {
            return Err(WorkerRequestError::WrongWorker);
        }
        if request.authorization.worker_generation.get() != self.generation.get() {
            return Err(WorkerRequestError::WrongGeneration);
        }
        let request = WorkerOperationRequest::Lifecycle(request);
        let encoded =
            encode_message(&request.message()).map_err(WorkerRequestError::InvalidRequest)?;
        let (completion, result) = mpsc::sync_channel(1);
        let complete_write = Arc::new(AtomicBool::new(false));
        let pending = PendingRequest {
            completion: CompletionDelivery::for_request(&request, completion),
            complete_write: Arc::clone(&complete_write),
            encoded,
            request,
        };
        match self.queue.try_admit(RequestLane::High, pending) {
            AdmitOutcome::Admitted => Ok(PendingLifecycleResult {
                result,
                complete_write,
                // An observation after enqueue, not the queue's linearization instant.
                admitted_observed_at: Instant::now(),
            }),
            AdmitOutcome::Full if self.is_active() => Err(WorkerRequestError::Backpressure),
            AdmitOutcome::Full | AdmitOutcome::Closed => Err(WorkerRequestError::Unavailable),
        }
    }
}
