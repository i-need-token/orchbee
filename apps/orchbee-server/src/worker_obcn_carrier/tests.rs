use orchbee_local_ipc::{AsLocalIo as AsFd, LocalIo as BorrowedFd, LocalStream as UnixStream};
use std::{
    error::Error,
    io::{self, Read, Write},
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

mod lifecycle;
mod worktree_mutation;

use orchbee_capability_node_protocol::{
    AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, CapabilityNodeMessage,
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity,
    LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId, SendCleanupStatus, SendKey, SendProgress,
    SendRecord, SendRecords, SendRejection, SendText, SendTextInput, TerminalCwdValidationOutcome,
    TerminalCwdValidationRequest, TerminalCwdValidationResult, TerminalFence, TerminalId,
    TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchRejection, TerminalLaunchResult,
    TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
    TerminalSendOutcome, TerminalSendRequest, TerminalSendResult, TerminalStartOutcome,
    TerminalStartRejection, TerminalStartResult, WorkerGeneration, WorkerId as CapabilityWorkerId,
    read_message, write_message,
};
use orchbee_local_worker_protocol::WorkerId;

use crate::worker_registry::{RegisteredWorkerWireOwner, WorkerAdmission, WorkerRegistry};

use super::{
    AdmitOutcome, CompletionDelivery, ExchangeWait, PendingRequest, RequestLane,
    WorkerCarrierLimits, WorkerCarrierShutdown, WorkerOperationRequest, WorkerOperationResult,
    WorkerRequestError, WorkerRequestHandle, WorkerResponseFailure, WorkerTransportFailure,
    WorkerWireExit, WorkerWireOwner, create_worker_carrier_with_limits, send_completion,
};

const LOCAL_WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);
const GENERATION: NonZeroU64 = NonZeroU64::MIN;
const TEST_IO_TIMEOUT: Duration = Duration::from_millis(80);
const TEST_POLL_INTERVAL: Duration = Duration::from_millis(5);
const TEST_COMPLETION_TIMEOUT: Duration = Duration::from_secs(2);

#[test]
fn editor_command_carrier_uses_low_lane_and_preserves_literal_selection_on_windows_pipe()
-> TestResult {
    use orchbee_capability_node_protocol::{
        LaunchSelection, TerminalEditorCommandOutcome, TerminalEditorCommandRequest,
        TerminalEditorCommandResult, TerminalEditorCommandSource,
    };
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let request = TerminalEditorCommandRequest {
        request_id: OperationId::from_bytes([98; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
    };
    assert_eq!(
        WorkerOperationRequest::EditorCommand(request).lane(),
        RequestLane::Low
    );
    let response = TerminalEditorCommandResult {
        request,
        outcome: TerminalEditorCommandOutcome::Resolved {
            source: TerminalEditorCommandSource::Visual,
            selection: LaunchSelection::new(
                r"C:\编辑 器\editor.exe".into(),
                vec!["".into(), "--wait".into(), "字面 & % ! 🙂".into()],
            )?,
        },
    };
    let expected = response.clone();
    let (sender, receiver) = mpsc::channel();
    let owner_task = spawn_owner(owner, server, sender);
    let worker_task = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::TerminalEditorCommandRequest(request)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalEditorCommandResult(response),
        )?;
        Ok(())
    });
    let observed = handle.request(WorkerOperationRequest::EditorCommand(request));
    worker_task.join().map_err(|_| "editor peer panicked")??;
    let exit = receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;
    owner_task
        .join()
        .map_err(|_| "editor wire owner panicked")?;
    assert_eq!(observed?, WorkerOperationResult::EditorCommand(expected));
    assert_eq!(exit, WorkerWireExit::PeerClosed);
    Ok(())
}

#[test]
fn editor_command_carrier_rejects_every_echo_mismatch_and_wrong_direction() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalEditorCommandOutcome, TerminalEditorCommandRefusal, TerminalEditorCommandRequest,
        TerminalEditorCommandResult,
    };
    let request = TerminalEditorCommandRequest {
        request_id: OperationId::from_bytes([98; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
    };
    let expected = WorkerOperationRequest::EditorCommand(request);
    let result = TerminalEditorCommandResult {
        request,
        outcome: TerminalEditorCommandOutcome::Refused(
            TerminalEditorCommandRefusal::InvalidCommand,
        ),
    };
    assert!(matches!(
        super::validate_response(
            &expected,
            CapabilityNodeMessage::TerminalEditorCommandResult(result.clone())
        ),
        Ok(WorkerOperationResult::EditorCommand(_))
    ));
    for field in 0..3 {
        let mut bad = result.clone();
        match field {
            0 => bad.request.request_id = OperationId::from_bytes([97; 16])?,
            1 => bad.request.worker_id = CapabilityWorkerId::from_bytes([99; 16])?,
            _ => bad.request.worker_generation = WorkerGeneration::new(NonZeroU64::MAX),
        }
        assert!(
            super::validate_response(
                &expected,
                CapabilityNodeMessage::TerminalEditorCommandResult(bad)
            )
            .is_err()
        );
    }
    assert!(matches!(
        super::validate_response(
            &expected,
            CapabilityNodeMessage::TerminalEditorCommandRequest(request)
        ),
        Err(WorkerResponseFailure::WrongDirection)
    ));
    let other =
        WorkerOperationRequest::Titles(orchbee_capability_node_protocol::TerminalTitlesRequest {
            request_id: request.request_id,
            worker_id: request.worker_id,
            worker_generation: request.worker_generation,
            identities: vec![host_identity(1)?],
        });
    assert!(matches!(
        super::validate_response(
            &other,
            CapabilityNodeMessage::TerminalEditorCommandResult(result)
        ),
        Err(WorkerResponseFailure::WrongResultKind)
    ));
    let wrong = orchbee_capability_node_protocol::TerminalTitlesResult {
        request_id: request.request_id,
        rows: vec![],
    };
    assert!(matches!(
        super::validate_response(
            &expected,
            CapabilityNodeMessage::TerminalTitlesResult(wrong)
        ),
        Err(WorkerResponseFailure::WrongResultKind)
    ));
    Ok(())
}

#[test]
fn bounded_discovery_admission_fences_route_and_preserves_the_high_lane_capacity() -> TestResult {
    use orchbee_capability_node_protocol::{WorktreeJobAction, WorktreeJobRequest};
    let (handle, _owner, _shutdown, _server, _worker) = test_carrier()?;
    let request = WorktreeJobRequest {
        operation_id: OperationId::from_bytes([1; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        action: WorktreeJobAction::start_status(b"C:/w".to_vec())?,
    };
    let mut stale = request.clone();
    stale.worker_generation = WorkerGeneration::new(nonzero(GENERATION.get() + 1)?);
    assert!(matches!(
        handle.admit_worktree_job(stale),
        Err(WorkerRequestError::WrongGeneration)
    ));
    let mut wrong = request.clone();
    wrong.worker_id = CapabilityWorkerId::from_bytes([9; 16])?;
    assert!(matches!(
        handle.admit_worktree_job(wrong),
        Err(WorkerRequestError::WrongWorker)
    ));
    let mut retained = Vec::new();
    for n in 1..=8 {
        retained.push(handle.admit_worktree_job(WorktreeJobRequest {
            operation_id: OperationId::from_bytes([n; 16])?,
            ..request.clone()
        })?);
    }
    assert!(matches!(
        handle.admit_worktree_job(request),
        Err(WorkerRequestError::Backpressure)
    ));
    Ok(())
}

#[test]
fn physical_title_batch_uses_low_lane_and_carries_large_complete_values_on_windows_pipe()
-> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalTitleObservation, TerminalTitleRow, TerminalTitlesRequest, TerminalTitlesResult,
    };
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (sender, receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, sender);
    let request = TerminalTitlesRequest {
        request_id: OperationId::from_bytes([99; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        identities: vec![host_identity(1)?, host_identity(2)?],
    };
    let result = TerminalTitlesResult {
        request_id: request.request_id,
        rows: request
            .identities
            .iter()
            .map(|id| TerminalTitleRow {
                identity: *id,
                observation: TerminalTitleObservation::Observed {
                    revision: NonZeroU64::new(u64::MAX).unwrap(),
                    title: Some("🙂".repeat(16384)),
                },
            })
            .collect(),
    };
    assert_eq!(
        WorkerOperationRequest::Titles(request.clone()).lane(),
        RequestLane::Low
    );
    let expected_request = request.clone();
    let response = result.clone();
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::TerminalTitlesRequest(expected_request)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalTitlesResult(response),
        )?;
        Ok(())
    });
    assert_eq!(
        handle.request(WorkerOperationRequest::Titles(request))?,
        WorkerOperationResult::Titles(result)
    );
    worker_thread.join().map_err(|_| "fake Worker panicked")??;
    assert_eq!(
        receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::PeerClosed
    );
    owner_thread
        .join()
        .map_err(|_| "title wire owner panicked")?;
    Ok(())
}

#[test]
fn physical_title_reply_cannot_change_identity_order_or_operation() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalTitleObservation, TerminalTitleRow, TerminalTitlesRequest, TerminalTitlesResult,
    };
    let request = TerminalTitlesRequest {
        request_id: OperationId::from_bytes([99; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        identities: vec![host_identity(1)?, host_identity(2)?],
    };
    let exact = TerminalTitlesResult {
        request_id: request.request_id,
        rows: request
            .identities
            .iter()
            .map(|id| TerminalTitleRow {
                identity: *id,
                observation: TerminalTitleObservation::Unavailable,
            })
            .collect(),
    };
    assert!(
        super::validate_response(
            &WorkerOperationRequest::Titles(request.clone()),
            CapabilityNodeMessage::TerminalTitlesResult(exact.clone())
        )
        .is_ok()
    );
    for bad in 0..3 {
        let mut response = exact.clone();
        match bad {
            0 => response.rows.reverse(),
            1 => response.request_id = OperationId::from_bytes([98; 16])?,
            _ => response.rows[0].identity = host_identity(3)?,
        }
        assert!(
            super::validate_response(
                &WorkerOperationRequest::Titles(request.clone()),
                CapabilityNodeMessage::TerminalTitlesResult(response)
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn shutdown_freezes_both_lanes_and_preserves_the_in_flight_request() -> TestResult {
    use orchbee_capability_node_protocol::{
        ShutdownBudget, WorkerShutdownOutcome, WorkerShutdownRequest, WorkerShutdownResult,
    };
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let ordinary = WorkerOperationRequest::Launch(launch_request(authorization(
        1,
        1,
        GENERATION.get(),
        LOCAL_WORKER_ID,
    )?)?);
    let first = handle.admit(ordinary.clone(), false)?;
    let in_flight = owner.queue.dispatch_next().ok_or("in flight")?;
    let queued = handle.admit(ordinary.clone(), false)?;
    let request = WorkerShutdownRequest {
        operation_id: OperationId::from_bytes([8; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        budget: ShutdownBudget::new(15_000)?,
        terminals: orchbee_capability_node_protocol::ShutdownTerminals::Dispose,
    };
    let stopping = handle.admit(WorkerOperationRequest::Shutdown(request), false)?;
    assert!(matches!(
        queued.recv(),
        Err(WorkerRequestError::Unavailable)
    ));
    assert!(matches!(
        handle.admit(ordinary.clone(), false),
        Err(WorkerRequestError::Unavailable)
    ));
    assert!(matches!(
        handle.admit(ordinary, true),
        Err(WorkerRequestError::Unavailable)
    ));
    let (low, _rx) = pending_for_test(WorkerOperationRequest::AgentDrain(
        orchbee_capability_node_protocol::AgentDrainRequest {
            worker_id: request.worker_id,
            worker_generation: request.worker_generation,
            token: NonZeroU64::MIN,
            acknowledged: 0,
        },
    ))?;
    assert_eq!(
        owner.queue.try_admit(RequestLane::Low, low),
        AdmitOutcome::Closed
    );
    assert!(send_completion(
        in_flight,
        Err(WorkerRequestError::NotDelivered(
            WorkerTransportFailure::Closed
        ))
    ));
    assert!(matches!(
        first.recv(),
        Err(WorkerRequestError::NotDelivered(_))
    ));
    let pending = owner.queue.dispatch_next().ok_or("shutdown pending")?;
    assert_eq!(pending.request, WorkerOperationRequest::Shutdown(request));
    let result = WorkerShutdownResult {
        request,
        outcome: WorkerShutdownOutcome::NoHost,
    };
    assert_eq!(
        super::validate_response(
            &pending.request,
            CapabilityNodeMessage::WorkerShutdownResult(result)
        ),
        Ok(WorkerOperationResult::Shutdown(result))
    );
    let mut wrong = result;
    wrong.request.budget = ShutdownBudget::new(14_999)?;
    assert_eq!(
        super::validate_response(
            &pending.request,
            CapabilityNodeMessage::WorkerShutdownResult(wrong)
        ),
        Err(WorkerResponseFailure::MismatchedAuthorization)
    );
    assert!(send_completion(
        pending,
        Ok(WorkerOperationResult::Shutdown(result))
    ));
    assert_eq!(stopping.recv()?, WorkerOperationResult::Shutdown(result));
    assert!(owner.queue.dispatch_next().is_none());
    Ok(())
}

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
fn carries_an_exact_cwd_validation_on_the_existing_serialized_wire() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let request = cwd_validation_request(1, GENERATION.get(), LOCAL_WORKER_ID, "/tmp/workspace")?;
    let expected = TerminalCwdValidationResult {
        request: request.clone(),
        outcome: TerminalCwdValidationOutcome::DirectoryAvailable,
    };
    let worker_request = request.clone();
    let worker_result = expected.clone();
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::TerminalCwdValidationRequest(worker_request)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalCwdValidationResult(worker_result),
        )?;
        Ok(())
    });

    assert_eq!(handle.validate_terminal_cwd(request)?, expected);
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
fn cwd_validation_result_requires_the_complete_request_echo() -> TestResult {
    let request = cwd_validation_request(1, GENERATION.get(), LOCAL_WORKER_ID, "/tmp/workspace")?;
    let other_worker = WorkerId::from_bytes([8; 16]);
    let cases = [
        (
            TerminalCwdValidationRequest {
                operation_id: OperationId::from_bytes([9; 16])?,
                ..request.clone()
            },
            WorkerResponseFailure::MismatchedCorrelation,
        ),
        (
            TerminalCwdValidationRequest {
                worker_id: CapabilityWorkerId::from_bytes(other_worker.as_bytes())?,
                ..request.clone()
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
        (
            TerminalCwdValidationRequest {
                worker_generation: WorkerGeneration::new(nonzero(GENERATION.get() + 1)?),
                ..request.clone()
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
        (
            TerminalCwdValidationRequest {
                cwd: AbsoluteCwd::new("/tmp/other")?,
                ..request.clone()
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
    ];
    for (actual, expected_failure) in cases {
        assert_post_delivery_failure(
            WorkerOperationRequest::CwdValidation(request.clone()),
            CapabilityNodeMessage::TerminalCwdValidationResult(TerminalCwdValidationResult {
                request: actual,
                outcome: TerminalCwdValidationOutcome::DirectoryAvailable,
            }),
            expected_failure,
        )?;
    }
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
    let queued_result = enqueue_request(&handle, queued_request)?;
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
    let receiver = enqueue_request(&handle, request)?;
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
        completion: CompletionDelivery::for_request(&request, completion),
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
    let pending = take_dispatched(&owner)?;
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
fn bounds_one_wire_request_eight_queued_requests_and_rejects_more_work() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let first = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let ninth = launch_request(authorization(11, 11, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let first_result = launch_result(&first);
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
        for _queued in 0..8 {
            let CapabilityNodeMessage::AuthorizedTerminalLaunch(request) =
                read_message(&mut worker)?
            else {
                return Err("expected a queued Launch request".into());
            };
            write_message(
                &mut worker,
                &CapabilityNodeMessage::TerminalLaunchResult(launch_result(&request)),
            )?;
        }
        Ok(())
    });

    let first_handle = handle.clone();
    let first_request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let first_caller =
        thread::spawn(move || first_handle.request(WorkerOperationRequest::Launch(first_request)));
    first_seen_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;

    let mut queued_results = Vec::new();
    for operation in 2_u8..10 {
        queued_results.push(enqueue_request(
            &handle,
            WorkerOperationRequest::Launch(launch_request(authorization(
                operation,
                u64::from(operation),
                GENERATION.get(),
                LOCAL_WORKER_ID,
            )?)?),
        )?);
    }
    assert_eq!(
        handle.request(WorkerOperationRequest::Launch(ninth)),
        Err(WorkerRequestError::Backpressure)
    );

    release_sender.send(())?;
    assert_eq!(
        first_caller
            .join()
            .map_err(|_panic| "requester panicked")??,
        WorkerOperationResult::Launch(first_result)
    );
    for queued_result in queued_results {
        assert!(matches!(
            queued_result.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
            Ok(WorkerOperationResult::Launch(_))
        ));
    }
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
fn ready_hints_route_while_idle_and_awaiting_a_response_and_complete_nothing() -> TestResult {
    use crate::viewer_ready::{ConnectionViewerReady, ViewerReadyRoutes};
    use orchbee_capability_node_protocol::TerminalViewerRecordReady;
    use orchbee_workbench_protocol::MultiplexViewerId;
    let routes = Arc::new(ViewerReadyRoutes::default());
    let connection = Arc::new(ConnectionViewerReady::default());
    let attachment = OperationId::from_bytes([9; 16])?;
    let _route = routes.route(attachment, Arc::clone(&connection));
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, _exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(
        owner.with_viewer_ready(Arc::clone(&routes)),
        server,
        exit_sender,
    );
    let hint = |value: u8| -> TestResult<CapabilityNodeMessage> {
        Ok(CapabilityNodeMessage::TerminalViewerRecordReady(
            TerminalViewerRecordReady {
                attachment_id: OperationId::from_bytes([value; 16])?,
            },
        ))
    };
    let viewer = MultiplexViewerId::new(u128::from_be_bytes([9; 16])).ok_or("viewer")?;
    let take_routed = |connection: &ConnectionViewerReady| -> TestResult {
        let deadline = Instant::now() + TEST_COMPLETION_TIMEOUT;
        loop {
            let taken = connection.take();
            if !taken.is_empty() {
                assert_eq!(taken, [viewer]);
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("the hint was not routed".into());
            }
            thread::yield_now();
        }
    };

    write_message(&mut worker, &hint(9)?)?;
    take_routed(&connection)?;

    // The attachment with no route is dropped; neither hint takes the place of the response.
    let request = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let result = launch_result(&request);
    let known = hint(9)?;
    let unknown = hint(10)?;
    let worker_thread = thread::spawn(move || -> TestResult<UnixStream> {
        let _request = read_message(&mut worker)?;
        write_message(&mut worker, &unknown)?;
        write_message(&mut worker, &known)?;
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(result),
        )?;
        Ok(worker)
    });
    assert!(
        handle
            .request(WorkerOperationRequest::Launch(request))
            .is_ok()
    );
    take_routed(&connection)?;
    let worker = worker_thread.join().map_err(|_panic| "worker panicked")??;
    assert!(handle.is_active());
    drop(worker);
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

/// `pending` promises a byte. A drain that cleared it before reading could consume the byte of a
/// signal that landed between the two, leaving `pending` set with nothing to read: every later
/// signal then skipped its byte, and the owner slept through queued work.
#[test]
fn a_signal_racing_a_drain_still_ends_a_later_wait() -> TestResult {
    use super::{OwnerWake, poll_timeout};
    use orchbee_local_ipc::{PollFd, PollFlags, poll};
    use std::sync::atomic::AtomicU64;
    const SIGNALS: u64 = 200_000;
    let wake = Arc::new(OwnerWake::new()?);
    let published = Arc::new(AtomicU64::new(0));
    let signaler = {
        let wake = Arc::clone(&wake);
        let published = Arc::clone(&published);
        thread::spawn(move || {
            for _ in 0..SIGNALS {
                published.fetch_add(1, Ordering::Release);
                wake.signal();
            }
        })
    };
    let timeout = poll_timeout(TEST_COMPLETION_TIMEOUT)?;
    let mut seen = 0;
    while seen < SIGNALS {
        let mut descriptors = [PollFd::new(wake.reader.as_local_io(), PollFlags::POLLIN)];
        let ready = poll(&mut descriptors, timeout)?;
        wake.drain();
        // Read after the drain, the way the owner inspects its queue after waiting.
        let observed = published.load(Ordering::Acquire);
        assert!(
            ready > 0 || observed > seen,
            "published work behind a wait no signal could end: {seen} of {SIGNALS}"
        );
        seen = observed;
    }
    signaler.join().map_err(|_panic| "signaler panicked")?;
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
        let WorkerAdmission::Accepted { generation } = registry.admit(LOCAL_WORKER_ID, None) else {
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
    match handle.queue.try_admit(
        request.lane(),
        PendingRequest {
            completion: CompletionDelivery::for_request(&request, completion),
            complete_write: Arc::new(AtomicBool::new(false)),
            encoded: orchbee_capability_node_protocol::encode_message(&request.message())?,
            request,
        },
    ) {
        AdmitOutcome::Admitted => Ok(result),
        _ => Err(io::Error::other("test Worker request was not queued").into()),
    }
}

/// Wait until the queue dispatches one request under the lane rules, like the
/// wire owner popping work between exchanges.
fn take_dispatched(owner: &WorkerWireOwner) -> TestResult<PendingRequest> {
    let deadline = Instant::now() + TEST_COMPLETION_TIMEOUT;
    loop {
        if let Some(pending) = owner.queue.dispatch_next() {
            return Ok(pending);
        }
        assert!(Instant::now() < deadline, "queued request never dispatched");
        thread::sleep(Duration::from_millis(1));
    }
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
        registry.admit(LOCAL_WORKER_ID, None),
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
    } = registry.admit(LOCAL_WORKER_ID, None)
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
    fn as_local_io(&self) -> BorrowedFd<'_> {
        self.stream.as_local_io()
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
    fn as_local_io(&self) -> BorrowedFd<'_> {
        self.stream.as_local_io()
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
    fn as_local_io(&self) -> BorrowedFd<'_> {
        self.stream.as_local_io()
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

fn cwd_validation_request(
    operation: u8,
    generation: u64,
    worker_id: WorkerId,
    cwd: &str,
) -> TestResult<TerminalCwdValidationRequest> {
    Ok(TerminalCwdValidationRequest {
        operation_id: OperationId::from_bytes([operation; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(worker_id.as_bytes())?,
        worker_generation: WorkerGeneration::new(nonzero(generation)?),
        cwd: AbsoluteCwd::new(cwd)?,
    })
}

fn launch_request(
    authorization: TerminalOperationAuthorization,
) -> TestResult<AuthorizedTerminalLaunch> {
    Ok(AuthorizedTerminalLaunch::new(
        authorization,
        None,
        TerminalLaunchMode::Deferred,
        None,
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
fn queued_detach_waits_fifo_when_the_high_lane_is_full() -> TestResult {
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
    // Fill the high lane so the cleanup Detach waits as a deposited FIFO waiter
    // instead of occupying a lane slot directly.
    let mut queued_results = Vec::new();
    for operation in 10_u8..18 {
        queued_results.push(enqueue_request(
            &handle,
            WorkerOperationRequest::Launch(launch_request(authorization(
                operation,
                u64::from(operation),
                GENERATION.get(),
                LOCAL_WORKER_ID,
            )?)?),
        )?);
    }
    let cleanup = TerminalViewerRequest {
        request_id: OperationId::from_bytes([3; 16])?,
        action: TerminalViewerAction::Detach,
        ..request
    };
    let cleanup_handle = handle.clone();
    let cleanup_thread = thread::spawn(move || cleanup_handle.enqueue_detach(cleanup));
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    for _queued in 0..8 {
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_message(&mut worker)?
        else {
            return Err("expected a queued Launch request".into());
        };
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(launch_result(&launch)),
        )?;
    }
    for queued_result in queued_results {
        assert!(matches!(
            queued_result.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
            Ok(WorkerOperationResult::Launch(_))
        ));
    }
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

#[test]
fn paste_results_validate_count_boundaries_and_exact_result_class() -> TestResult {
    use orchbee_capability_node_protocol::{
        ControlRejection, PasteAcceptedBytes, PasteContent, TerminalControlAction,
        TerminalControlOutcome, TerminalControlRequest, TerminalControlResult,
    };
    let content = PasteContent::new(b"payload".to_vec())?;
    let paste_request = |request_id: u8| -> TestResult<TerminalControlRequest> {
        Ok(TerminalControlRequest {
            request_id: OperationId::from_bytes([request_id; 16])?,
            worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(GENERATION),
            viewer_id: OperationId::from_bytes([2; 16])?,
            controller_id: OperationId::from_bytes([3; 16])?,
            action: TerminalControlAction::Paste {
                sequence: NonZeroU64::MIN,
                content: content.clone(),
            },
        })
    };
    let result_for = |request: &TerminalControlRequest,
                      outcome: TerminalControlOutcome|
     -> TerminalControlResult {
        TerminalControlResult {
            request_id: request.request_id,
            viewer_id: request.viewer_id,
            controller_id: request.controller_id,
            outcome,
        }
    };

    // A positive count, the exact 2 * content + 12 upper bound, and a
    // definite Rejected outcome are all accepted on one serialized wire.
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let first_request = paste_request(1)?;
    let second_request = paste_request(4)?;
    let third_request = paste_request(5)?;
    let first_result = result_for(
        &first_request,
        TerminalControlOutcome::Pasted {
            accepted_bytes: PasteAcceptedBytes::new(7)?,
        },
    );
    let bounded_result = result_for(
        &second_request,
        TerminalControlOutcome::Pasted {
            accepted_bytes: PasteAcceptedBytes::new(26)?,
        },
    );
    let rejected_result = result_for(
        &third_request,
        TerminalControlOutcome::Rejected(ControlRejection::HostRejected),
    );
    let worker_requests = (
        first_request.clone(),
        second_request.clone(),
        third_request.clone(),
    );
    let worker_results = (first_result, bounded_result, rejected_result);
    let worker_thread = thread::spawn(move || -> TestResult {
        for (request, result) in [
            (worker_requests.0, worker_results.0),
            (worker_requests.1, worker_results.1),
            (worker_requests.2, worker_results.2),
        ] {
            assert_eq!(
                read_message(&mut worker)?,
                CapabilityNodeMessage::TerminalControlRequest(request)
            );
            write_message(
                &mut worker,
                &CapabilityNodeMessage::TerminalControlResult(result),
            )?;
        }
        Ok(())
    });
    assert_eq!(
        handle.request(WorkerOperationRequest::Control(first_request))?,
        WorkerOperationResult::Control(first_result)
    );
    assert_eq!(
        handle.request(WorkerOperationRequest::Control(second_request))?,
        WorkerOperationResult::Control(bounded_result)
    );
    assert_eq!(
        handle.request(WorkerOperationRequest::Control(third_request))?,
        WorkerOperationResult::Control(rejected_result)
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

    // One byte beyond the 2 * content + 12 bound is not a paste result.
    let request = paste_request(6)?;
    for accepted_bytes in [27u32, PasteAcceptedBytes::MAXIMUM] {
        assert_post_delivery_failure(
            WorkerOperationRequest::Control(request.clone()),
            CapabilityNodeMessage::TerminalControlResult(result_for(
                &request,
                TerminalControlOutcome::Pasted {
                    accepted_bytes: PasteAcceptedBytes::new(accepted_bytes)?,
                },
            )),
            WorkerResponseFailure::WrongResultKind,
        )?;
    }

    // A different success class is never a paste success.
    for outcome in [
        TerminalControlOutcome::InputAccepted,
        TerminalControlOutcome::Resized(TerminalPixelGeometry::new(80, 24, 8, 16)?),
        TerminalControlOutcome::Acquired,
        TerminalControlOutcome::Released,
    ] {
        assert_post_delivery_failure(
            WorkerOperationRequest::Control(request.clone()),
            CapabilityNodeMessage::TerminalControlResult(result_for(&request, outcome)),
            WorkerResponseFailure::WrongResultKind,
        )?;
    }

    // Correlation and target checks still precede result-class validation.
    let wrong = OperationId::from_bytes([9; 16])?;
    for (result, failure) in [
        (
            TerminalControlResult {
                request_id: wrong,
                ..result_for(
                    &request,
                    TerminalControlOutcome::Pasted {
                        accepted_bytes: PasteAcceptedBytes::new(7)?,
                    },
                )
            },
            WorkerResponseFailure::MismatchedCorrelation,
        ),
        (
            TerminalControlResult {
                viewer_id: wrong,
                ..result_for(
                    &request,
                    TerminalControlOutcome::Pasted {
                        accepted_bytes: PasteAcceptedBytes::new(7)?,
                    },
                )
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
        (
            TerminalControlResult {
                controller_id: wrong,
                ..result_for(
                    &request,
                    TerminalControlOutcome::Pasted {
                        accepted_bytes: PasteAcceptedBytes::new(7)?,
                    },
                )
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
    ] {
        assert_post_delivery_failure(
            WorkerOperationRequest::Control(request.clone()),
            CapabilityNodeMessage::TerminalControlResult(result),
            failure,
        )?;
    }

    // A request-direction frame or a different operation's result retires
    // the carrier.
    assert_post_delivery_failure(
        WorkerOperationRequest::Control(request.clone()),
        CapabilityNodeMessage::TerminalControlRequest(paste_request(8)?),
        WorkerResponseFailure::WrongDirection,
    )?;
    let unrelated_launch = launch_request(authorization(9, 9, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Control(request),
        CapabilityNodeMessage::TerminalLaunchResult(launch_result(&unrelated_launch)),
        WorkerResponseFailure::WrongResultKind,
    )?;
    Ok(())
}

fn one_text(value: &str, enter: bool) -> TestResult<SendRecords> {
    Ok(SendRecords::new(vec![SendRecord::Text(
        SendTextInput::new(SendText::new(value.to_owned())?, enter)?,
    )])?)
}

fn one_key(key: SendKey) -> TestResult<SendRecords> {
    Ok(SendRecords::new(vec![SendRecord::Stroke(
        orchbee_capability_node_protocol::SendKeyStroke::new(
            orchbee_capability_node_protocol::SendStrokeKey::Named(key),
            0,
        )?,
    )])?)
}

fn send_request(
    operation: u8,
    attachment: u8,
    generation: u64,
    worker_id: WorkerId,
    records: SendRecords,
) -> TestResult<TerminalSendRequest> {
    Ok(TerminalSendRequest {
        request_id: OperationId::from_bytes([operation; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(worker_id.as_bytes())?,
        worker_generation: WorkerGeneration::new(nonzero(generation)?),
        target: host_identity(7)?,
        attachment_id: OperationId::from_bytes([attachment; 16])?,
        records,
        precondition: orchbee_capability_node_protocol::SendPrecondition::None,
    })
}

#[test]
fn carries_send_requests_and_results_on_the_serialized_wire() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let text_request = send_request(
        10,
        11,
        GENERATION.get(),
        LOCAL_WORKER_ID,
        one_text("echo hi", true)?,
    )?;
    let key_request = send_request(
        12,
        13,
        GENERATION.get(),
        LOCAL_WORKER_ID,
        SendRecords::new(vec![SendRecord::Stroke(
            orchbee_capability_node_protocol::SendKeyStroke::new(
                orchbee_capability_node_protocol::SendStrokeKey::Character('c'),
                1,
            )?,
        )])?,
    )?;
    let text_result = TerminalSendResult::new(
        text_request.request_id,
        TerminalSendOutcome::Accepted {
            progress: SendProgress::complete(1, 8),
            cleanup: SendCleanupStatus::Confirmed,
        },
    )?;
    let key_result = TerminalSendResult::new(
        key_request.request_id,
        TerminalSendOutcome::Rejected {
            progress: SendProgress::none(),
            reason: SendRejection::HostInputBackpressure,
            cleanup: SendCleanupStatus::Unconfirmed,
        },
    )?;
    let (worker_text, worker_key) = (text_request.clone(), key_request.clone());
    let (reply_text, reply_key) = (text_result, key_result);
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::TerminalSendRequest(worker_text)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalSendResult(reply_text),
        )?;
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::TerminalSendRequest(worker_key)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalSendResult(reply_key),
        )?;
        Ok(())
    });

    assert_eq!(
        handle.request(WorkerOperationRequest::Send(text_request))?,
        WorkerOperationResult::Send(text_result)
    );
    assert_eq!(
        handle.request(WorkerOperationRequest::Send(key_request))?,
        WorkerOperationResult::Send(key_result)
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
fn send_result_correlation_mismatch_is_post_delivery_unknown() -> TestResult {
    let request = send_request(
        10,
        11,
        GENERATION.get(),
        LOCAL_WORKER_ID,
        one_key(SendKey::Enter)?,
    )?;
    let mismatched = TerminalSendResult::new(
        OperationId::from_bytes([9; 16])?,
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
    )?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Send(request),
        CapabilityNodeMessage::TerminalSendResult(mismatched),
        WorkerResponseFailure::MismatchedCorrelation,
    )
}

#[test]
fn send_wrong_result_kind_and_direction_retire_the_carrier() -> TestResult {
    let request = send_request(
        10,
        11,
        GENERATION.get(),
        LOCAL_WORKER_ID,
        one_key(SendKey::Escape)?,
    )?;
    let unrelated_launch = launch_request(authorization(9, 9, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Send(request.clone()),
        CapabilityNodeMessage::TerminalLaunchResult(launch_result(&unrelated_launch)),
        WorkerResponseFailure::WrongResultKind,
    )?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Send(request.clone()),
        CapabilityNodeMessage::TerminalSendRequest(send_request(
            20,
            21,
            GENERATION.get(),
            LOCAL_WORKER_ID,
            one_key(SendKey::Tab)?,
        )?),
        WorkerResponseFailure::WrongDirection,
    )?;
    Ok(())
}

#[test]
fn send_response_loss_after_delivery_is_outcome_unknown() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let request = send_request(
        10,
        11,
        GENERATION.get(),
        LOCAL_WORKER_ID,
        one_text("printf", false)?,
    )?;
    let worker_thread = thread::spawn(move || -> TestResult {
        let _request = read_message(&mut worker)?;
        drop(worker);
        Ok(())
    });
    assert!(matches!(
        handle.request(WorkerOperationRequest::Send(request)),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_))
    ));
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert!(matches!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::PeerClosed | WorkerWireExit::RequestFailed
    ));
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn send_on_a_retired_carrier_is_unavailable_not_reused() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let request = send_request(
        10,
        11,
        GENERATION.get(),
        LOCAL_WORKER_ID,
        one_key(SendKey::Home)?,
    )?;
    let mismatched = TerminalSendResult::new(
        OperationId::from_bytes([9; 16])?,
        TerminalSendOutcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: SendCleanupStatus::Unconfirmed,
        },
    )?;
    let worker_thread = thread::spawn(move || -> TestResult {
        let _request = read_message(&mut worker)?;
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalSendResult(mismatched),
        )?;
        Ok(())
    });
    assert_eq!(
        handle.request(WorkerOperationRequest::Send(request.clone())),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::MismatchedCorrelation
        ))
    );
    worker_thread
        .join()
        .map_err(|_panic| "fake Worker panicked")??;
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::RequestFailed
    );
    // The retired carrier never serves another send on the same stream.
    assert_eq!(
        handle.request(WorkerOperationRequest::Send(request)),
        Err(WorkerRequestError::Unavailable)
    );
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

fn viewer_request(
    request_id: u8,
) -> TestResult<orchbee_capability_node_protocol::TerminalViewerRequest> {
    use orchbee_capability_node_protocol::{TerminalViewerAction, TerminalViewerRequest};
    Ok(TerminalViewerRequest {
        request_id: OperationId::from_bytes([request_id; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        attachment_id: OperationId::from_bytes([2; 16])?,
        action: TerminalViewerAction::Poll,
    })
}

fn launch_operation(operation: u8) -> TestResult<WorkerOperationRequest> {
    Ok(WorkerOperationRequest::Launch(launch_request(
        authorization(
            operation,
            u64::from(operation),
            GENERATION.get(),
            LOCAL_WORKER_ID,
        )?,
    )?))
}

fn request_key(request: &WorkerOperationRequest) -> TestResult<OperationId> {
    match request {
        WorkerOperationRequest::Launch(request) => Ok(request.authorization().operation_id()),
        WorkerOperationRequest::Viewer(request) => Ok(request.request_id),
        WorkerOperationRequest::AgentDrain(request) => {
            Ok(OperationId::from_bytes([request.token.get() as u8; 16])?)
        }
        _ => unreachable!("test requests are Launch, Viewer or AgentDrain"),
    }
}

fn pending_for_test(
    request: WorkerOperationRequest,
) -> TestResult<(
    PendingRequest,
    Receiver<Result<WorkerOperationResult, WorkerRequestError>>,
)> {
    let (completion, result) = mpsc::sync_channel(1);
    Ok((
        PendingRequest {
            completion: CompletionDelivery::for_request(&request, completion),
            complete_write: Arc::new(AtomicBool::new(false)),
            encoded: orchbee_capability_node_protocol::encode_message(&request.message())?,
            request,
        },
        result,
    ))
}

fn waiter_count(owner: &WorkerWireOwner) -> usize {
    owner.queue.lock().high_waiters.len()
}

#[test]
fn each_lane_admits_eight_requests_then_applies_typed_backpressure() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let mut retained = Vec::new();
    for operation in 1_u8..=8 {
        retained.push(enqueue_request(&handle, launch_operation(operation)?)?);
    }
    // A full high lane does not consume the independent low lane.
    retained.push(enqueue_request(
        &handle,
        WorkerOperationRequest::Viewer(viewer_request(1)?),
    )?);
    for request_id in 2_u8..=8 {
        retained.push(enqueue_request(
            &handle,
            WorkerOperationRequest::Viewer(viewer_request(request_id)?),
        )?);
    }
    // With both lanes at capacity the handle surfaces the existing typed busy.
    assert_eq!(
        handle.request(launch_operation(20)?),
        Err(WorkerRequestError::Backpressure)
    );
    assert_eq!(
        handle.request(WorkerOperationRequest::Viewer(viewer_request(20)?)),
        Err(WorkerRequestError::Backpressure)
    );
    // Closing drains every queued payload exactly once with Unavailable.
    let drained = owner.queue.close_and_drain();
    assert_eq!(drained.len(), 16);
    for pending in drained {
        let _delivered = super::send_completion(pending, Err(WorkerRequestError::Unavailable));
    }
    for result in retained {
        assert_eq!(
            result.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
            Err(WorkerRequestError::Unavailable)
        );
    }
    Ok(())
}

#[test]
fn dispatch_is_fifo_and_rotates_four_high_then_one_low_while_both_lanes_wait() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let mut retained = Vec::new();
    for operation in 1_u8..=8 {
        retained.push(enqueue_request(&handle, launch_operation(operation)?)?);
    }
    for request_id in 1_u8..=8 {
        retained.push(enqueue_request(
            &handle,
            WorkerOperationRequest::Viewer(viewer_request(request_id)?),
        )?);
    }
    let mut sequence = Vec::new();
    while let Some(pending) = owner.queue.dispatch_next() {
        sequence.push(request_key(&pending.request)?);
        drop(pending);
    }
    // High dispatches at most four consecutively while low is non-empty, then
    // low must be served once; each lane keeps strict FIFO order.
    let expected: Vec<OperationId> = [1_u8, 2, 3, 4, 1, 5, 6, 7, 8, 2, 3, 4, 5, 6, 7, 8]
        .iter()
        .map(|value| OperationId::from_bytes([*value; 16]))
        .collect::<Result<_, _>>()?;
    assert_eq!(sequence, expected);
    assert!(owner.queue.dispatch_next().is_none());
    drop(retained);
    Ok(())
}

#[test]
fn blocked_cleanup_promotes_into_freed_slots_before_new_try_admissions() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let mut retained = Vec::new();
    for operation in 1_u8..=8 {
        retained.push(enqueue_request(&handle, launch_operation(operation)?)?);
    }
    // A cleanup producer deposits its payload and waits once the lane is full.
    let (waiter_request, waiter_result) = pending_for_test(launch_operation(30)?)?;
    let waiter_queue = Arc::clone(&handle.queue);
    let waiter_thread = thread::spawn(move || waiter_queue.block_admit_high(waiter_request));
    let deadline = Instant::now() + TEST_COMPLETION_TIMEOUT;
    while waiter_count(&owner) == 0 {
        assert!(Instant::now() < deadline, "cleanup waiter never deposited");
        thread::sleep(Duration::from_millis(1));
    }
    // While a waiter exists a try-admission cannot enter the lane even though
    // a dispatch is about to free a slot.
    let (pending, _result) = pending_for_test(launch_operation(40)?)?;
    assert_eq!(
        handle.queue.try_admit(RequestLane::High, pending),
        AdmitOutcome::Full
    );
    // Each dispatch promotes the oldest waiter into the freed tail slot; the
    // waiter joins the FIFO behind the seven remaining queued requests.
    let first = owner
        .queue
        .dispatch_next()
        .ok_or("expected a queued request")?;
    drop(first);
    assert!(waiter_count(&owner) == 0, "waiter was not promoted");
    assert!(waiter_thread.join().map_err(|_| "waiter panicked")?);
    let mut sequence = Vec::new();
    while let Some(pending) = owner.queue.dispatch_next() {
        sequence.push(request_key(&pending.request)?);
        drop(pending);
    }
    let expected: Vec<OperationId> = [2_u8, 3, 4, 5, 6, 7, 8, 30]
        .iter()
        .map(|value| OperationId::from_bytes([*value; 16]))
        .collect::<Result<_, _>>()?;
    assert_eq!(sequence, expected);
    drop(waiter_result);
    drop(retained);
    Ok(())
}

#[test]
fn closed_queue_settles_every_queued_and_waiting_request_once() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let mut results = Vec::new();
    for operation in 1_u8..=8 {
        results.push(enqueue_request(&handle, launch_operation(operation)?)?);
    }
    for request_id in 1_u8..=8 {
        results.push(enqueue_request(
            &handle,
            WorkerOperationRequest::Viewer(viewer_request(request_id)?),
        )?);
    }
    let (waiter_request, waiter_result) = pending_for_test(launch_operation(30)?)?;
    let waiter_queue = Arc::clone(&handle.queue);
    let waiter_thread = thread::spawn(move || waiter_queue.block_admit_high(waiter_request));
    let deadline = Instant::now() + TEST_COMPLETION_TIMEOUT;
    while waiter_count(&owner) == 0 {
        assert!(Instant::now() < deadline, "cleanup waiter never deposited");
        thread::sleep(Duration::from_millis(1));
    }

    let drained = owner.queue.close_and_drain();
    assert_eq!(drained.len(), 17);
    for pending in drained {
        let _delivered = super::send_completion(pending, Err(WorkerRequestError::Unavailable));
    }
    // Every producer settles once with Unavailable; the blocked producer wakes
    // refused instead of being admitted into a dead queue.
    for result in results {
        assert_eq!(
            result.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
            Err(WorkerRequestError::Unavailable)
        );
        assert!(result.recv().is_err(), "request settled more than once");
    }
    assert!(!waiter_thread.join().map_err(|_| "waiter panicked")?);
    assert_eq!(
        waiter_result.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        Err(WorkerRequestError::Unavailable)
    );
    // A second drain settles nothing further and admission stays closed.
    assert!(owner.queue.close_and_drain().is_empty());
    assert_eq!(
        handle.queue.try_admit(
            RequestLane::Low,
            pending_for_test(WorkerOperationRequest::Viewer(viewer_request(40)?))?.0
        ),
        AdmitOutcome::Closed
    );
    assert!(
        !handle
            .queue
            .block_admit_high(pending_for_test(launch_operation(50)?)?.0)
    );
    Ok(())
}

#[test]
fn dropping_the_last_handle_closes_the_queue_like_all_senders_gone() -> TestResult {
    let (handle, owner, _shutdown, server, _worker) = test_carrier()?;
    let mut dispatched = None;
    assert!(matches!(
        owner
            .queue
            .take_next(Duration::from_millis(5), &mut dispatched),
        super::TakeNext::Idle
    ));
    drop(handle);
    assert!(matches!(
        owner
            .queue
            .take_next(Duration::from_millis(5), &mut dispatched),
        super::TakeNext::Closed
    ));
    assert_eq!(owner.run(server), WorkerWireExit::Shutdown);
    Ok(())
}

// ---- AgentDrain LOW-lane exchange (typed CNP kinds 17/18) ----

fn drain_request(
    token: u64,
    acknowledged: u64,
) -> TestResult<orchbee_capability_node_protocol::AgentDrainRequest> {
    Ok(orchbee_capability_node_protocol::AgentDrainRequest {
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        token: nonzero(token)?,
        acknowledged,
    })
}

fn drain_reply(
    token_echo: u64,
    acknowledged_echo: u64,
    queue_tail: u64,
    entries: Vec<orchbee_capability_node_protocol::AgentDrainEntry>,
) -> TestResult<orchbee_capability_node_protocol::AgentDrainReply> {
    Ok(orchbee_capability_node_protocol::AgentDrainReply::new(
        nonzero(token_echo)?,
        acknowledged_echo,
        queue_tail,
        entries,
    )?)
}

fn drain_page() -> TestResult<Vec<orchbee_capability_node_protocol::AgentDrainEntry>> {
    Ok(vec![
        orchbee_capability_node_protocol::AgentDrainEntry::Event {
            sequence: nonzero(1)?,
            identity: host_identity(1)?,
            payload: "drain-test-payload".to_owned(),
        },
    ])
}

#[test]
fn queued_abandoned_drain_is_discarded_and_required_work_continues() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let drain = drain_request(1, 0)?;
    let exchange = handle.admit_agent_drain(drain)?;
    // The receiver is dropped while the request still sits in the LOW lane:
    // the owner is not running yet, so abandonment is guaranteed to precede
    // dispatch and reply delivery.
    drop(exchange);
    // A Required request queued beside it must still be served after the
    // abandoned drain's reply is consumed and discarded.
    let required_result = enqueue_request(&handle, launch_operation(2)?)?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let launch = launch_request(authorization(2, 2, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let expected = launch_result(&launch);
    let worker_thread = thread::spawn(move || -> TestResult {
        // The occupied HIGH lane dispatches before the LOW-lane drain.
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(launch)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(expected),
        )?;
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AgentDrainRequest(drain)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::AgentDrainReply(drain_reply(1, 0, 0, Vec::new())?),
        )?;
        Ok(())
    });

    assert_eq!(
        required_result.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        Ok(WorkerOperationResult::Launch(expected))
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
fn in_flight_abandoned_drain_is_discarded_and_required_work_continues() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let drain = drain_request(1, 0)?;
    let launch = launch_request(authorization(2, 2, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let expected = launch_result(&launch);
    let (seen_sender, seen_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AgentDrainRequest(drain)
        );
        seen_sender.send(())?;
        release_receiver.recv()?;
        write_message(
            &mut worker,
            &CapabilityNodeMessage::AgentDrainReply(drain_reply(1, 0, 0, Vec::new())?),
        )?;
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(launch)
        );
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(expected),
        )?;
        Ok(())
    });

    let exchange = handle.admit_agent_drain(drain_request(1, 0)?)?;
    // The request is in flight on the wire; the deterministic barrier drops
    // the receiver before the reply is ever written.
    seen_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;
    drop(exchange);
    release_sender.send(())?;
    assert_eq!(
        handle.request(launch_operation(2)?)?,
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
fn in_flight_abandoned_required_exchange_retires_the_carrier() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let launch = launch_request(authorization(1, 1, GENERATION.get(), LOCAL_WORKER_ID)?)?;
    let expected = launch_result(&launch);
    let (seen_sender, seen_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(launch)
        );
        seen_sender.send(())?;
        release_receiver.recv()?;
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalLaunchResult(expected),
        )?;
        Ok(())
    });

    let exchange = handle.admit(launch_operation(1)?, false)?;
    seen_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?;
    drop(exchange);
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
fn drain_echo_mismatches_retire_the_carrier() -> TestResult {
    for (request, reply) in [
        // token_echo must equal the request token exactly.
        (drain_request(1, 0)?, drain_reply(2, 0, 0, Vec::new())?),
        // acknowledged_echo must equal the request acknowledgment exactly.
        (drain_request(1, 0)?, drain_reply(1, 7, 0, Vec::new())?),
    ] {
        let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
        let (exit_sender, exit_receiver) = mpsc::channel();
        let owner_thread = spawn_owner(owner, server, exit_sender);
        let worker_thread = thread::spawn(move || -> TestResult {
            assert_eq!(
                read_message(&mut worker)?,
                CapabilityNodeMessage::AgentDrainRequest(request)
            );
            write_message(&mut worker, &CapabilityNodeMessage::AgentDrainReply(reply))?;
            Ok(())
        });
        let exchange = handle.admit_agent_drain(request)?;
        assert_eq!(
            exchange.recv(),
            Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
                WorkerResponseFailure::MismatchedCorrelation
            ))
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
    }
    Ok(())
}

#[test]
fn drain_wrong_result_kind_direction_and_malformed_frame_retire_the_carrier() -> TestResult {
    // A well-formed frame of another result kind is rejected like every
    // existing request kind.
    let drain = drain_request(1, 0)?;
    let wrong_kind = CapabilityNodeMessage::TerminalLaunchResult(launch_result(&launch_request(
        authorization(9, 9, GENERATION.get(), LOCAL_WORKER_ID)?,
    )?));
    assert_drain_delivery_failure(drain, wrong_kind, |failure| {
        matches!(
            failure,
            WorkerRequestError::PostDeliveryOutcomeUnknown(WorkerResponseFailure::WrongResultKind)
        )
    })?;

    // A request-direction message is rejected like every existing kind.
    let drain = drain_request(1, 0)?;
    let wrong_direction = CapabilityNodeMessage::AgentDrainRequest(drain_request(1, 0)?);
    assert_drain_delivery_failure(drain, wrong_direction, |failure| {
        matches!(
            failure,
            WorkerRequestError::PostDeliveryOutcomeUnknown(WorkerResponseFailure::WrongDirection)
        )
    })?;

    // A malformed frame keeps the existing fatal wire behavior.
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let drain = drain_request(1, 0)?;
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AgentDrainRequest(drain)
        );
        worker.write_all(&[0xff, 0xff, 0xff, 0xff])?;
        worker.flush()?;
        Ok(())
    });
    let exchange = handle.admit_agent_drain(drain_request(1, 0)?)?;
    assert!(matches!(
        exchange.recv(),
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::InvalidFrame(_) | WorkerResponseFailure::Disconnected
        ))
    ));
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

fn assert_drain_delivery_failure(
    drain: orchbee_capability_node_protocol::AgentDrainRequest,
    response: CapabilityNodeMessage,
    expect: impl Fn(&WorkerRequestError) -> bool,
) -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AgentDrainRequest(drain)
        );
        write_message(&mut worker, &response)?;
        Ok(())
    });
    let exchange = handle.admit_agent_drain(drain)?;
    match exchange.recv() {
        Err(failure) if expect(&failure) => {}
        other => return Err(format!("unexpected drain delivery outcome: {other:?}").into()),
    }
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
fn drain_admission_rejects_stale_routes_before_the_wire() -> TestResult {
    let (handle, _owner, _shutdown, _server, _worker) = test_carrier()?;
    let mut wrong_worker = drain_request(1, 0)?;
    wrong_worker.worker_id = CapabilityWorkerId::from_bytes([9; 16])?;
    assert!(matches!(
        handle.admit_agent_drain(wrong_worker),
        Err(WorkerRequestError::WrongWorker)
    ));
    let mut wrong_generation = drain_request(1, 0)?;
    wrong_generation.worker_generation = WorkerGeneration::new(nonzero(GENERATION.get() + 1)?);
    assert!(matches!(
        handle.admit_agent_drain(wrong_generation),
        Err(WorkerRequestError::WrongGeneration)
    ));
    Ok(())
}

#[test]
fn elapsed_drain_wait_keeps_the_attempt_for_a_later_reply() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let exchange = handle.admit_agent_drain(drain_request(1, 0)?)?;
    // No owner is running: the bounded wait elapses without abandoning the
    // admitted attempt.
    assert!(matches!(
        exchange.recv_deadline(Instant::now() + Duration::from_millis(20)),
        ExchangeWait::Elapsed
    ));

    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    let drain = drain_request(1, 0)?;
    let reply = drain_reply(1, 0, 1, drain_page()?)?;
    let expected = reply.clone();
    let worker_thread = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::AgentDrainRequest(drain)
        );
        write_message(&mut worker, &CapabilityNodeMessage::AgentDrainReply(reply))?;
        Ok(())
    });
    match exchange.recv_deadline(Instant::now() + TEST_COMPLETION_TIMEOUT) {
        ExchangeWait::Ready(result) => match *result {
            Ok(WorkerOperationResult::AgentDrain(actual)) => assert_eq!(actual, expected),
            other => {
                return Err(format!("unexpected drain wait outcome: {other:?}").into());
            }
        },
        other => return Err(format!("unexpected drain wait outcome: {other:?}").into()),
    }
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
fn lost_drain_exchange_maps_complete_write_like_the_blocking_wait() -> TestResult {
    for complete in [false, true] {
        let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
        let exchange = handle.admit_agent_drain(drain_request(1, 0)?)?;
        let queued = owner
            .queue
            .dispatch_next()
            .ok_or("expected a queued drain request")?;
        queued.complete_write.store(complete, Ordering::Release);
        drop(queued);
        match exchange.recv_deadline(Instant::now() + TEST_COMPLETION_TIMEOUT) {
            ExchangeWait::Lost(WorkerRequestError::Unavailable) if !complete => {}
            ExchangeWait::Lost(WorkerRequestError::PostDeliveryOutcomeUnknown(
                WorkerResponseFailure::CarrierStopped,
            )) if complete => {}
            other => {
                return Err(format!(
                    "unexpected drain wait outcome for complete={complete}: {other:?}"
                )
                .into());
            }
        }
    }
    Ok(())
}

#[test]
fn buffered_drain_result_is_ready_at_an_elapsed_deadline() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let exchange = handle.admit_agent_drain(drain_request(1, 0)?)?;
    // Populate the exchange's completion channel synchronously through the
    // REAL delivery path: dispatch the admitted drain off the LOW lane and
    // send a typed AgentDrain result via the same send_completion the owner
    // uses for every settled request. No wire, no thread, no timing — the
    // channel is provably populated before recv_deadline is called. A peer
    // write observed on a done channel cannot prove this state: the owner
    // may not have decoded or delivered the reply yet.
    let queued = owner
        .queue
        .dispatch_next()
        .ok_or("expected a queued drain request")?;
    let reply = drain_reply(1, 0, 0, Vec::new())?;
    assert!(send_completion(
        queued,
        Ok(WorkerOperationResult::AgentDrain(reply.clone()))
    ));
    // The deadline is already in the past. A populated channel must still
    // surface Ready with the typed payload — recv_deadline only reports
    // Elapsed when the channel is EMPTY at the deadline, and never discards
    // a buffered result.
    let elapsed_deadline = Instant::now() - Duration::from_millis(1);
    match exchange.recv_deadline(elapsed_deadline) {
        ExchangeWait::Ready(result) => match *result {
            Ok(WorkerOperationResult::AgentDrain(actual)) => assert_eq!(actual, reply),
            other => {
                return Err(format!("buffered drain result was not Ready: {other:?}").into());
            }
        },
        other => {
            return Err(format!("buffered drain result was not Ready: {other:?}").into());
        }
    }
    Ok(())
}

#[test]
fn drain_exchanges_fill_the_low_lane_and_rotate_after_four_highs() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let mut retained = Vec::new();
    for operation in 1_u8..=8 {
        retained.push(enqueue_request(&handle, launch_operation(operation)?)?);
    }
    let mut drains = Vec::new();
    for token in 1_u64..=8 {
        drains.push(handle.admit_agent_drain(drain_request(token, 0)?)?);
    }
    // The ninth drain meets the existing LOW-lane bound with typed
    // backpressure; try_admit never blocks.
    assert!(matches!(
        handle.admit_agent_drain(drain_request(9, 0)?),
        Err(WorkerRequestError::Backpressure)
    ));
    // Drains share the LOW lane: after every four-high burst one drain
    // dispatches; each lane keeps strict FIFO order.
    let mut sequence = Vec::new();
    while let Some(pending) = owner.queue.dispatch_next() {
        sequence.push(request_key(&pending.request)?);
        drop(pending);
    }
    let expected: Vec<OperationId> = [1_u8, 2, 3, 4, 1, 5, 6, 7, 8, 2, 3, 4, 5, 6, 7, 8]
        .iter()
        .map(|value| OperationId::from_bytes([*value; 16]))
        .collect::<Result<_, _>>()?;
    assert_eq!(sequence, expected);
    drop(drains);
    drop(retained);
    Ok(())
}

#[test]
fn transient_clear_carrier_refuses_wrong_operation_and_request_viewer_attachment_echo() -> TestResult
{
    use orchbee_capability_node_protocol::{
        TerminalControlAction as Action, TerminalControlOutcome as Outcome, TerminalControlRequest,
        TerminalControlResult,
    };
    let request = TerminalControlRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        viewer_id: OperationId::from_bytes([2; 16])?,
        controller_id: OperationId::from_bytes([3; 16])?,
        action: Action::Clear,
    };
    let base = TerminalControlResult {
        request_id: request.request_id,
        viewer_id: request.viewer_id,
        controller_id: request.controller_id,
        outcome: Outcome::Cleared {
            generation: NonZeroU64::MIN,
            revision: NonZeroU64::MIN,
            cleanup_confirmed: true,
        },
    };
    let wrong = OperationId::from_bytes([9; 16])?;
    for (response, failure) in [
        (
            TerminalControlResult {
                outcome: Outcome::InputAccepted,
                ..base
            },
            WorkerResponseFailure::WrongResultKind,
        ),
        (
            TerminalControlResult {
                request_id: wrong,
                ..base
            },
            WorkerResponseFailure::MismatchedCorrelation,
        ),
        (
            TerminalControlResult {
                viewer_id: wrong,
                ..base
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
        (
            TerminalControlResult {
                controller_id: wrong,
                ..base
            },
            WorkerResponseFailure::MismatchedTarget,
        ),
    ] {
        assert_post_delivery_failure(
            WorkerOperationRequest::Control(request.clone()),
            CapabilityNodeMessage::TerminalControlResult(response),
            failure,
        )?;
    }
    let mut input = request;
    input.action = Action::Input {
        sequence: NonZeroU64::MIN,
        bytes: orchbee_capability_node_protocol::ControlInput::new(vec![b'x'])?,
    };
    for outcome in [base.outcome, Outcome::ClearOutcomeUnknown] {
        assert_post_delivery_failure(
            WorkerOperationRequest::Control(input.clone()),
            CapabilityNodeMessage::TerminalControlResult(TerminalControlResult { outcome, ..base }),
            WorkerResponseFailure::WrongResultKind,
        )?;
    }
    Ok(())
}

/// A refused acknowledgement retires the Worker carrier, so every Agent read
/// loses its Worker after the first Host-color report.
#[test]
fn host_colors_acknowledgement_keeps_the_worker_carrier() -> TestResult {
    use orchbee_capability_node_protocol::{
        HostDefaultColors, TerminalControlAction as Action, TerminalControlOutcome as Outcome,
        TerminalControlRequest, TerminalControlResult,
    };
    let request = TerminalControlRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        viewer_id: OperationId::from_bytes([2; 16])?,
        controller_id: OperationId::from_bytes([3; 16])?,
        action: Action::HostColors {
            sequence: NonZeroU64::MIN,
            colors: HostDefaultColors::new(0x00d0_d0d0, 0x0010_1010, None, [None; 16])?,
        },
    };
    let accepted = TerminalControlResult {
        request_id: request.request_id,
        viewer_id: request.viewer_id,
        controller_id: request.controller_id,
        outcome: Outcome::HostColorsAccepted,
    };
    assert!(matches!(
        super::validate_response(
            &WorkerOperationRequest::Control(request.clone()),
            CapabilityNodeMessage::TerminalControlResult(accepted),
        ),
        Ok(WorkerOperationResult::Control(TerminalControlResult {
            outcome: Outcome::HostColorsAccepted,
            ..
        }))
    ));
    assert_post_delivery_failure(
        WorkerOperationRequest::Control(request),
        CapabilityNodeMessage::TerminalControlResult(TerminalControlResult {
            outcome: Outcome::InputAccepted,
            ..accepted
        }),
        WorkerResponseFailure::WrongResultKind,
    )
}
