use std::{
    error::Error,
    io::{self, Read, Write},
    num::NonZeroU64,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::net::UnixStream,
    },
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

mod lifecycle;

use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, AuthorizedTerminalStart, CapabilityNodeMessage, HostCreationId,
    HostEpoch, HostTerminalId, HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds, LeaseId,
    OperationId, TerminalFence, TerminalId, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchRejection, TerminalLaunchResult, TerminalOperationAuthorization,
    TerminalOperationLease, TerminalPixelGeometry, TerminalStartOutcome, TerminalStartRejection,
    TerminalStartResult, WorkerGeneration, WorkerId as CapabilityWorkerId, read_message,
    write_message,
};
use orchbee_local_site_protocol::WorkerId;

use crate::worker_registry::{RegisteredWorkerWireOwner, WorkerAdmission, WorkerRegistry};

use super::{
    PendingRequest, WorkerCarrierLimits, WorkerCarrierShutdown, WorkerOperationRequest,
    WorkerOperationResult, WorkerRequestError, WorkerRequestHandle, WorkerResponseFailure,
    WorkerTransportFailure, WorkerWireExit, WorkerWireOwner, create_worker_carrier_with_limits,
};

const LOCAL_WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);
const GENERATION: NonZeroU64 = NonZeroU64::MIN;
const TEST_IO_TIMEOUT: Duration = Duration::from_millis(80);
const TEST_POLL_INTERVAL: Duration = Duration::from_millis(5);
const TEST_COMPLETION_TIMEOUT: Duration = Duration::from_secs(2);

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[test]
fn carries_launch_and_start_results_on_one_serialized_wire() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let expected_launch_request =
        launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let expected_start_request =
        start_request(authorization(2, 2, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let expected_launch = launch_result(&expected_launch_request);
    let expected_start = start_result(&expected_start_request, expected_start_request.identity())?;

    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(expected_launch_request)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(expected_launch),
        )?;
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalStart(expected_start_request)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalStartResult(expected_start),
        )?;
        Ok(())
    });

    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(launch_request(
            authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID,)?
        )?))?,
        WorkerOperationResult::Launch(expected_launch)
    );
    assert_eq!(
        handle.request(WorkerOperationRequest::Start(start_request(
            authorization(2, 2, GENERATION.get(), LOCAL_WORKER_ID,)?
        )?))?,
        WorkerOperationResult::Start(expected_start)
    );
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::PeerClosed
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn preserves_an_exact_worker_reported_unknown_result_before_peer_close() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let expected = TerminalLaunchResult::new(
        request.authorization(),
        TerminalLaunchOutcome::OutcomeUnknown {
            host_epoch: host_identity(1)?.host_epoch(),
        },
    )?;
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(request)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(expected),
        )?;
        Ok(())
    });

    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(launch_request(
            authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID,)?,
        )?))?,
        WorkerOperationResult::Launch(expected)
    );
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::PeerClosed
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn rejects_wrong_direction_and_result_kind_after_complete_delivery() -> TestResult {
    let request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let wrong_direction = CapabilityNodeMessage::AuthorizedTerminalLaunch(request.clone());
    assert_post_delivery_failure(
        WorkerOperationRequest::Launch(request.clone()),
        wrong_direction,
        WorkerResponseFailure::WrongDirection,
    )?;

    let start = start_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let wrong_kind =
        CapabilityNodeMessage::TerminalStartResult(start_result(&start, start.identity())?);
    assert_post_delivery_failure(
        WorkerOperationRequest::Launch(request),
        wrong_kind,
        WorkerResponseFailure::WrongResultKind,
    )
}

#[test]
fn rejects_mismatched_correlation_authorization_echo_and_start_target() -> TestResult {
    let request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let mismatched_correlation = TerminalLaunchResult::new(
        authorization(9, 1, GENERATION.get(), LOCAL_WORKER_ID)?,
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
            host_epoch: host_identity(1)?.host_epoch(),
        }),
    )?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Launch(request.clone()),
        CapabilityNodeMessage::TerminalLaunchResult(mismatched_correlation),
        WorkerResponseFailure::MismatchedCorrelation,
    )?;

    let mismatched_echo = TerminalLaunchResult::new(
        authorization(1, 2, GENERATION.get(), LOCAL_WORKER_ID)?,
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
            host_epoch: host_identity(1)?.host_epoch(),
        }),
    )?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Launch(request),
        CapabilityNodeMessage::TerminalLaunchResult(mismatched_echo),
        WorkerResponseFailure::MismatchedAuthorization,
    )?;

    let start = start_request(authorization(3, 3, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let mismatched_target = start_result(&start, host_identity(9)?)?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Start(start),
        CapabilityNodeMessage::TerminalStartResult(mismatched_target),
        WorkerResponseFailure::MismatchedTarget,
    )
}

#[test]
fn distinguishes_response_timeout_and_disconnect_after_complete_delivery() -> TestResult {
    let request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let (release_sender, release_receiver) = mpsc::channel();
    let worker_thread = thread::spawn(move || -> TestResult {
        let _request = read_message(&mut worker)?;
        release_receiver.recv()?;
        Ok(())
    });
    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(request.clone())),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::TimedOut
        ))
    );
    release_sender.send(())?;
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::RequestFailed
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;

    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let (request_seen_sender, request_seen_receiver) = mpsc::channel();
    let (disconnect_sender, disconnect_receiver) = mpsc::channel();
    let worker_thread = thread::spawn(move || -> TestResult {
        let _request = read_message(&mut worker)?;
        request_seen_sender.send(())?;
        disconnect_receiver.recv()?;
        Ok(())
    });
    let first_handle = handle.clone();
    let first_caller =
        thread::spawn(move || first_handle.request(WorkerOperationRequest::Launch(request)));
    request_seen_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;
    let queued_request = WorkerOperationRequest::Launch(launch_request(authorization(
        2,
        2,
        GENERATION.get(),
        LOCAL_WORKER_ID,
    )?)?);
    let (queued_completion, queued_result) = mpsc::sync_channel(1);
    handle.sender.try_send(PendingRequest {
        completion: Some(queued_completion),
        complete_write: Arc::new(AtomicBool::new(false)),
        encoded: orchbee_capability_node_protocol::encode_message(&queued_request.message())?,
        request: queued_request,
    })?;
    disconnect_sender.send(())?;
    assert_eq!(
        first_caller.join().map_err(|_panic| "requester panicked")?,
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::Disconnected
        ))
    );
    assert_eq!(
        queued_result.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        Err(WorkerRequestError::Unavailable)
    );
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::RequestFailed
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn reports_a_preexisting_peer_record_before_any_request_byte_as_not_delivered() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    worker.write_all(b"X")?;
    let request = WorkerOperationRequest::Launch(launch_request(authorization(
        1,
        1,
        GENERATION.get(),
        LOCAL_WORKER_ID,
    )?)?);
    let (completion, receiver) = mpsc::sync_channel(1);
    handle.sender.try_send(PendingRequest {
        completion: Some(completion),
        complete_write: Arc::new(AtomicBool::new(false)),
        encoded: orchbee_capability_node_protocol::encode_message(&request.message())?,
        request,
    })?;
    assert_eq!(owner.run(server), WorkerWireExit::UnexpectedPeerData);
    assert_eq!(
        receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        Err(WorkerRequestError::NotDelivered(
            WorkerTransportFailure::UnexpectedPeerData
        ))
    );
    Ok(())
}

#[test]
fn partial_request_frame_write_is_not_delivered() -> TestResult {
    let (stream, _peer) = UnixStream::pair()?;
    let mut stream = PartialWriteThenClose {
        bytes_written: 0,
        stream,
    };
    let request = WorkerOperationRequest::Launch(launch_request(authorization(
        1,
        1,
        GENERATION.get(),
        LOCAL_WORKER_ID,
    )?)?);
    let (completion, result) = mpsc::sync_channel(1);
    let pending = PendingRequest {
        completion: Some(completion),
        complete_write: Arc::new(AtomicBool::new(false)),
        encoded: orchbee_capability_node_protocol::encode_message(&request.message())?,
        request,
    };
    let carrier = create_worker_carrier_with_limits(
        LOCAL_WORKER_ID,
        GENERATION,
        WorkerCarrierLimits {
            io_timeout: TEST_IO_TIMEOUT,
            response_timeout: TEST_IO_TIMEOUT,
            poll_interval: TEST_POLL_INTERVAL,
        },
    );
    let (_handle, _shutdown, mut owner) = carrier.into_parts();
    owner.in_flight = Some(pending);

    assert!(!owner.exchange_in_flight(&mut stream));
    assert_eq!(stream.bytes_written, 1);
    assert_eq!(
        result.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        Err(WorkerRequestError::NotDelivered(
            WorkerTransportFailure::Closed
        ))
    );
    Ok(())
}

#[test]
fn completion_channel_loss_preserves_complete_write_ambiguity() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let caller = thread::spawn(move || handle.request(WorkerOperationRequest::Launch(request)));
    let pending = owner.receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;
    pending.complete_write.store(true, Ordering::Release);
    drop(pending);

    assert_eq!(
        caller.join().map_err(|_panic| "requester panicked")?,
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::CarrierStopped
        ))
    );
    Ok(())
}

#[test]
fn rejects_stale_routes_before_the_wire_without_retiring_the_carrier() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let wrong_worker = WorkerId::from_bytes([8; 16]);
    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(launch_request(
            authorization(1, 1, GENERATION.get(), wrong_worker,)?
        )?)),
        Err(WorkerRequestError::WrongWorker)
    );
    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(launch_request(
            authorization(1, 1, GENERATION.get() + 1, LOCAL_WORKER_ID,)?
        )?)),
        Err(WorkerRequestError::WrongGeneration)
    );

    let valid = launch_request(authorization(2, 2, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let expected = launch_result(&valid);
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(valid)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(expected),
        )?;
        Ok(())
    });
    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(launch_request(
            authorization(2, 2, GENERATION.get(), LOCAL_WORKER_ID,)?
        )?))?,
        WorkerOperationResult::Launch(expected)
    );
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::PeerClosed
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn bounds_one_wire_request_one_queued_request_and_rejects_more_work() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let first = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let second = launch_request(authorization(2, 2, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let third = launch_request(authorization(3, 3, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let first_result = launch_result(&first);
    let second_result = launch_result(&second);
    let (first_seen_sender, first_seen_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(first)
        );
        first_seen_sender.send(())?;
        release_receiver.recv()?;
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(first_result),
        )?;
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(second)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(second_result),
        )?;
        Ok(())
    });

    let first_handle = handle.clone();
    let first_request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let first_caller =
        thread::spawn(move || first_handle.request(WorkerOperationRequest::Launch(first_request)));
    first_seen_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;

    let second_request = WorkerOperationRequest::Launch(launch_request(authorization(
        2,
        2,
        GENERATION.get(),
        LOCAL_WORKER_ID,
    )?)?);
    let (second_completion, second_receiver) = mpsc::sync_channel(1);
    handle.sender.try_send(PendingRequest {
        completion: Some(second_completion),
        complete_write: Arc::new(AtomicBool::new(false)),
        encoded: orchbee_capability_node_protocol::encode_message(&second_request.message())?,
        request: second_request,
    })?;
    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(third)),
        Err(WorkerRequestError::Backpressure)
    );

    release_sender.send(())?;
    assert_eq!(
        first_caller
            .join()
            .map_err(|_panic| "requester panicked")??,
        WorkerOperationResult::Launch(first_result)
    );
    assert_eq!(
        second_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)??,
        WorkerOperationResult::Launch(second_result)
    );
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::PeerClosed
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn idle_peer_data_is_rejected_and_explicit_shutdown_is_bounded() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    write_message(
        &mut worker,
        &CapabilityNodeMessage::AuthorizedTerminalLaunch(request),
    )?;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::UnexpectedPeerData
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    assert!(!handle.is_active());

    let (_handle, owner, shutdown, server, _worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    drop(shutdown);
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::Shutdown
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn partial_response_cannot_extend_the_finite_read() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let (release_sender, release_receiver) = mpsc::channel();
    let worker_thread = thread::spawn(move || -> TestResult {
        let _request = read_message(&mut worker)?;
        worker.write_all(b"OBCN")?;
        release_receiver.recv()?;
        Ok(())
    });
    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(request)),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::TimedOut
        ))
    );
    release_sender.send(())?;
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::RequestFailed
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn registered_owner_panic_preserves_delivery_drains_and_allows_same_worker_reconnect() -> TestResult
{
    let (registry, generation, handle, registered_owner) = registered_test_carrier()?;
    let first = enqueue_request(
        &handle,
        WorkerOperationRequest::Launch(launch_request(authorization(
            1,
            1,
            generation.get(),
            LOCAL_WORKER_ID,
        )?)?),
    )?;
    let (server, _worker) = UnixStream::pair()?;
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let owner_thread = thread::spawn(move || {
        registered_owner.run_configured_for_test(PanicDuringResponseRead {
            entered: Some(entered_sender),
            probed: false,
            release: release_receiver,
            stream: server,
        })
    });
    entered_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;
    let queued = enqueue_request(
        &handle,
        WorkerOperationRequest::Launch(launch_request(authorization(
            2,
            2,
            generation.get(),
            LOCAL_WORKER_ID,
        )?)?),
    )?;

    release_sender.send(())?;
    assert!(owner_thread.join().is_err());
    assert_eq!(
        first.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::CarrierStopped
        ))
    );
    assert_eq!(
        queued.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        Err(WorkerRequestError::Unavailable)
    );
    assert!(!handle.is_active());
    assert_same_worker_can_reconnect(&registry, generation)?;
    Ok(())
}

#[test]
fn lost_completion_receiver_retires_and_allows_same_worker_reconnect() -> TestResult {
    let (registry, generation, handle, registered_owner) = registered_test_carrier()?;
    let (server, mut worker) = UnixStream::pair()?;
    worker.set_read_timeout(Some(TEST_COMPLETION_TIMEOUT))?;
    worker.set_write_timeout(Some(TEST_COMPLETION_TIMEOUT))?;
    let request = launch_request(authorization(1, 1, generation.get(), LOCAL_WORKER_ID)?)?;
    let expected = launch_result(&request);
    let completion = enqueue_request(&handle, WorkerOperationRequest::Launch(request.clone()))?;
    drop(completion);

    let owner_thread = thread::spawn(move || registered_owner.run(server));
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(request)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(expected),
        )?;
        Ok(())
    });

    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        owner_thread
            .join()
            .map_err(|_panic| "wire owner panicked")?,
        WorkerWireExit::RequestFailed
    );
    assert!(!handle.is_active());
    assert_same_worker_can_reconnect(&registry, generation)?;
    Ok(())
}

#[test]
fn active_registration_cannot_retire_during_a_partial_write() -> TestResult {
    let (registry, generation, handle, registered_owner) = registered_test_carrier()?;
    let completion = enqueue_request(
        &handle,
        WorkerOperationRequest::Launch(launch_request(authorization(
            1,
            1,
            generation.get(),
            LOCAL_WORKER_ID,
        )?)?),
    )?;
    let (server, _worker) = UnixStream::pair()?;
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let owner_thread = thread::spawn(move || {
        registered_owner.run_configured_for_test(PartialWriteGate {
            bytes_written: 0,
            entered: Some(entered_sender),
            release: release_receiver,
            stream: server,
        })
    });
    entered_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;

    assert_attached_generation_cannot_retire(&registry, generation)?;
    release_sender.send(())?;
    assert_eq!(
        completion.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        Err(WorkerRequestError::NotDelivered(
            WorkerTransportFailure::Closed
        ))
    );
    assert_eq!(
        owner_thread
            .join()
            .map_err(|_panic| "wire owner panicked")?,
        WorkerWireExit::RequestFailed
    );
    assert_same_worker_can_reconnect(&registry, generation)?;
    Ok(())
}

#[test]
fn active_registration_cannot_retire_before_exact_result_publication() -> TestResult {
    let (registry, generation, handle, registered_owner) = registered_test_carrier()?;
    let (server, mut worker) = UnixStream::pair()?;
    worker.set_read_timeout(Some(TEST_COMPLETION_TIMEOUT))?;
    worker.set_write_timeout(Some(TEST_COMPLETION_TIMEOUT))?;
    let request = launch_request(authorization(1, 1, generation.get(), LOCAL_WORKER_ID)?)?;
    let expected = launch_result(&request);
    let caller_request = request.clone();
    let (request_seen_sender, request_seen_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(request)
        );
        request_seen_sender.send(())?;
        release_receiver.recv()?;
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(expected),
        )?;
        Ok(())
    });
    let owner_thread = thread::spawn(move || registered_owner.run(server));
    let caller =
        thread::spawn(move || handle.request(WorkerOperationRequest::Launch(caller_request)));
    request_seen_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;

    assert_attached_generation_cannot_retire(&registry, generation)?;
    release_sender.send(())?;
    assert_eq!(
        caller.join().map_err(|_panic| "requester panicked")??,
        WorkerOperationResult::Launch(expected)
    );
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        owner_thread
            .join()
            .map_err(|_panic| "wire owner panicked")?,
        WorkerWireExit::PeerClosed
    );
    assert_same_worker_can_reconnect(&registry, generation)?;
    Ok(())
}

fn assert_post_delivery_failure(
    request: WorkerOperationRequest,
    response: CapabilityNodeMessage,
    expected: WorkerResponseFailure,
) -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let worker_thread = thread::spawn(move || -> TestResult {
        let _request = read_message(&mut worker)?;
        write_message(&mut worker, &response)?;
        Ok(())
    });
    assert_eq!(
        handle.request(request),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(expected))
    );
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::RequestFailed
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

fn test_carrier() -> TestResult<(
    WorkerRequestHandle,
    WorkerWireOwner,
    WorkerCarrierShutdown,
    UnixStream,
    UnixStream,
)> {
    let (server, worker) = UnixStream::pair()?;
    worker.set_read_timeout(Some(TEST_COMPLETION_TIMEOUT))?;
    worker.set_write_timeout(Some(TEST_COMPLETION_TIMEOUT))?;
    let carrier = create_worker_carrier_with_limits(
        LOCAL_WORKER_ID,
        GENERATION,
        WorkerCarrierLimits {
            io_timeout: TEST_IO_TIMEOUT,
            response_timeout: TEST_IO_TIMEOUT,
            poll_interval: TEST_POLL_INTERVAL,
        },
    );
    let (handle, shutdown, owner) = carrier.into_parts();
    Ok((handle, owner, shutdown, server, worker))
}

fn spawn_owner(
    owner: WorkerWireOwner,
    stream: UnixStream,
    exit_sender: mpsc::Sender<WorkerWireExit>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let _ignored = exit_sender.send(owner.run(stream));
    })
}

fn registered_test_carrier() -> TestResult<(
    Arc<Mutex<WorkerRegistry>>,
    NonZeroU64,
    WorkerRequestHandle,
    RegisteredWorkerWireOwner,
)> {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let generation = {
        let mut registry = registry
            .lock()
            .map_err(|_poisoned| io::Error::other("Worker registry was poisoned"))?;
        let WorkerAdmission::Accepted { generation } = registry.admit(LOCAL_WORKER_ID) else {
            return Err(io::Error::other("test Worker was not admitted").into());
        };
        generation
    };
    let carrier = create_worker_carrier_with_limits(
        LOCAL_WORKER_ID,
        generation,
        WorkerCarrierLimits {
            io_timeout: TEST_IO_TIMEOUT,
            response_timeout: TEST_IO_TIMEOUT,
            poll_interval: TEST_POLL_INTERVAL,
        },
    );
    let handle = carrier.request_handle();
    let registered_owner = WorkerRegistry::register_carrier(&registry, generation, carrier)
        .ok_or_else(|| io::Error::other("test Worker carrier was not registered"))?;
    Ok((registry, generation, handle, registered_owner))
}

fn enqueue_request(
    handle: &WorkerRequestHandle,
    request: WorkerOperationRequest,
) -> TestResult<Receiver<Result<WorkerOperationResult, WorkerRequestError>>> {
    let (completion, result) = mpsc::sync_channel(1);
    handle
        .sender
        .try_send(PendingRequest {
            completion: Some(completion),
            complete_write: Arc::new(AtomicBool::new(false)),
            encoded: orchbee_capability_node_protocol::encode_message(&request.message())?,
            request,
        })
        .map_err(|_error| io::Error::other("test Worker request was not queued"))?;
    Ok(result)
}

fn assert_attached_generation_cannot_retire(
    registry: &Arc<Mutex<WorkerRegistry>>,
    generation: NonZeroU64,
) -> TestResult {
    let mut registry = registry
        .lock()
        .map_err(|_poisoned| io::Error::other("Worker registry was poisoned"))?;
    assert!(!registry.cancel_pending_admission(generation));
    assert_eq!(
        registry.admit(LOCAL_WORKER_ID),
        WorkerAdmission::Duplicate {
            active_generation: generation,
            active_worker_id: LOCAL_WORKER_ID,
        }
    );
    assert_eq!(
        registry.request_handle().map(|handle| handle.generation()),
        Some(generation)
    );
    Ok(())
}

fn assert_same_worker_can_reconnect(
    registry: &Arc<Mutex<WorkerRegistry>>,
    retired_generation: NonZeroU64,
) -> TestResult {
    let mut registry = registry
        .lock()
        .map_err(|_poisoned| io::Error::other("Worker registry was poisoned"))?;
    assert!(registry.request_handle().is_none());
    let WorkerAdmission::Accepted {
        generation: replacement_generation,
    } = registry.admit(LOCAL_WORKER_ID)
    else {
        return Err(io::Error::other("same Worker could not reconnect after cleanup").into());
    };
    assert!(replacement_generation > retired_generation);
    Ok(())
}

struct PanicDuringResponseRead {
    entered: Option<mpsc::Sender<()>>,
    probed: bool,
    release: Receiver<()>,
    stream: UnixStream,
}

impl Read for PanicDuringResponseRead {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        if !self.probed {
            self.probed = true;
            return Err(io::Error::from(io::ErrorKind::WouldBlock));
        }
        let entered = self
            .entered
            .take()
            .unwrap_or_else(|| panic!("panic response read may only be entered once"));
        entered
            .send(())
            .unwrap_or_else(|error| panic!("panic response read must be observed: {error}"));
        self.release
            .recv()
            .unwrap_or_else(|error| panic!("panic response read must be released: {error}"));
        panic!("injected wire owner panic");
    }
}

impl Write for PanicDuringResponseRead {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsFd for PanicDuringResponseRead {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

struct PartialWriteGate {
    bytes_written: usize,
    entered: Option<mpsc::Sender<()>>,
    release: Receiver<()>,
    stream: UnixStream,
}

impl Read for PartialWriteGate {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::WouldBlock))
    }
}

impl Write for PartialWriteGate {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.bytes_written == 0 {
            let written = self.stream.write(&buffer[..1])?;
            self.bytes_written += written;
            return Ok(written);
        }
        let entered = self
            .entered
            .take()
            .unwrap_or_else(|| panic!("partial write gate may only be entered once"));
        entered
            .send(())
            .unwrap_or_else(|error| panic!("partial write gate entry must be observed: {error}"));
        self.release
            .recv()
            .unwrap_or_else(|error| panic!("partial write gate must be released: {error}"));
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "injected partial request write failure",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsFd for PartialWriteGate {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

struct PartialWriteThenClose {
    bytes_written: usize,
    stream: UnixStream,
}

impl Read for PartialWriteThenClose {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other(
            "a partial request write must not reach response reading",
        ))
    }
}

impl Write for PartialWriteThenClose {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.bytes_written == 0 {
            let written = self.stream.write(&buffer[..1])?;
            self.bytes_written += written;
            return Ok(written);
        }
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "injected partial request write failure",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsFd for PartialWriteThenClose {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

fn authorization(
    operation: u8,
    fence: u64,
    generation: u64,
    worker_id: WorkerId,
) -> TestResult<TerminalOperationAuthorization> {
    Ok(TerminalOperationAuthorization::new(
        TerminalId::from_bytes([1; 16])?,
        OperationId::from_bytes([operation; 16])?,
        CapabilityWorkerId::from_bytes(worker_id.as_bytes())?,
        WorkerGeneration::new(nonzero(generation)?),
        TerminalFence::new(nonzero(fence)?),
        TerminalOperationLease::new(
            LeaseId::from_bytes([fence as u8; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(nonzero(10_000)?),
        ),
        orchbee_capability_node_protocol::TERMINAL_LAUNCH_CAPABILITY_VERSION,
    ))
}

fn launch_request(
    authorization: TerminalOperationAuthorization,
) -> TestResult<AuthorizedTerminalLaunch> {
    Ok(AuthorizedTerminalLaunch::new(
        authorization,
        None,
        TerminalLaunchMode::Deferred,
    ))
}

fn launch_result(request: &AuthorizedTerminalLaunch) -> TerminalLaunchResult {
    TerminalLaunchResult::new(
        request.authorization(),
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
            host_epoch: HostEpoch::from_bytes([1; 16])
                .unwrap_or_else(|error| panic!("test Host epoch must be valid: {error}")),
        }),
    )
    .unwrap_or_else(|error| panic!("test launch result must be valid: {error}"))
}

fn start_request(
    authorization: TerminalOperationAuthorization,
) -> TestResult<AuthorizedTerminalStart> {
    Ok(AuthorizedTerminalStart::new(
        authorization,
        host_identity(1)?,
        TerminalPixelGeometry::new(80, 24, 8, 16)?,
    ))
}

fn start_result(
    request: &AuthorizedTerminalStart,
    target: HostTerminalIdentity,
) -> TestResult<TerminalStartResult> {
    Ok(TerminalStartResult::new(
        request.authorization(),
        target,
        TerminalStartOutcome::Rejected(TerminalStartRejection::WrongWorker),
    )?)
}

fn host_identity(value: u8) -> TestResult<HostTerminalIdentity> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([value; 16])?,
        HostEpoch::from_bytes([value; 16])?,
        HostTerminalId::from_bytes([value; 16])?,
    ))
}

fn nonzero(value: u64) -> TestResult<NonZeroU64> {
    NonZeroU64::new(value).ok_or_else(|| "test value must be nonzero".into())
}

#[test]
fn queued_detach_is_not_lost_when_the_single_submission_slot_is_full() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, TerminalViewerResult,
    };
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        attachment_id: OperationId::from_bytes([2; 16])?,
        action: TerminalViewerAction::Poll,
    };
    // Hold the owner before start so the submission slot is deterministically full.
    let pending = enqueue_request(&handle, WorkerOperationRequest::Viewer(request))?;
    let cleanup = TerminalViewerRequest {
        request_id: OperationId::from_bytes([3; 16])?,
        action: TerminalViewerAction::Detach,
        ..request
    };
    let cleanup_handle = handle.clone();
    let cleanup_thread = thread::spawn(move || cleanup_handle.enqueue_detach(cleanup));
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    assert_eq!(
        read_message(&mut worker)?,
        CapabilityNodeMessage::TerminalViewerRequest(request)
    );
    write_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
            request_id: request.request_id,
            attachment_id: request.attachment_id,
            outcome: TerminalViewerOutcome::Empty,
        }),
    )?;
    assert!(pending.recv_timeout(TEST_COMPLETION_TIMEOUT)?.is_ok());
    assert_eq!(
        read_message(&mut worker)?,
        CapabilityNodeMessage::TerminalViewerRequest(cleanup)
    );
    write_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
            request_id: cleanup.request_id,
            attachment_id: cleanup.attachment_id,
            outcome: TerminalViewerOutcome::Detached,
        }),
    )?;
    cleanup_thread
        .join()
        .map_err(|_| "cleanup thread panicked")?;
    let next = enqueue_request(&handle, WorkerOperationRequest::Viewer(request))?;
    assert_eq!(
        read_message(&mut worker)?,
        CapabilityNodeMessage::TerminalViewerRequest(request)
    );
    write_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
            request_id: request.request_id,
            attachment_id: request.attachment_id,
            outcome: TerminalViewerOutcome::Empty,
        }),
    )?;
    assert!(
        next.recv_timeout(TEST_COMPLETION_TIMEOUT)?.is_ok(),
        "cleanup completion must keep the shared carrier alive"
    );
    drop(worker);
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::PeerClosed
    );
    owner_thread.join().map_err(|_| "owner panicked")?;
    Ok(())
}

#[test]
fn viewer_scope_and_outcome_mismatch_retire_the_carrier() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, TerminalViewerResult,
    };
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        attachment_id: OperationId::from_bytes([2; 16])?,
        action: TerminalViewerAction::Poll,
    };
    let result = TerminalViewerResult {
        request_id: request.request_id,
        attachment_id: request.attachment_id,
        outcome: TerminalViewerOutcome::Empty,
    };
    for (actual, expected) in [
        (
            TerminalViewerResult {
                request_id: OperationId::from_bytes([9; 16])?,
                ..result.clone()
            },
            WorkerResponseFailure::MismatchedCorrelation,
        ),
        (
            TerminalViewerResult {
                attachment_id: OperationId::from_bytes([9; 16])?,
                ..result.clone()
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
        (
            TerminalViewerResult {
                outcome: TerminalViewerOutcome::Attached,
                ..result
            },
            WorkerResponseFailure::WrongResultKind,
        ),
    ] {
        assert_post_delivery_failure(
            WorkerOperationRequest::Viewer(request),
            CapabilityNodeMessage::TerminalViewerResult(actual),
            expected,
        )?;
    }
    Ok(())
}

#[test]
fn controller_results_require_exact_request_viewer_grant_and_geometry() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalControlAction, TerminalControlOutcome, TerminalControlRequest,
        TerminalControlResult,
    };
    let geometry = TerminalPixelGeometry::new(80, 24, 8, 16)?;
    let request = TerminalControlRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        viewer_id: OperationId::from_bytes([2; 16])?,
        controller_id: OperationId::from_bytes([3; 16])?,
        action: TerminalControlAction::Resize {
            sequence: NonZeroU64::MIN,
            geometry,
        },
    };
    let result = TerminalControlResult {
        request_id: request.request_id,
        viewer_id: request.viewer_id,
        controller_id: request.controller_id,
        outcome: TerminalControlOutcome::Resized(geometry),
    };
    let wrong = OperationId::from_bytes([9; 16])?;
    for (result, failure) in [
        (
            TerminalControlResult {
                request_id: wrong,
                ..result
            },
            WorkerResponseFailure::MismatchedCorrelation,
        ),
        (
            TerminalControlResult {
                viewer_id: wrong,
                ..result
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
        (
            TerminalControlResult {
                controller_id: wrong,
                ..result
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
        (
            TerminalControlResult {
                outcome: TerminalControlOutcome::Resized(TerminalPixelGeometry::new(
                    81, 24, 8, 16,
                )?),
                ..result
            },
            WorkerResponseFailure::WrongResultKind,
        ),
    ] {
        assert_post_delivery_failure(
            WorkerOperationRequest::Control(request.clone()),
            CapabilityNodeMessage::TerminalControlResult(result),
            failure,
        )?;
    }
    Ok(())
}
