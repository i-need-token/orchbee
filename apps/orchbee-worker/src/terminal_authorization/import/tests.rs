use std::num::NonZeroU64;

use super::*;
use crate::terminal_authorization::{
    AuthorizedHostOperationResult, HostOperationAdmission, LifecycleAdmission,
};
use orchbee_capability_node_protocol::{
    AdoptCloseInFlight, AuthorizedTerminalStart, HostCreationId, HostEpoch, HostTerminalId,
    LeaseExpiresAtUnixMilliseconds, LeaseId, LifecycleRejection, LifecycleStepOrdinal, OperationId,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalFence,
    TerminalLifecycleAction as Action, TerminalLifecycleAuthorization,
    TerminalLifecycleOutcome as Outcome, TerminalLifecycleRequest, TerminalOperationAuthorization,
    TerminalOperationLease, TerminalPixelGeometry, TerminalStartOutcome, TerminalStartRejection,
    WorkerGeneration, WorkerId,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const NOW: u64 = 100;
const CLOSE: u8 = 20;

#[test]
fn an_imported_fence_refuses_every_start_at_or_below_it() -> TestResult {
    let mut kernel = kernel()?;
    let awaiting = terminal(1, false)?;
    kernel.import(
        &[binding(
            1,
            awaiting.identity,
            AdoptBindingState::AwaitingStart,
            5,
            None,
        )?],
        &[awaiting],
    )?;
    for (fence, operation) in [(4, 40), (5, 41)] {
        assert_eq!(
            kernel.admit_start(
                start(1, awaiting.identity, fence, operation)?,
                NOW,
                epoch()?
            )?,
            HostOperationAdmission::ResultReady
        );
        assert!(matches!(
            kernel.take_result(),
            Some(AuthorizedHostOperationResult::Start(result))
                if matches!(result.outcome(), TerminalStartOutcome::Rejected(_))
        ));
    }
    assert!(matches!(
        kernel.admit_start(start(1, awaiting.identity, 6, 42)?, NOW, epoch()?)?,
        HostOperationAdmission::Dispatch(_)
    ));
    Ok(())
}

#[test]
fn an_imported_close_refuses_a_resend_and_accepts_a_later_attempt() -> TestResult {
    let mut kernel = kernel()?;
    let running = terminal(1, true)?;
    let close = AdoptCloseInFlight {
        close_operation_id: OperationId::from_bytes([CLOSE; 16])?,
        attempt_generation: nz(2),
        step_ordinal: LifecycleStepOrdinal::new(3)?,
        step_effect_unknown: true,
    };
    kernel.import(
        &[binding(
            1,
            running.identity,
            AdoptBindingState::Started,
            5,
            Some(close),
        )?],
        &[running],
    )?;
    // The step the lost Worker may have run, and any step after it in that attempt, never reach
    // the Host again.
    for (attempt, ordinal, id) in [(2, 3, 30), (2, 2, 31), (2, 4, 32), (1, 5, 33)] {
        let resend = lifecycle(
            1,
            running.identity,
            5,
            Action::Terminate,
            attempt,
            ordinal,
            id,
        )?;
        assert_eq!(
            kernel.admit_lifecycle(resend, NOW, epoch()?)?,
            LifecycleAdmission::ResultReady
        );
        assert_eq!(
            kernel
                .take_lifecycle_result()
                .map(|result| result.outcome()),
            Some(Outcome::Rejected(LifecycleRejection::LifecycleConflict))
        );
    }
    let later = lifecycle(1, running.identity, 5, Action::Terminate, 3, 1, 34)?;
    assert!(matches!(
        kernel.admit_lifecycle(later, NOW, epoch()?)?,
        LifecycleAdmission::Dispatch(_)
    ));
    Ok(())
}

#[test]
fn an_unmatched_host_terminal_is_quarantined_and_never_addressed() -> TestResult {
    let mut kernel = kernel()?;
    let stray = terminal(9, true)?;
    let outcome = kernel.import(&[], &[stray])?;
    assert_eq!(outcome.quarantined, vec![stray.identity]);
    assert_eq!(outcome.imported, 0);
    assert_eq!(
        kernel.admit_start(start(9, stray.identity, 1, 40)?, NOW, epoch()?)?,
        HostOperationAdmission::ResultReady
    );
    assert!(matches!(
        kernel.take_result(),
        Some(AuthorizedHostOperationResult::Start(result))
            if matches!(result.outcome(), TerminalStartOutcome::Rejected(TerminalStartRejection::ConflictingReplay))
    ));
    for action in [
        Action::Dispose,
        Action::InspectInventoryPage { cursor: None },
    ] {
        let request = lifecycle(9, stray.identity, 1, action, 1, 1, 41)?;
        assert_eq!(
            kernel.admit_lifecycle(request, NOW, epoch()?)?,
            LifecycleAdmission::ResultReady
        );
        assert!(matches!(
            kernel
                .take_lifecycle_result()
                .map(|result| result.outcome()),
            Some(Outcome::Rejected(_))
        ));
    }
    Ok(())
}

#[test]
fn a_binding_the_inventory_lacks_or_contradicts_is_missing() -> TestResult {
    let mut kernel = kernel()?;
    let kept = terminal(1, true)?;
    let gone = terminal(2, true)?;
    let contradicted = terminal(3, false)?;
    let exited = terminal(4, true)?;
    let outcome = kernel.import(
        &[
            binding(1, kept.identity, AdoptBindingState::Started, 5, None)?,
            binding(2, gone.identity, AdoptBindingState::Started, 5, None)?,
            binding(
                3,
                contradicted.identity,
                AdoptBindingState::Started,
                5,
                None,
            )?,
            binding(4, exited.identity, AdoptBindingState::Started, 5, None)?,
        ],
        &[kept, contradicted, exited],
    )?;
    assert_eq!(outcome.imported, 2);
    assert_eq!(outcome.missing, vec![product(2)?, product(3)?]);
    assert_eq!(outcome.quarantined, vec![contradicted.identity]);
    let request = lifecycle(2, gone.identity, 5, Action::Dispose, 1, 1, 30)?;
    assert_eq!(
        kernel.admit_lifecycle(request, NOW, epoch()?)?,
        LifecycleAdmission::ResultReady
    );
    assert_eq!(
        kernel
            .take_lifecycle_result()
            .map(|result| result.outcome()),
        Some(Outcome::Rejected(LifecycleRejection::BindingUnavailable))
    );
    Ok(())
}

#[test]
fn an_import_needs_a_kernel_that_admitted_nothing() -> TestResult {
    let mut kernel = kernel()?;
    let running = terminal(1, true)?;
    let bindings = [binding(
        1,
        running.identity,
        AdoptBindingState::Started,
        5,
        None,
    )?];
    kernel.import(&bindings, &[running])?;
    let before = kernel.clone();
    assert!(kernel.import(&bindings, &[running]).is_err());
    assert_eq!(kernel, before);
    Ok(())
}

fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("nonzero fixture")
}
fn kernel() -> TestResult<AuthorizedHostOperationKernel> {
    Ok(AuthorizedHostOperationKernel::new(
        orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]),
        nz(7),
    )?)
}
fn epoch() -> TestResult<HostEpoch> {
    Ok(HostEpoch::from_bytes([4; 16])?)
}
fn product(value: u8) -> TestResult<TerminalId> {
    Ok(TerminalId::from_bytes([value; 16])?)
}
fn terminal(value: u8, started: bool) -> TestResult<InventoryTerminal> {
    Ok(InventoryTerminal {
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes([value + 100; 16])?,
            epoch()?,
            HostTerminalId::from_bytes([value + 200; 16])?,
        ),
        started,
        handover: None,
    })
}
fn binding(
    value: u8,
    identity: HostTerminalIdentity,
    state: AdoptBindingState,
    fence: u64,
    close: Option<AdoptCloseInFlight>,
) -> TestResult<AdoptBinding> {
    Ok(AdoptBinding {
        terminal_id: product(value)?,
        identity,
        state,
        fence: TerminalFence::new(nz(fence)),
        close,
    })
}
fn start(
    value: u8,
    identity: HostTerminalIdentity,
    fence: u64,
    operation: u8,
) -> TestResult<AuthorizedTerminalStart> {
    Ok(AuthorizedTerminalStart::new(
        TerminalOperationAuthorization::new(
            product(value)?,
            OperationId::from_bytes([operation; 16])?,
            WorkerId::from_bytes([3; 16])?,
            WorkerGeneration::new(nz(7)),
            TerminalFence::new(nz(fence)),
            TerminalOperationLease::new(
                LeaseId::from_bytes([operation; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(nz(500)),
            ),
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        ),
        identity,
        TerminalPixelGeometry::new(80, 24, 8, 16)?,
    ))
}
fn lifecycle(
    value: u8,
    target: HostTerminalIdentity,
    fence: u64,
    action: Action,
    attempt: u64,
    ordinal: u8,
    id: u8,
) -> TestResult<TerminalLifecycleRequest> {
    Ok(TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: product(value)?,
            worker_id: WorkerId::from_bytes([3; 16])?,
            worker_generation: WorkerGeneration::new(nz(7)),
            binding_fence: TerminalFence::new(nz(fence)),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes([8; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(nz(500)),
            ),
        },
        close_operation_id: OperationId::from_bytes([CLOSE; 16])?,
        request_id: OperationId::from_bytes([id; 16])?,
        attempt_generation: nz(attempt),
        step_ordinal: LifecycleStepOrdinal::new(ordinal)?,
        target,
        action,
    })
}
