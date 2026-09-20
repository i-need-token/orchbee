use super::*;
use orchbee_capability_node_protocol::{
    LifecycleInventoryCursor, LifecycleRejection, LifecycleStepOrdinal,
    TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalLifecycleAction, TerminalLifecycleAuthorization,
    TerminalLifecycleOutcome, TerminalLifecycleRequest, TerminalLifecycleResult,
};

fn request() -> TestResult<TerminalLifecycleRequest> {
    Ok(TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: TerminalId::from_bytes([1; 16])?,
            worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(GENERATION),
            binding_fence: TerminalFence::new(NonZeroU64::MIN),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes([2; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(nonzero(1000)?),
            ),
        },
        close_operation_id: OperationId::from_bytes([3; 16])?,
        request_id: OperationId::from_bytes([4; 16])?,
        attempt_generation: NonZeroU64::MIN,
        step_ordinal: LifecycleStepOrdinal::new(1)?,
        target: host_identity(5)?,
        action: TerminalLifecycleAction::InventoryPage { cursor: None },
    })
}

fn result(request: TerminalLifecycleRequest) -> TestResult<TerminalLifecycleResult> {
    Ok(TerminalLifecycleResult::new(
        request,
        TerminalLifecycleOutcome::Rejected(LifecycleRejection::LeaseExpired),
    )?)
}

#[test]
fn every_lifecycle_seal_field_is_checked_even_on_a_definite_refusal() -> TestResult {
    let expected = request()?;
    let mut cases = Vec::new();
    for field in 0..15 {
        let mut actual = expected;
        let failure = match field {
            0 => {
                actual.request_id = OperationId::from_bytes([9; 16])?;
                WorkerResponseFailure::MismatchedCorrelation
            }
            1 => {
                actual.close_operation_id = OperationId::from_bytes([9; 16])?;
                WorkerResponseFailure::MismatchedCorrelation
            }
            2 => {
                actual.attempt_generation = nonzero(2)?;
                WorkerResponseFailure::MismatchedCorrelation
            }
            3 => {
                actual.step_ordinal = LifecycleStepOrdinal::new(2)?;
                WorkerResponseFailure::MismatchedCorrelation
            }
            4 => {
                actual.authorization.terminal_id = TerminalId::from_bytes([9; 16])?;
                WorkerResponseFailure::MismatchedAuthorization
            }
            5 => {
                actual.authorization.worker_id = CapabilityWorkerId::from_bytes([9; 16])?;
                WorkerResponseFailure::MismatchedAuthorization
            }
            6 => {
                actual.authorization.worker_generation = WorkerGeneration::new(nonzero(2)?);
                WorkerResponseFailure::MismatchedAuthorization
            }
            7 => {
                actual.authorization.binding_fence = TerminalFence::new(nonzero(2)?);
                WorkerResponseFailure::MismatchedAuthorization
            }
            8 => {
                actual.authorization.lease = TerminalOperationLease::new(
                    LeaseId::from_bytes([9; 16])?,
                    expected.authorization.lease.expires_at(),
                );
                WorkerResponseFailure::MismatchedAuthorization
            }
            9 => {
                actual.authorization.lease = TerminalOperationLease::new(
                    expected.authorization.lease.id(),
                    LeaseExpiresAtUnixMilliseconds::new(nonzero(1001)?),
                );
                WorkerResponseFailure::MismatchedAuthorization
            }
            10 => {
                actual.authorization.capability_version =
                    orchbee_capability_node_protocol::TerminalLifecycleCapabilityVersion::new(
                        std::num::NonZeroU16::new(2).unwrap(),
                    );
                WorkerResponseFailure::MismatchedAuthorization
            }
            11 => {
                actual.target = HostTerminalIdentity::new(
                    HostCreationId::from_bytes([9; 16])?,
                    expected.target.host_epoch(),
                    expected.target.terminal_id(),
                );
                WorkerResponseFailure::MismatchedTarget
            }
            12 => {
                actual.target = HostTerminalIdentity::new(
                    expected.target.creation_id(),
                    HostEpoch::from_bytes([9; 16])?,
                    expected.target.terminal_id(),
                );
                WorkerResponseFailure::MismatchedTarget
            }
            13 => {
                actual.target = HostTerminalIdentity::new(
                    expected.target.creation_id(),
                    expected.target.host_epoch(),
                    HostTerminalId::from_bytes([9; 16])?,
                );
                WorkerResponseFailure::MismatchedTarget
            }
            14 => {
                actual.action = TerminalLifecycleAction::Dispose;
                WorkerResponseFailure::WrongResultKind
            }
            _ => unreachable!(),
        };
        cases.push((actual, failure));
    }
    for cursor in [
        Some(LifecycleInventoryCursor {
            inventory_generation: NonZeroU64::MIN,
            offset: 0,
        }),
        Some(LifecycleInventoryCursor {
            inventory_generation: nonzero(2)?,
            offset: 128,
        }),
    ] {
        cases.push((
            TerminalLifecycleRequest {
                action: TerminalLifecycleAction::InventoryPage { cursor },
                ..expected
            },
            WorkerResponseFailure::WrongResultKind,
        ));
    }
    for (actual, failure) in cases {
        assert_eq!(
            super::super::validate_response(
                &WorkerOperationRequest::Lifecycle(expected),
                CapabilityNodeMessage::TerminalLifecycleResult(result(actual)?)
            ),
            Err(failure)
        );
    }
    assert_eq!(
        super::super::validate_response(
            &WorkerOperationRequest::Lifecycle(expected),
            CapabilityNodeMessage::TerminalLifecycleResult(result(expected)?)
        ),
        Ok(WorkerOperationResult::Lifecycle(result(expected)?))
    );
    Ok(())
}

#[test]
fn lifecycle_requests_require_the_registered_worker_and_generation() -> TestResult {
    let (handle, _owner, _shutdown, _server, _worker) = test_carrier()?;
    let mut request = request()?;
    request.authorization.worker_id = CapabilityWorkerId::from_bytes([9; 16])?;
    assert_eq!(
        handle.request(WorkerOperationRequest::Lifecycle(request)),
        Err(WorkerRequestError::WrongWorker)
    );
    request.authorization.worker_id = CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?;
    request.authorization.worker_generation = WorkerGeneration::new(nonzero(2)?);
    assert_eq!(
        handle.request(WorkerOperationRequest::Lifecycle(request)),
        Err(WorkerRequestError::WrongGeneration)
    );
    Ok(())
}

#[test]
fn lifecycle_corruption_retires_the_carrier_after_complete_delivery() -> TestResult {
    let request = request()?;
    let mut wrong = request;
    wrong.request_id = OperationId::from_bytes([9; 16])?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Lifecycle(request),
        CapabilityNodeMessage::TerminalLifecycleResult(result(wrong)?),
        WorkerResponseFailure::MismatchedCorrelation,
    )?;
    assert_post_delivery_failure(
        WorkerOperationRequest::Lifecycle(request),
        CapabilityNodeMessage::TerminalLifecycleRequest(request),
        WorkerResponseFailure::WrongDirection,
    )?;
    Ok(())
}

#[test]
fn lifecycle_and_existing_message_directions_never_alias() -> TestResult {
    let request = request()?;
    let viewer = orchbee_capability_node_protocol::TerminalViewerRequest {
        request_id: request.request_id,
        worker_id: request.authorization.worker_id,
        worker_generation: request.authorization.worker_generation,
        attachment_id: request.close_operation_id,
        action: orchbee_capability_node_protocol::TerminalViewerAction::Poll,
    };
    assert_eq!(
        super::super::validate_response(
            &WorkerOperationRequest::Viewer(viewer),
            CapabilityNodeMessage::TerminalLifecycleResult(result(request)?)
        ),
        Err(WorkerResponseFailure::WrongResultKind)
    );
    assert_eq!(
        super::super::validate_response(
            &WorkerOperationRequest::Viewer(viewer),
            CapabilityNodeMessage::TerminalLifecycleRequest(request)
        ),
        Err(WorkerResponseFailure::WrongDirection)
    );
    assert_eq!(
        super::super::validate_response(
            &WorkerOperationRequest::Lifecycle(request),
            CapabilityNodeMessage::TerminalViewerRequest(viewer)
        ),
        Err(WorkerResponseFailure::WrongDirection)
    );
    assert_eq!(
        super::super::validate_response(
            &WorkerOperationRequest::Lifecycle(request),
            CapabilityNodeMessage::TerminalViewerResult(
                orchbee_capability_node_protocol::TerminalViewerResult {
                    request_id: viewer.request_id,
                    attachment_id: viewer.attachment_id,
                    outcome: orchbee_capability_node_protocol::TerminalViewerOutcome::Empty
                }
            )
        ),
        Err(WorkerResponseFailure::WrongResultKind)
    );
    Ok(())
}

#[test]
fn admission_receipt_precedes_owner_work_and_backpressure_returns_none() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let request = request()?;
    let before = std::time::Instant::now();
    let receipt = handle.submit_lifecycle(request)?;
    let after = std::time::Instant::now();
    assert!(before <= receipt.admitted_observed_at());
    assert!(receipt.admitted_observed_at() <= after);
    // The high lane admits up to its bound before backpressure applies.
    let mut held = Vec::new();
    for _queued in 0..7 {
        held.push(handle.submit_lifecycle(request)?);
    }
    assert!(matches!(
        handle.submit_lifecycle(request),
        Err(WorkerRequestError::Backpressure)
    ));
    let queued = owner
        .queue
        .dispatch_next()
        .ok_or("expected a queued lifecycle request")?;
    assert_eq!(queued.request, WorkerOperationRequest::Lifecycle(request));
    for _queued in 0..7 {
        assert!(owner.queue.dispatch_next().is_some());
    }
    assert!(owner.queue.dispatch_next().is_none());
    // Retiring the handle after admission must not erase the already returned receipt.
    handle.active.store(false, Ordering::Release);
    queued
        .completion
        .as_ref()
        .ok_or("missing completion")?
        .send(Ok(WorkerOperationResult::Lifecycle(result(request)?)))?;
    assert_eq!(receipt.wait()?, result(request)?);
    Ok(())
}

#[test]
fn lifecycle_receipt_preserves_original_errors_and_lost_completion_classification() -> TestResult {
    for complete in [false, true] {
        let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
        let receipt = handle.submit_lifecycle(request()?)?;
        let queued = owner
            .queue
            .dispatch_next()
            .ok_or("expected a queued lifecycle request")?;
        queued.complete_write.store(complete, Ordering::Release);
        drop(queued);
        assert_eq!(
            receipt.wait(),
            Err(if complete {
                WorkerRequestError::PostDeliveryOutcomeUnknown(
                    WorkerResponseFailure::CarrierStopped,
                )
            } else {
                WorkerRequestError::Unavailable
            })
        );
    }
    for error in [
        WorkerRequestError::Unavailable,
        WorkerRequestError::NotDelivered(WorkerTransportFailure::Closed),
        WorkerRequestError::PostDeliveryOutcomeUnknown(WorkerResponseFailure::TimedOut),
    ] {
        let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
        let receipt = handle.submit_lifecycle(request()?)?;
        let queued = owner
            .queue
            .dispatch_next()
            .ok_or("expected a queued lifecycle request")?;
        queued
            .completion
            .as_ref()
            .ok_or("missing completion")?
            .send(Err(error))?;
        assert_eq!(receipt.wait(), Err(error));
    }
    Ok(())
}

#[test]
fn lifecycle_admission_rejects_stale_routes_without_occupying_the_queue() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let mut request = request()?;
    request.authorization.worker_id = CapabilityWorkerId::from_bytes([9; 16])?;
    assert!(matches!(
        handle.submit_lifecycle(request),
        Err(WorkerRequestError::WrongWorker)
    ));
    request.authorization.worker_id = CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?;
    request.authorization.worker_generation = WorkerGeneration::new(nonzero(2)?);
    assert!(matches!(
        handle.submit_lifecycle(request),
        Err(WorkerRequestError::WrongGeneration)
    ));
    assert!(owner.queue.dispatch_next().is_none());
    handle.active.store(false, Ordering::Release);
    assert!(matches!(
        handle.submit_lifecycle(request),
        Err(WorkerRequestError::Unavailable)
    ));
    assert!(owner.queue.dispatch_next().is_none());
    Ok(())
}

#[test]
fn lifecycle_receipt_waits_for_the_exact_real_carrier_result() -> TestResult {
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let request = request()?;
    let receipt = handle.submit_lifecycle(request)?;
    let (exit_sender, exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(owner, server, exit_sender);
    assert_eq!(
        read_message(&mut worker)?,
        CapabilityNodeMessage::TerminalLifecycleRequest(request)
    );
    write_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalLifecycleResult(result(request)?),
    )?;
    assert_eq!(receipt.wait()?, result(request)?);
    drop(worker);
    assert_eq!(
        exit_receiver.recv_timeout(TEST_COMPLETION_TIMEOUT)?,
        WorkerWireExit::PeerClosed
    );
    owner_thread.join().map_err(|_| "owner panicked")?;
    Ok(())
}
