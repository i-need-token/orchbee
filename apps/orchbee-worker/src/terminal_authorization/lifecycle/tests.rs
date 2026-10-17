use std::num::NonZeroU64;

use super::*;
use crate::terminal_authorization::{
    AuthorizedHostOperationResult, HostOperationAdmission, LaunchHostCompletion,
};
use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostCreationId, HostEpoch, HostTerminalId,
    HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds, LeaseId, LifecycleRejection,
    LifecycleStepOrdinal, LifecycleUnknownReason, OperationId, TERMINAL_LAUNCH_CAPABILITY_VERSION,
    TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalFence, TerminalId, TerminalLaunchMode,
    TerminalLaunchOutcome, TerminalLaunchRejection, TerminalLifecycleAction as Action,
    TerminalLifecycleAuthorization, TerminalLifecycleOutcome as Outcome, TerminalLifecycleRequest,
    TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
    TerminalStartOutcome, TerminalStartRejection, WorkerGeneration, WorkerId,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn idle_inspection_never_claims_close_owner_or_advances_mutation_receipts() -> TestResult {
    let mut kernel = kernel()?;
    let mut inspect = request(Action::InspectInventoryPage { cursor: None }, 1, 1, 30)?;
    inspect.close_operation_id = OperationId::from_bytes([99; 16])?;
    let page = Outcome::InventoryPage(orchbee_capability_node_protocol::LifecycleInventoryPage {
        inventory_generation: nz(1),
        next_cursor: None,
        returned_count: 0,
        target: None,
    });
    assert_eq!(complete(&mut kernel, inspect, page)?.outcome(), page);
    assert!(
        kernel
            .bindings
            .get(&inspect.authorization.terminal_id)
            .unwrap()
            .lifecycle
            .is_none()
    );
    // A fresh confirmed Close uses its own owner/first ordinal after inspection.
    let close = request(Action::Terminate, 1, 1, 31)?;
    assert_eq!(
        complete(&mut kernel, close, Outcome::TerminateAccepted)?.outcome(),
        Outcome::TerminateAccepted
    );
    let receipts = kernel
        .bindings
        .get(&close.authorization.terminal_id)
        .unwrap()
        .lifecycle;
    inspect.request_id = OperationId::from_bytes([32; 16])?;
    complete(&mut kernel, inspect, page)?;
    let after = kernel
        .bindings
        .get(&close.authorization.terminal_id)
        .unwrap()
        .lifecycle
        .unwrap();
    assert_eq!(after.owner, receipts.unwrap().owner);
    assert_eq!(after.high_ordinal, receipts.unwrap().high_ordinal);
    assert_eq!(after.last_mutation, receipts.unwrap().last_mutation);
    Ok(())
}

#[test]
fn phase_and_action_mismatched_completions_cannot_replace_the_receipt() -> TestResult {
    let mut kernel = kernel()?;
    let request = request(Action::Dispose, 1, 1, 30)?;
    let dispatch = admit(&mut kernel, request)?;
    assert!(
        kernel
            .record_lifecycle_completion(dispatch, Outcome::Disposed)
            .is_err()
    );
    kernel.begin_lifecycle_handoff(dispatch, 300, request.target.host_epoch())?;
    for outcome in [
        Outcome::HostUnavailableBeforeHandoff,
        Outcome::Rejected(LifecycleRejection::LeaseExpired),
        Outcome::ForceAccepted,
    ] {
        assert!(
            kernel
                .record_lifecycle_completion(dispatch, outcome)
                .is_err()
        );
        assert!(kernel.take_lifecycle_result().is_none());
    }
    kernel.record_lifecycle_completion(dispatch, Outcome::Disposed)?;
    assert!(
        kernel
            .record_lifecycle_completion(dispatch, Outcome::Disposed)
            .is_err()
    );
    assert_eq!(
        kernel
            .take_lifecycle_result()
            .expect("exact Dispose")
            .outcome(),
        Outcome::Disposed
    );
    Ok(())
}

fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("nonzero fixture")
}
fn request(
    action: Action,
    attempt: u64,
    ordinal: u8,
    id: u8,
) -> TestResult<TerminalLifecycleRequest> {
    Ok(TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: TerminalId::from_bytes([1; 16])?,
            worker_id: WorkerId::from_bytes([3; 16])?,
            worker_generation: WorkerGeneration::new(nz(7)),
            binding_fence: TerminalFence::new(nz(1)),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes([8; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(nz(500)),
            ),
        },
        close_operation_id: OperationId::from_bytes([20; 16])?,
        request_id: OperationId::from_bytes([id; 16])?,
        attempt_generation: nz(attempt),
        step_ordinal: LifecycleStepOrdinal::new(ordinal)?,
        target: HostTerminalIdentity::new(
            HostCreationId::from_bytes([2; 16])?,
            HostEpoch::from_bytes([4; 16])?,
            HostTerminalId::from_bytes([5; 16])?,
        ),
        action,
    })
}
fn original() -> TestResult<AuthorizedTerminalStart> {
    let request = request(Action::Dispose, 1, 1, 30)?;
    Ok(AuthorizedTerminalStart::new(
        TerminalOperationAuthorization::new(
            request.authorization.terminal_id,
            OperationId::from_bytes([9; 16])?,
            request.authorization.worker_id,
            request.authorization.worker_generation,
            request.authorization.binding_fence,
            TerminalOperationLease::new(
                LeaseId::from_bytes([9; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(nz(200)),
            ),
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        ),
        request.target,
        TerminalPixelGeometry::new(80, 24, 8, 16)?,
    ))
}
fn kernel() -> TestResult<AuthorizedHostOperationKernel> {
    let mut kernel = AuthorizedHostOperationKernel::new(
        orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
        nz(7),
    )?;
    let start = original()?;
    let HostOperationAdmission::Dispatch(dispatch) =
        kernel.admit_start(start, 100, start.identity().host_epoch())?
    else {
        panic!("start dispatch");
    };
    kernel.record_pre_handoff_failure(&dispatch, 100)?;
    assert!(kernel.take_result().is_some());
    Ok(kernel)
}
fn admit(
    kernel: &mut AuthorizedHostOperationKernel,
    request: TerminalLifecycleRequest,
) -> TestResult<LifecycleDispatch> {
    let LifecycleAdmission::Dispatch(dispatch) =
        kernel.admit_lifecycle(request, 300, request.target.host_epoch())?
    else {
        panic!("lifecycle dispatch");
    };
    Ok(dispatch)
}
fn complete(
    kernel: &mut AuthorizedHostOperationKernel,
    request: TerminalLifecycleRequest,
    outcome: Outcome,
) -> TestResult<orchbee_capability_node_protocol::TerminalLifecycleResult> {
    let dispatch = admit(kernel, request)?;
    assert_eq!(
        kernel.begin_lifecycle_handoff(dispatch, 300, request.target.host_epoch())?,
        HostHandoffAdmission::HandedOff
    );
    kernel.record_lifecycle_completion(dispatch, outcome)?;
    Ok(kernel.take_lifecycle_result().expect("completed result"))
}
fn rejected(
    kernel: &mut AuthorizedHostOperationKernel,
    request: TerminalLifecycleRequest,
    reason: LifecycleRejection,
) -> TestResult {
    assert_eq!(
        kernel.admit_lifecycle(request, 300, request.target.host_epoch())?,
        LifecycleAdmission::ResultReady
    );
    assert_eq!(
        kernel.take_lifecycle_result().expect("rejection").outcome(),
        Outcome::Rejected(reason)
    );
    Ok(())
}

#[test]
fn fresh_close_lease_replays_after_expiry_and_preserves_original_start() -> TestResult {
    let mut kernel = kernel()?;
    let request = request(Action::Terminate, 1, 1, 30)?;
    let receipt = complete(&mut kernel, request, Outcome::TerminateAccepted)?;
    assert_eq!(
        kernel.admit_lifecycle(request, u64::MAX, request.target.host_epoch())?,
        LifecycleAdmission::ResultReady
    );
    assert_eq!(kernel.take_lifecycle_result(), Some(receipt));
    let original = original()?;
    assert_eq!(
        kernel.admit_start(original, u64::MAX, original.identity().host_epoch())?,
        HostOperationAdmission::ResultReady
    );
    assert!(
        matches!(kernel.take_result(), Some(AuthorizedHostOperationResult::Start(result)) if matches!(result.outcome(), TerminalStartOutcome::Failed(_)))
    );
    Ok(())
}

#[test]
fn old_mutation_replays_after_multiple_queries_and_conflicting_ids_never_dispatch() -> TestResult {
    let mut kernel = kernel()?;
    let mutation = request(Action::Terminate, 1, 1, 30)?;
    let receipt = complete(&mut kernel, mutation, Outcome::TerminateAccepted)?;
    for (ordinal, id) in [(2, 31), (3, 32)] {
        complete(
            &mut kernel,
            request(Action::InventoryPage { cursor: None }, 1, ordinal, id)?,
            Outcome::OutcomeUnknown(LifecycleUnknownReason::Deadline),
        )?;
    }
    assert_eq!(
        kernel.admit_lifecycle(mutation, 300, mutation.target.host_epoch())?,
        LifecycleAdmission::ResultReady
    );
    assert_eq!(kernel.take_lifecycle_result(), Some(receipt));
    for (attempt, ordinal) in [(1, 4), (2, 1)] {
        let changed = request(Action::Force, attempt, ordinal, 30)?;
        rejected(&mut kernel, changed, LifecycleRejection::LifecycleConflict)?;
    }
    rejected(
        &mut kernel,
        request(Action::InventoryPage { cursor: None }, 1, 2, 31)?,
        LifecycleRejection::LifecycleConflict,
    )?;
    Ok(())
}

#[test]
fn unknown_fences_every_later_mutation_even_after_query_and_higher_attempt() -> TestResult {
    let mut kernel = kernel()?;
    let mutation = request(Action::Force, 1, 1, 30)?;
    let unknown = complete(
        &mut kernel,
        mutation,
        Outcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
    )?;
    complete(
        &mut kernel,
        request(Action::InventoryPage { cursor: None }, 1, 2, 31)?,
        Outcome::InventoryFailure(
            orchbee_capability_node_protocol::LifecycleInventoryFailure::StaleCursor,
        ),
    )?;
    for action in [Action::Terminate, Action::Force, Action::Dispose] {
        for (attempt, ordinal) in [(1, 3), (2, 1), (u64::MAX, 1)] {
            rejected(
                &mut kernel,
                request(action, attempt, ordinal, 40)?,
                LifecycleRejection::LifecycleConflict,
            )?;
        }
    }
    assert_eq!(
        kernel.admit_lifecycle(mutation, 300, mutation.target.host_epoch())?,
        LifecycleAdmission::ResultReady
    );
    assert_eq!(kernel.take_lifecycle_result(), Some(unknown));
    Ok(())
}

#[test]
fn known_disposal_survives_query_failure_and_blocks_new_effects() -> TestResult {
    let mut kernel = kernel()?;
    let dispose = request(Action::Dispose, 1, 1, 30)?;
    let receipt = complete(&mut kernel, dispose, Outcome::Disposed)?;
    complete(
        &mut kernel,
        request(Action::InventoryPage { cursor: None }, 2, 1, 31)?,
        Outcome::OutcomeUnknown(LifecycleUnknownReason::InvalidResponse),
    )?;
    rejected(
        &mut kernel,
        request(Action::Dispose, 2, 2, 32)?,
        LifecycleRejection::LifecycleConflict,
    )?;
    assert_eq!(
        kernel.admit_lifecycle(dispose, 300, dispose.target.host_epoch())?,
        LifecycleAdmission::ResultReady
    );
    assert_eq!(kernel.take_lifecycle_result(), Some(receipt));
    Ok(())
}

#[test]
fn changed_owner_authority_identity_and_capability_are_effect_free() -> TestResult {
    let base = request(Action::Terminate, 1, 1, 30)?;
    let mut cases = Vec::new();
    let mut changed = base;
    changed.authorization.worker_id = WorkerId::from_bytes([99; 16])?;
    cases.push((changed, LifecycleRejection::WrongWorker));
    changed = base;
    changed.authorization.worker_generation = WorkerGeneration::new(nz(8));
    cases.push((changed, LifecycleRejection::StaleWorkerGeneration));
    changed = base;
    changed.authorization.binding_fence = TerminalFence::new(nz(2));
    cases.push((changed, LifecycleRejection::BindingFenceMismatch));
    changed = base;
    changed.authorization.terminal_id = TerminalId::from_bytes([99; 16])?;
    cases.push((changed, LifecycleRejection::BindingUnavailable));
    changed = base;
    changed.target = HostTerminalIdentity::new(
        HostCreationId::from_bytes([99; 16])?,
        base.target.host_epoch(),
        base.target.terminal_id(),
    );
    cases.push((changed, LifecycleRejection::HostIdentityMismatch));
    changed = base;
    changed.authorization.capability_version =
        orchbee_capability_node_protocol::TerminalLifecycleCapabilityVersion::new(
            std::num::NonZeroU16::new(3).expect("three"),
        );
    cases.push((changed, LifecycleRejection::UnsupportedCapability));
    for (candidate, reason) in cases {
        rejected(&mut kernel()?, candidate, reason)?;
    }
    let mut kernel = kernel()?;
    complete(&mut kernel, base, Outcome::TerminateAccepted)?;
    let mut other = request(Action::Force, 2, 1, 31)?;
    other.close_operation_id = OperationId::from_bytes([99; 16])?;
    rejected(&mut kernel, other, LifecycleRejection::LifecycleConflict)?;
    Ok(())
}

#[test]
fn handoff_revalidates_lease_with_monotone_time_and_single_global_slot() -> TestResult {
    let mut kernel = kernel()?;
    let request = request(Action::Dispose, 1, 1, 30)?;
    let dispatch = admit(&mut kernel, request)?;
    assert_eq!(
        kernel.admit_lifecycle(request, 500, request.target.host_epoch())?,
        LifecycleAdmission::SlotOccupied
    );
    assert_eq!(
        kernel.admit_start(original()?, 100, request.target.host_epoch())?,
        HostOperationAdmission::SlotOccupied
    );
    assert_eq!(
        kernel.begin_lifecycle_handoff(dispatch, 1, request.target.host_epoch())?,
        HostHandoffAdmission::ResultReady
    );
    assert_eq!(
        kernel.take_lifecycle_result().expect("expired").outcome(),
        Outcome::Rejected(LifecycleRejection::LeaseExpired)
    );
    assert!(
        kernel
            .begin_lifecycle_handoff(dispatch, 300, request.target.host_epoch())
            .is_err()
    );
    Ok(())
}

#[test]
fn prehandoff_unavailable_is_known_cached_and_allows_authorized_later_attempt() -> TestResult {
    let mut kernel = kernel()?;
    let request = request(Action::Dispose, 1, 1, 30)?;
    let dispatch = admit(&mut kernel, request)?;
    kernel.lifecycle_pre_handoff_failure(dispatch, 300)?;
    assert_eq!(
        kernel
            .take_lifecycle_result()
            .expect("unavailable")
            .outcome(),
        Outcome::HostUnavailableBeforeHandoff
    );
    complete(
        &mut kernel,
        self::request(Action::Dispose, 2, 1, 31)?,
        Outcome::Disposed,
    )?;
    Ok(())
}

#[test]
fn ordinal_and_attempt_exhaustion_keep_maximum_receipt_readable() -> TestResult {
    let mut kernel = kernel()?;
    complete(
        &mut kernel,
        request(Action::Terminate, 1, 1, 30)?,
        Outcome::TerminateAccepted,
    )?;
    for (attempt, ordinal, id) in [
        (1, 31, 31),
        (1, 32, 32),
        (u64::MAX - 1, 1, 33),
        (u64::MAX, 1, 34),
        (u64::MAX, 31, 35),
        (u64::MAX, 32, 36),
    ] {
        let query = request(Action::InventoryPage { cursor: None }, attempt, ordinal, id)?;
        let result = complete(
            &mut kernel,
            query,
            Outcome::InventoryFailure(
                orchbee_capability_node_protocol::LifecycleInventoryFailure::PageRejected,
            ),
        )?;
        assert_eq!(
            kernel.admit_lifecycle(query, 300, query.target.host_epoch())?,
            LifecycleAdmission::ResultReady
        );
        assert_eq!(kernel.take_lifecycle_result(), Some(result));
    }
    assert!(LifecycleStepOrdinal::new(33).is_err());
    assert!(u64::MAX.checked_add(1).is_none());
    rejected(
        &mut kernel,
        request(Action::Force, u64::MAX, 32, 37)?,
        LifecycleRejection::LifecycleConflict,
    )?;
    Ok(())
}

#[test]
fn close_owner_blocks_higher_fence_launch_start_but_keeps_exact_launch_replay() -> TestResult {
    let start = original()?;
    let launch = AuthorizedTerminalLaunch::new(
        start.authorization(),
        None,
        TerminalLaunchMode::Deferred,
        None,
    );
    let mut kernel = AuthorizedHostOperationKernel::new(
        orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
        nz(7),
    )?;
    let HostOperationAdmission::Dispatch(dispatch) = kernel.admit_launch(launch.clone(), 100)?
    else {
        panic!("launch");
    };
    kernel.begin_host_handoff(&dispatch, 100, start.identity().host_epoch())?;
    kernel.record_launch_completion(
        &dispatch,
        LaunchHostCompletion::Accepted {
            identity: start.identity(),
        },
    )?;
    let original_result = kernel.take_result();
    complete(
        &mut kernel,
        request(Action::Terminate, 1, 1, 30)?,
        Outcome::TerminateAccepted,
    )?;
    assert_eq!(
        kernel.admit_launch(launch, 300)?,
        HostOperationAdmission::ResultReady
    );
    assert_eq!(kernel.take_result(), original_result);
    let authorization = start.authorization();
    let higher = TerminalOperationAuthorization::new(
        authorization.terminal_id(),
        OperationId::from_bytes([80; 16])?,
        authorization.worker_id(),
        authorization.worker_generation(),
        TerminalFence::new(nz(2)),
        request(Action::Dispose, 1, 1, 30)?.authorization.lease,
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    kernel.admit_launch(
        AuthorizedTerminalLaunch::new(higher, None, TerminalLaunchMode::Deferred, None),
        300,
    )?;
    assert!(
        matches!(kernel.take_result(), Some(AuthorizedHostOperationResult::Launch(result)) if result.outcome() == TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::LifecycleActive))
    );
    kernel.admit_start(
        AuthorizedTerminalStart::new(higher, start.identity(), start.geometry()),
        300,
        start.identity().host_epoch(),
    )?;
    assert!(
        matches!(kernel.take_result(), Some(AuthorizedHostOperationResult::Start(result)) if result.outcome() == TerminalStartOutcome::Rejected(TerminalStartRejection::LifecycleActive))
    );
    Ok(())
}
