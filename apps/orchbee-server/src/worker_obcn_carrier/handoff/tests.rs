use super::super::{
    CapabilityNodeMessage as Message, CompletionDelivery, RequestLane, WorkerCarrierShutdown,
    WorkerRequestHandle, WorkerResponseFailure, WorkerWireExit, create_worker_carrier,
    encode_message, validate_response,
};
use super::*;
use orchbee_capability_node_protocol::{
    AbsoluteCwd, OperationId, TerminalCwdValidationOutcome, TerminalCwdValidationRequest,
    TerminalCwdValidationResult, WorkerGeneration, WorkerHandoffId, WorkerHandoffResult,
    WorkerId as CapabilityWorkerId, read_message, write_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_site_protocol::WorkerId;
use std::{
    num::NonZeroU64,
    sync::{Arc, atomic::AtomicBool, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

const WORKER: WorkerId = WorkerId::from_bytes([2; 16]);
fn request(action: Action) -> WorkerHandoffRequest {
    WorkerHandoffRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: CapabilityWorkerId::from_bytes(WORKER.as_bytes()).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(3).unwrap()),
        target: WorkerHandoffTarget {
            handoff_id: WorkerHandoffId::from_bytes([4; 16]).unwrap(),
            cohort_sha256: [5; 32],
        },
        action,
    }
}
fn cwd() -> TerminalCwdValidationRequest {
    TerminalCwdValidationRequest {
        operation_id: OperationId::from_bytes([6; 16]).unwrap(),
        worker_id: request(Action::Prepare).worker_id,
        worker_generation: request(Action::Prepare).worker_generation,
        cwd: AbsoluteCwd::new(if cfg!(windows) {
            r"C:\missing"
        } else {
            "/missing"
        })
        .unwrap(),
    }
}
fn pending(
    request: WorkerOperationRequest,
) -> (
    PendingRequest,
    mpsc::Receiver<Result<WorkerOperationResult, WorkerRequestError>>,
) {
    let (completion, receiver) = mpsc::sync_channel(1);
    (
        PendingRequest {
            encoded: encode_message(&request.message()).unwrap(),
            request,
            completion: CompletionDelivery::Required(completion),
            complete_write: Arc::new(AtomicBool::new(false)),
        },
        receiver,
    )
}
fn response(
    request: WorkerHandoffRequest,
    outcome: Outcome,
) -> Result<WorkerOperationResult, WorkerRequestError> {
    Ok(WorkerOperationResult::Handoff(
        WorkerHandoffResult::new(request, outcome).unwrap(),
    ))
}
fn admit(
    queue: &CarrierQueue,
    request: WorkerHandoffRequest,
) -> mpsc::Receiver<Result<WorkerOperationResult, WorkerRequestError>> {
    let (pending, receiver) = pending(request.into());
    assert_eq!(queue.admit_handoff(pending), AdmitOutcome::Admitted);
    receiver
}

#[test]
fn handoff_queue_fences_both_lanes_settles_unwritten_requests_and_owns_one_control_exchange() {
    let queue = CarrierQueue::new();
    let (high, high_result) = pending(WorkerOperationRequest::CwdValidation(cwd()));
    let (low, low_result) = pending(WorkerOperationRequest::CwdValidation(cwd()));
    assert_eq!(
        queue.try_admit(RequestLane::High, high),
        AdmitOutcome::Admitted
    );
    assert_eq!(
        queue.try_admit(RequestLane::Low, low),
        AdmitOutcome::Admitted
    );
    let _prepare_result = admit(&queue, request(Action::Prepare));
    assert_eq!(
        high_result.recv().unwrap(),
        Err(WorkerRequestError::Unavailable)
    );
    assert_eq!(
        low_result.recv().unwrap(),
        Err(WorkerRequestError::Unavailable)
    );
    for lane in [RequestLane::High, RequestLane::Low] {
        assert_eq!(
            queue.try_admit(
                lane,
                pending(WorkerOperationRequest::CwdValidation(cwd())).0
            ),
            AdmitOutcome::Closed
        );
    }
    assert!(!queue.block_admit_high(pending(WorkerOperationRequest::CwdValidation(cwd())).0));
    assert_eq!(
        queue.admit_handoff(pending(request(Action::Commit).into()).0),
        AdmitOutcome::Full,
        "Prepare result is still pending"
    );
    let dispatched = queue.dispatch_next().unwrap();
    assert_eq!(
        dispatched.request,
        WorkerOperationRequest::Handoff(request(Action::Prepare))
    );
    queue.complete_handoff(
        request(Action::Prepare),
        &response(request(Action::Prepare), Outcome::Prepared),
    );
    let mut wrong = request(Action::Commit);
    wrong.target.cohort_sha256[0] ^= 1;
    assert_eq!(
        queue.admit_handoff(pending(wrong.into()).0),
        AdmitOutcome::Full
    );
    let _abort_result = admit(&queue, request(Action::Abort));
    assert_eq!(
        queue.admit_handoff(pending(request(Action::Commit).into()).0),
        AdmitOutcome::Full,
        "one queued or dispatched handoff at a time"
    );
    assert_eq!(
        queue.dispatch_next().unwrap().request,
        WorkerOperationRequest::Handoff(request(Action::Abort))
    );
    queue.complete_handoff(
        request(Action::Abort),
        &response(request(Action::Abort), Outcome::Aborted),
    );
    assert_eq!(
        queue.try_admit(
            RequestLane::High,
            pending(WorkerOperationRequest::CwdValidation(cwd())).0
        ),
        AdmitOutcome::Admitted
    );
    for request in queue.close_and_drain() {
        let _delivered = send_completion(request, Err(WorkerRequestError::Unavailable));
    }
}

#[test]
fn handoff_known_refusals_reopen_only_when_worker_confirms_no_prepared_owner() {
    for reason in [
        Refusal::TargetUnavailable,
        Refusal::Authentication,
        Refusal::AttachmentsBusy,
        Refusal::Capacity,
        Refusal::ReusedTicket,
        Refusal::Unmanaged,
        Refusal::WrongWorker,
        Refusal::Busy,
        Refusal::TicketMismatch,
    ] {
        let queue = CarrierQueue::new();
        let _receipt = admit(&queue, request(Action::Prepare));
        let dispatched = queue.dispatch_next().unwrap();
        queue.complete_handoff(
            request(Action::Prepare),
            &response(request(Action::Prepare), Outcome::Refused(reason)),
        );
        drop(dispatched);
        assert_eq!(
            queue.lock().handoff.is_some(),
            matches!(reason, Refusal::Busy | Refusal::TicketMismatch)
        );
    }
    for action in [Action::Abort, Action::Commit] {
        let queue = CarrierQueue::new();
        let _receipt = admit(&queue, request(Action::Prepare));
        drop(queue.dispatch_next().unwrap());
        queue.complete_handoff(
            request(Action::Prepare),
            &response(request(Action::Prepare), Outcome::Prepared),
        );
        let _receipt = admit(&queue, request(action));
        drop(queue.dispatch_next().unwrap());
        queue.complete_handoff(
            request(action),
            &response(request(action), Outcome::Refused(Refusal::NotPrepared)),
        );
        assert!(queue.lock().handoff.is_none());
    }
}

#[test]
fn handoff_unknown_outcome_keeps_fence_and_committed_outcome_permanently_closes_queue() {
    let queue = CarrierQueue::new();
    let _receipt = admit(&queue, request(Action::Prepare));
    drop(queue.dispatch_next().unwrap());
    queue.complete_handoff(
        request(Action::Prepare),
        &Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::Disconnected,
        )),
    );
    assert!(queue.lock().handoff.as_ref().unwrap().pending);
    drop(queue);
    let queue = CarrierQueue::new();
    let _receipt = admit(&queue, request(Action::Prepare));
    drop(queue.dispatch_next().unwrap());
    queue.complete_handoff(
        request(Action::Prepare),
        &response(request(Action::Prepare), Outcome::Prepared),
    );
    let _receipt = admit(&queue, request(Action::Commit));
    drop(queue.dispatch_next().unwrap());
    queue.complete_handoff(
        request(Action::Commit),
        &response(request(Action::Commit), Outcome::Committed),
    );
    assert!(queue.lock().closed);
    assert_eq!(
        queue.admit_handoff(pending(request(Action::Prepare).into()).0),
        AdmitOutcome::Closed
    );
    assert_eq!(
        queue.try_admit(
            RequestLane::Low,
            pending(WorkerOperationRequest::CwdValidation(cwd())).0
        ),
        AdmitOutcome::Closed
    );
}

#[test]
fn handoff_prepare_releases_blocked_cleanup_and_refuses_concurrent_shutdown() {
    use super::super::HIGH_LANE_CAPACITY;
    use orchbee_capability_node_protocol::{ShutdownBudget, WorkerShutdownRequest};
    let queue = Arc::new(CarrierQueue::new());
    let mut receipts = Vec::new();
    for _ in 0..HIGH_LANE_CAPACITY {
        let (pending, receipt) = pending(WorkerOperationRequest::CwdValidation(cwd()));
        assert_eq!(
            queue.try_admit(RequestLane::High, pending),
            AdmitOutcome::Admitted
        );
        receipts.push(receipt);
    }
    let (cleanup, cleanup_receipt) = pending(WorkerOperationRequest::CwdValidation(cwd()));
    let waiting = Arc::clone(&queue);
    let cleanup = thread::spawn(move || waiting.block_admit_high(cleanup));
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while queue.lock().high_waiters.is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "cleanup must enter its owned waiter slot"
        );
        thread::yield_now();
    }
    let _prepare = admit(&queue, request(Action::Prepare));
    assert!(!cleanup.join().unwrap());
    receipts.push(cleanup_receipt);
    for receipt in receipts {
        assert_eq!(
            receipt.recv().unwrap(),
            Err(WorkerRequestError::Unavailable)
        );
    }
    let stop = WorkerShutdownRequest {
        operation_id: request(Action::Prepare).operation_id,
        worker_id: request(Action::Prepare).worker_id,
        worker_generation: request(Action::Prepare).worker_generation,
        budget: ShutdownBudget::new(100).unwrap(),
    };
    assert_eq!(
        queue.admit_shutdown(pending(WorkerOperationRequest::Shutdown(stop)).0),
        AdmitOutcome::Full
    );
    drop(queue.dispatch_next().unwrap());
    queue.complete_handoff(
        request(Action::Prepare),
        &response(
            request(Action::Prepare),
            Outcome::Refused(Refusal::AttachmentsBusy),
        ),
    );
    assert_eq!(
        queue.admit_shutdown(pending(WorkerOperationRequest::Shutdown(stop)).0),
        AdmitOutcome::Admitted
    );
    for pending in queue.close_and_drain() {
        let _delivered = send_completion(pending, Err(WorkerRequestError::Unavailable));
    }
}

#[test]
fn handoff_carrier_rejects_every_echo_mismatch_and_handshake_on_the_normal_wire() {
    let expected = request(Action::Prepare);
    for field in 0..6 {
        let mut actual = expected;
        match field {
            0 => actual.operation_id = OperationId::from_bytes([9; 16]).unwrap(),
            1 => actual.worker_id = CapabilityWorkerId::from_bytes([9; 16]).unwrap(),
            2 => actual.worker_generation = WorkerGeneration::new(NonZeroU64::MIN),
            3 => actual.target.handoff_id = WorkerHandoffId::from_bytes([9; 16]).unwrap(),
            4 => actual.target.cohort_sha256[0] ^= 1,
            5 => actual.action = Action::Abort,
            _ => unreachable!(),
        }
        let result =
            WorkerHandoffResult::new(actual, Outcome::Refused(Refusal::Authentication)).unwrap();
        assert_eq!(
            validate_response(&expected.into(), Message::WorkerHandoffResult(result)).unwrap_err(),
            if field == 0 {
                WorkerResponseFailure::MismatchedCorrelation
            } else {
                WorkerResponseFailure::MismatchedAuthorization
            }
        );
    }
    for (message, failure) in [
        (
            Message::WorkerHandoffRequest(expected),
            WorkerResponseFailure::WrongDirection,
        ),
        (
            Message::WorkerHandoffReady(expected.peer()),
            WorkerResponseFailure::WrongDirection,
        ),
        (
            Message::WorkerHandoffHello(expected.peer()),
            WorkerResponseFailure::WrongResultKind,
        ),
        (
            Message::WorkerHandoffOwned(expected.peer()),
            WorkerResponseFailure::WrongResultKind,
        ),
    ] {
        assert_eq!(
            validate_response(&expected.into(), message).unwrap_err(),
            failure
        );
    }
}

struct Wire {
    handle: WorkerRequestHandle,
    stop: Option<WorkerCarrierShutdown>,
    owner: Option<JoinHandle<WorkerWireExit>>,
}
impl Wire {
    fn new() -> (Self, LocalStream) {
        let (server, worker) = LocalStream::pair().unwrap();
        worker
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        worker
            .set_write_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let (handle, stop, owner) =
            create_worker_carrier(WORKER, NonZeroU64::new(3).unwrap()).into_parts();
        let owner = thread::spawn(move || owner.run(server));
        (
            Self {
                handle,
                stop: Some(stop),
                owner: Some(owner),
            },
            worker,
        )
    }
    fn finish(mut self) -> WorkerWireExit {
        self.owner.take().unwrap().join().unwrap()
    }
}
impl Drop for Wire {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(owner) = self.owner.take() {
            owner.join().unwrap();
        }
    }
}
fn answer(peer: &mut LocalStream, request: WorkerHandoffRequest, outcome: Outcome) {
    assert_eq!(
        read_message(peer).unwrap(),
        Message::WorkerHandoffRequest(request)
    );
    write_message(
        peer,
        &Message::WorkerHandoffResult(WorkerHandoffResult::new(request, outcome).unwrap()),
    )
    .unwrap();
}

#[test]
fn handoff_actual_carrier_finishes_inflight_before_prepare_and_recovers_normal_admission_after_abort()
 {
    let (wire, mut worker) = Wire::new();
    let (seen, received) = mpsc::sync_channel(1);
    let (release, proceed) = mpsc::sync_channel(1);
    let peer = thread::spawn(move || {
        assert_eq!(
            read_message(&mut worker).unwrap(),
            Message::TerminalCwdValidationRequest(cwd())
        );
        seen.send(()).unwrap();
        proceed.recv_timeout(Duration::from_secs(4)).unwrap();
        write_message(
            &mut worker,
            &Message::TerminalCwdValidationResult(TerminalCwdValidationResult {
                request: cwd(),
                outcome: TerminalCwdValidationOutcome::DirectoryAvailable,
            }),
        )
        .unwrap();
        answer(&mut worker, request(Action::Prepare), Outcome::Prepared);
        answer(&mut worker, request(Action::Abort), Outcome::Aborted);
        assert_eq!(
            read_message(&mut worker).unwrap(),
            Message::TerminalCwdValidationRequest(cwd())
        );
        write_message(
            &mut worker,
            &Message::TerminalCwdValidationResult(TerminalCwdValidationResult {
                request: cwd(),
                outcome: TerminalCwdValidationOutcome::DirectoryAvailable,
            }),
        )
        .unwrap();
    });
    let handle = wire.handle.clone();
    let original = thread::spawn(move || handle.validate_terminal_cwd(cwd()));
    received.recv_timeout(Duration::from_secs(4)).unwrap();
    let prepare = wire
        .handle
        .admit(request(Action::Prepare).into(), false)
        .unwrap();
    assert_eq!(
        wire.handle.validate_terminal_cwd(cwd()).unwrap_err(),
        WorkerRequestError::Unavailable
    );
    release.send(()).unwrap();
    assert_eq!(
        original.join().unwrap().unwrap().outcome,
        TerminalCwdValidationOutcome::DirectoryAvailable
    );
    assert_eq!(
        prepare.recv().unwrap(),
        response(request(Action::Prepare), Outcome::Prepared).unwrap()
    );
    assert_eq!(
        wire.handle.request(request(Action::Abort).into()).unwrap(),
        response(request(Action::Abort), Outcome::Aborted).unwrap()
    );
    assert_eq!(
        wire.handle.validate_terminal_cwd(cwd()).unwrap().outcome,
        TerminalCwdValidationOutcome::DirectoryAvailable
    );
    peer.join().unwrap();
    assert_eq!(wire.finish(), WorkerWireExit::PeerClosed);
}

#[test]
fn handoff_actual_carrier_committed_receipt_retires_old_generation_before_another_write() {
    let (wire, mut worker) = Wire::new();
    let peer = thread::spawn(move || {
        answer(&mut worker, request(Action::Prepare), Outcome::Prepared);
        answer(&mut worker, request(Action::Commit), Outcome::Committed);
        let mut byte = [0];
        assert_eq!(
            std::io::Read::read(&mut worker, &mut byte).unwrap(),
            0,
            "old carrier must close, without waiting for Worker EOF"
        );
    });
    assert_eq!(
        wire.handle
            .request(request(Action::Prepare).into())
            .unwrap(),
        response(request(Action::Prepare), Outcome::Prepared).unwrap()
    );
    assert_eq!(
        wire.handle.request(request(Action::Commit).into()).unwrap(),
        response(request(Action::Commit), Outcome::Committed).unwrap()
    );
    assert!(!wire.handle.is_active());
    assert_eq!(
        wire.handle.validate_terminal_cwd(cwd()).unwrap_err(),
        WorkerRequestError::Unavailable
    );
    assert_eq!(wire.finish(), WorkerWireExit::Shutdown);
    peer.join().unwrap();
}
