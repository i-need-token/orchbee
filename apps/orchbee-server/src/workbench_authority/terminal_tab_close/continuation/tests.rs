use super::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity,
    LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId, TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
    TerminalFence, TerminalId, TerminalLifecycleAuthorization, TerminalOperationLease,
    WorkerGeneration, WorkerId,
};

fn core(now: Instant, phase: Option<LifecycleTerminalPhase>) -> CloseContinuation {
    CloseContinuation::prepare(now, phase).expect("representable test deadline")
}

fn request(
    action: TerminalLifecycleAction,
    ordinal: LifecycleStepOrdinal,
    generation: NonZeroU64,
) -> TerminalLifecycleRequest {
    TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: TerminalId::from_bytes([1; 16]).expect("terminal"),
            worker_id: WorkerId::from_bytes([2; 16]).expect("worker"),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            binding_fence: TerminalFence::new(NonZeroU64::MIN),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes([3; 16]).expect("lease"),
                LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MIN),
            ),
        },
        close_operation_id: OperationId::from_bytes([4; 16]).expect("close operation"),
        request_id: OperationId::from_bytes([ordinal.get(); 16]).expect("step operation"),
        attempt_generation: generation,
        step_ordinal: ordinal,
        target: HostTerminalIdentity::new(
            HostCreationId::from_bytes([5; 16]).expect("creation"),
            HostEpoch::from_bytes([6; 16]).expect("epoch"),
            HostTerminalId::from_bytes([7; 16]).expect("Host terminal"),
        ),
        action,
    }
}

fn prepare_step(
    core: &mut CloseContinuation,
    now: Instant,
    generation: NonZeroU64,
) -> CloseStepToken {
    let CloseContinuationNext::Lifecycle {
        action,
        step_ordinal,
    } = core.next(now)
    else {
        panic!("expected lifecycle step");
    };
    core.reserve_step(now, generation, request(action, step_ordinal, generation))
        .expect("matching prepared step")
}

fn step(
    core: &mut CloseContinuation,
    now: Instant,
    generation: NonZeroU64,
    outcome: TerminalLifecycleOutcome,
) {
    let token = prepare_step(core, now, generation);
    let result =
        TerminalLifecycleResult::new(token.request(), outcome).expect("valid result fixture");
    core.confirm_admitted(&token, now, generation)
        .expect("one admission");
    core.settle_step(
        token,
        now,
        generation,
        CloseStepObservation::Result(&result),
    )
    .expect("one exact completion");
}

fn resume(core: &mut CloseContinuation, now: Instant) {
    let prepared = core.prepare_resume(now).expect("resumable known state");
    core.commit_resume(prepared);
}

fn final_page(phase: Option<LifecycleTerminalPhase>) -> TerminalLifecycleOutcome {
    TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
        inventory_generation: NonZeroU64::MIN,
        next_cursor: None,
        returned_count: u16::from(phase.is_some()),
        target: phase.map(|phase| LifecycleTargetObservation { revision: 1, phase }),
    })
}

#[test]
fn deadline_equality_closes_admission_without_spending_a_step() {
    let now = Instant::now();
    for (offset, available) in [(29_999, true), (30_000, false), (30_001, false)] {
        let mut close = core(now, Some(LifecycleTerminalPhase::AwaitingStart));
        assert_eq!(
            matches!(
                close.next(now + Duration::from_millis(offset)),
                CloseContinuationNext::Lifecycle { .. }
            ),
            available
        );
        assert_eq!(close.submitted_steps, 0);
    }
}

#[test]
fn backpressure_spends_no_step_and_yields_without_renewing_deadline() {
    let now = Instant::now();
    let mut close = core(now, None);
    let token = prepare_step(&mut close, now, NonZeroU64::MIN);
    close
        .reject_prequeue(token, now, CloseStepPrequeueFailure::Backpressure)
        .expect("prepared cancellation");
    assert_eq!(close.submitted_steps, 0);
    assert_eq!(close.last_query_at, None);
    assert_eq!(
        close.next(now),
        CloseContinuationNext::WaitUntil(now + QUERY_SPACING)
    );
    assert!(matches!(
        close.next(now + QUERY_SPACING),
        CloseContinuationNext::Lifecycle { .. }
    ));
    assert_eq!(
        close.next(now + ATTEMPT_DURATION),
        CloseContinuationNext::Parked
    );
}

#[test]
fn late_admission_and_dispose_receipt_survive_attempt_expiry() {
    let now = Instant::now();
    let mut close = core(now, Some(LifecycleTerminalPhase::AwaitingStart));
    let token = prepare_step(&mut close, now, NonZeroU64::MIN);
    let late = now + ATTEMPT_DURATION + Duration::from_secs(1);
    assert_eq!(close.next(late), CloseContinuationNext::Outstanding);
    assert!(!close.is_resumable());
    close
        .confirm_admitted(&token, late, NonZeroU64::MIN)
        .expect("late admitted receipt");
    assert_eq!(close.submitted_steps, 1);
    assert!(!close.is_resumable());
    let result = TerminalLifecycleResult::new(token.request(), TerminalLifecycleOutcome::Disposed)
        .expect("disposed");
    close
        .settle_step(
            token,
            late,
            NonZeroU64::MIN,
            CloseStepObservation::Result(&result),
        )
        .expect("late known effect");
    assert!(matches!(
        close.status(),
        WorkbenchTabCloseStatus::DisposedAwaitingPublication {
            reason: PublicationReason::TargetCleanupPending,
            resumable: true
        }
    ));
    resume(&mut close, late);
    assert_eq!(close.next(late), CloseContinuationNext::TargetCleanup);
}

#[test]
fn admitted_not_delivered_failure_is_counted_but_prequeue_unavailable_is_not() {
    let now = Instant::now();
    let mut close = core(now, None);
    let token = prepare_step(&mut close, now, NonZeroU64::MIN);
    close
        .reject_prequeue(token, now, CloseStepPrequeueFailure::Unavailable)
        .expect("prequeue failure");
    assert_eq!(close.submitted_steps, 0);
    let retry = now + QUERY_SPACING;
    let token = prepare_step(&mut close, retry, NonZeroU64::MIN);
    close
        .confirm_admitted(&token, retry, NonZeroU64::MIN)
        .expect("admitted");
    close
        .settle_step(
            token,
            retry,
            NonZeroU64::MIN,
            CloseStepObservation::NotDelivered,
        )
        .expect("known no delivery");
    assert_eq!(close.submitted_steps, 1);
}

#[test]
fn query_spacing_uses_admitted_observation_across_resume() {
    let now = Instant::now();
    let mut close = core(now, None);
    let queried = now + ATTEMPT_DURATION - Duration::from_millis(100);
    step(
        &mut close,
        queried,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::PageRejected),
    );
    assert_eq!(
        close.next(now + ATTEMPT_DURATION),
        CloseContinuationNext::Parked
    );
    resume(&mut close, now + ATTEMPT_DURATION);
    for milliseconds in [249, 250, 251] {
        let result = close.next(queried + Duration::from_millis(milliseconds));
        assert_eq!(
            matches!(result, CloseContinuationNext::Lifecycle { .. }),
            milliseconds >= 250
        );
    }
}

#[test]
fn stale_cursor_restarts_spend_all_32_steps_and_never_prove_absence() {
    let now = Instant::now();
    let mut close = core(now, None);
    for ordinal in 1..=32 {
        let at = now + QUERY_SPACING * u32::from(ordinal - 1);
        step(
            &mut close,
            at,
            NonZeroU64::MIN,
            TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::StaleCursor),
        );
        assert_eq!(close.submitted_steps, ordinal);
        if ordinal < 32 {
            assert!(!close.is_resumable());
        }
    }
    assert!(matches!(
        close.status(),
        WorkbenchTabCloseStatus::CleanupBlocked {
            reason: BlockedReason::InventoryIncomplete,
            resumable: true
        }
    ));
    assert_eq!(
        close.next(now + QUERY_SPACING * 32),
        CloseContinuationNext::Parked
    );
    assert!(!close.publication_ready());
}

#[test]
fn partial_absence_is_not_disposal_authority_but_complete_walk_still_needs_dispose() {
    let now = Instant::now();
    let mut close = core(now, None);
    let next = LifecycleInventoryCursor {
        inventory_generation: NonZeroU64::MIN,
        offset: 128,
    };
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
            inventory_generation: NonZeroU64::MIN,
            next_cursor: Some(next),
            returned_count: 128,
            target: None,
        }),
    );
    assert_eq!(
        close.next(now + QUERY_SPACING),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::InventoryPage { cursor: Some(next) },
            step_ordinal: LifecycleStepOrdinal::new(2).expect("ordinal")
        }
    );
    step(
        &mut close,
        now + QUERY_SPACING,
        NonZeroU64::MIN,
        final_page(None),
    );
    assert!(matches!(
        close.next(now + QUERY_SPACING),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::Dispose,
            ..
        }
    ));
    assert!(!close.publication_ready());
}

#[test]
fn duplicate_exact_target_across_pages_is_nonresumable_identity_conflict() {
    let now = Instant::now();
    let mut close = core(now, None);
    let observation = LifecycleTargetObservation {
        revision: 1,
        phase: LifecycleTerminalPhase::Failed,
    };
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
            inventory_generation: NonZeroU64::MIN,
            next_cursor: Some(LifecycleInventoryCursor {
                inventory_generation: NonZeroU64::MIN,
                offset: 1,
            }),
            returned_count: 1,
            target: Some(observation),
        }),
    );
    step(
        &mut close,
        now + QUERY_SPACING,
        NonZeroU64::MIN,
        final_page(Some(LifecycleTerminalPhase::Failed)),
    );
    assert!(matches!(
        close.status(),
        WorkbenchTabCloseStatus::CleanupBlocked {
            reason: BlockedReason::IdentityConflict,
            resumable: false
        }
    ));
}

#[test]
fn grace_and_force_acknowledgment_survive_resume_without_repeat_signals() {
    let now = Instant::now();
    let mut close = core(
        now,
        Some(LifecycleTerminalPhase::Running {
            terminate_requested: false,
        }),
    );
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::TerminateAccepted,
    );
    let grace = close.grace_deadline;
    assert_eq!(
        close.next(now + ATTEMPT_DURATION),
        CloseContinuationNext::Parked
    );
    let later = now + ATTEMPT_DURATION;
    resume(&mut close, later);
    assert_eq!(close.grace_deadline, grace);
    assert!(matches!(
        close.next(later),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::Force,
            ..
        }
    ));
    step(
        &mut close,
        later,
        NonZeroU64::new(2).expect("generation"),
        TerminalLifecycleOutcome::ForceAccepted,
    );
    step(
        &mut close,
        later,
        NonZeroU64::new(2).expect("generation"),
        TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::PageRejected),
    );
    assert!(close.force_acknowledged);
    assert!(matches!(
        close.next(later + QUERY_SPACING),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::InventoryPage { .. },
            ..
        }
    ));
}

#[test]
fn partial_settled_observation_suppresses_force_after_grace() {
    partial_settled_observation_retained(false);
}

#[test]
fn stale_inventory_restart_does_not_forget_settled_target() {
    partial_settled_observation_retained(true);
}

fn partial_settled_observation_retained(stale_restart: bool) {
    let now = Instant::now();
    let mut close = core(
        now,
        Some(LifecycleTerminalPhase::Running {
            terminate_requested: false,
        }),
    );
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::TerminateAccepted,
    );
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
            inventory_generation: NonZeroU64::MIN,
            next_cursor: Some(LifecycleInventoryCursor {
                inventory_generation: NonZeroU64::MIN,
                offset: 1,
            }),
            returned_count: 1,
            target: Some(LifecycleTargetObservation {
                revision: 3,
                phase: LifecycleTerminalPhase::Exited {
                    terminate_requested: true,
                },
            }),
        }),
    );
    if stale_restart {
        step(
            &mut close,
            now + QUERY_SPACING,
            NonZeroU64::MIN,
            TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::StaleCursor),
        );
    }
    assert!(matches!(
        close.next(now + TERMINATE_GRACE),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::InventoryPage { .. },
            ..
        }
    ));
    assert!(!close.publication_ready());
    step(
        &mut close,
        now + TERMINATE_GRACE,
        NonZeroU64::MIN,
        final_page(None),
    );
    assert!(matches!(
        close.next(now + TERMINATE_GRACE),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::Dispose,
            ..
        }
    ));
    assert!(!close.force_acknowledged);
}

#[test]
fn grace_queries_come_early_then_rejoin_the_query_grid() {
    let now = Instant::now();
    let running = |terminate_requested| {
        final_page(Some(LifecycleTerminalPhase::Running {
            terminate_requested,
        }))
    };
    let mut close = core(now, None);
    step(&mut close, now, NonZeroU64::MIN, running(false));
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::TerminateAccepted,
    );
    // 40, 80 and 150 ms catch a shell that exits at once; 250 ms rejoins the grid.
    for at in [40, 80, 150, 250] {
        let at = now + Duration::from_millis(at);
        assert_eq!(close.next(now), CloseContinuationNext::WaitUntil(at));
        step(&mut close, at, NonZeroU64::MIN, running(true));
    }
    assert_eq!(
        close.next(now),
        CloseContinuationNext::WaitUntil(now + QUERY_SPACING * 2)
    );
    assert_eq!(close.submitted_steps, 6);
}

#[test]
fn exact_settled_observation_ends_grace_early_without_force() {
    let now = Instant::now();
    let mut close = core(
        now,
        Some(LifecycleTerminalPhase::Running {
            terminate_requested: false,
        }),
    );
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::TerminateAccepted,
    );
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        final_page(Some(LifecycleTerminalPhase::Exited {
            terminate_requested: true,
        })),
    );
    assert!(matches!(
        close.next(now),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::Dispose,
            ..
        }
    ));
    assert!(!close.force_acknowledged);
}

#[test]
fn mutating_unknown_is_permanently_parked() {
    let now = Instant::now();
    let mut mutating = core(now, Some(LifecycleTerminalPhase::AwaitingStart));
    step(
        &mut mutating,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
    );
    assert!(!mutating.is_resumable());
    assert!(matches!(
        mutating.status(),
        WorkbenchTabCloseStatus::OutcomeUnknown {
            phase: UnknownPhase::Dispose,
            ..
        }
    ));
    assert_eq!(
        mutating.next(now + ATTEMPT_DURATION),
        CloseContinuationNext::Parked
    );
}

#[test]
fn foreign_token_and_wrong_generation_cannot_settle_or_replace_pending_step() {
    let now = Instant::now();
    let mut first = core(now, None);
    let mut second = core(now, None);
    let first_token = prepare_step(&mut first, now, NonZeroU64::MIN);
    let foreign = prepare_step(&mut second, now, NonZeroU64::MIN);
    assert!(
        first
            .confirm_admitted(&foreign, now, NonZeroU64::MIN)
            .is_err()
    );
    assert!(
        first
            .confirm_admitted(&first_token, now, NonZeroU64::new(2).expect("generation"))
            .is_err()
    );
    assert!(first.has_outstanding());
    assert_eq!(first.submitted_steps, 0);
    first
        .confirm_admitted(&first_token, now, NonZeroU64::MIN)
        .expect("exact token");
    assert!(
        first
            .confirm_admitted(&first_token, now, NonZeroU64::MIN)
            .is_err()
    );
}

#[test]
fn mismatched_result_echo_never_claims_disposal() {
    let now = Instant::now();
    let mut close = core(now, Some(LifecycleTerminalPhase::AwaitingStart));
    let token = prepare_step(&mut close, now, NonZeroU64::MIN);
    close
        .confirm_admitted(&token, now, NonZeroU64::MIN)
        .expect("admitted");
    let mut wrong = token.request();
    wrong.request_id = OperationId::from_bytes([99; 16]).expect("foreign request");
    let result = TerminalLifecycleResult::new(wrong, TerminalLifecycleOutcome::Disposed)
        .expect("valid foreign frame");
    close
        .settle_step(
            token,
            now,
            NonZeroU64::MIN,
            CloseStepObservation::Result(&result),
        )
        .expect("retain ambiguity");
    assert!(matches!(
        close.status(),
        WorkbenchTabCloseStatus::OutcomeUnknown {
            reason: UnknownReason::InvalidResponse,
            ..
        }
    ));
    assert!(!close.publication_ready());
}

#[test]
fn cleanup_failure_requires_active_cleanup_and_preserves_existing_parks() {
    let now = Instant::now();
    let mut close = core(now, Some(LifecycleTerminalPhase::AwaitingStart));
    assert!(!close.target_cleanup_failed());
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::Disposed,
    );
    assert!(close.target_cleanup_failed());
    assert!(!close.target_cleanup_failed());
    assert!(close.is_resumable());
    resume(&mut close, now);
    assert_eq!(
        close.next(now + ATTEMPT_DURATION),
        CloseContinuationNext::Parked
    );
    let budget_status = close.status();
    assert!(!close.target_cleanup_failed());
    assert_eq!(close.status(), budget_status);
    resume(&mut close, now + ATTEMPT_DURATION);
    assert!(close.target_cleanup_complete());
    assert!(!close.target_cleanup_failed());
    assert!(close.publication_ready());
}

#[test]
fn publication_failure_and_generation_exhaustion_preserve_disposal_and_cleanup() {
    let now = Instant::now();
    let mut close = core(now, Some(LifecycleTerminalPhase::AwaitingStart));
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::Disposed,
    );
    assert!(!close.publication_ready());
    assert!(close.target_cleanup_complete());
    assert!(close.publication_ready());
    close.publication_failed(PublicationReason::PreparationFailed);
    assert!(close.is_resumable());
    resume(&mut close, now);
    assert_eq!(close.next(now), CloseContinuationNext::Publication);
    close.publication_failed(PublicationReason::PreparationFailed);
    close.generation_exhausted();
    assert!(matches!(
        close.status(),
        WorkbenchTabCloseStatus::DisposedAwaitingPublication {
            reason: PublicationReason::PreparationFailed,
            resumable: false
        }
    ));
    assert!(!close.is_resumable());
}

#[test]
fn publication_commit_is_immutable_under_later_core_queries() {
    let now = Instant::now();
    let mut close = core(now, Some(LifecycleTerminalPhase::Failed));
    step(
        &mut close,
        now,
        NonZeroU64::MIN,
        TerminalLifecycleOutcome::Disposed,
    );
    assert!(close.target_cleanup_complete());
    let revision = WorkbenchRevision::parse("9").expect("revision");
    close.commit_closed(revision);
    close.publication_failed(PublicationReason::AuthorityRetired);
    close.generation_exhausted();
    assert_eq!(
        close.next(now + ATTEMPT_DURATION),
        CloseContinuationNext::Closed
    );
    assert_eq!(close.status(), WorkbenchTabCloseStatus::Closed { revision });
}

#[test]
fn post_ack_grace_overflow_retains_signal_and_permanently_parks() {
    let now = Instant::now();
    let mut lower = 0_u64;
    let mut upper = u64::MAX;
    while lower < upper {
        let middle = lower + (upper - lower) / 2 + 1;
        if now.checked_add(Duration::from_secs(middle)).is_some() {
            lower = middle;
        } else {
            upper = middle - 1;
        }
    }
    let last = now
        .checked_add(Duration::from_secs(lower))
        .expect("last representable second");
    assert!(last.checked_add(TERMINATE_GRACE).is_none());
    assert!(CloseContinuation::prepare(last, None).is_none());
    let mut close = core(
        now,
        Some(LifecycleTerminalPhase::Running {
            terminate_requested: false,
        }),
    );
    let token = prepare_step(&mut close, now, NonZeroU64::MIN);
    close
        .confirm_admitted(&token, now, NonZeroU64::MIN)
        .expect("admitted");
    let result =
        TerminalLifecycleResult::new(token.request(), TerminalLifecycleOutcome::TerminateAccepted)
            .expect("acknowledged");
    close
        .settle_step(
            token,
            last,
            NonZeroU64::MIN,
            CloseStepObservation::Result(&result),
        )
        .expect("known acknowledgment retained");
    assert!(close.terminate_acknowledged);
    assert_eq!(close.grace_deadline, None);
    assert!(matches!(
        close.status(),
        WorkbenchTabCloseStatus::CleanupBlocked {
            reason: BlockedReason::CleanupIncomplete,
            resumable: false
        }
    ));
    assert!(!close.is_resumable());
    assert_eq!(close.next(last), CloseContinuationNext::Parked);
}

#[test]
fn unfinished_and_containing_phases_do_not_authorize_disposal() {
    let now = Instant::now();
    for phase in [
        LifecycleTerminalPhase::Finishing {
            terminate_requested: true,
        },
        LifecycleTerminalPhase::ContainingFailure,
    ] {
        let mut close = core(now, None);
        step(&mut close, now, NonZeroU64::MIN, final_page(Some(phase)));
        assert!(matches!(
            close.next(now),
            CloseContinuationNext::Lifecycle {
                action: TerminalLifecycleAction::Terminate,
                ..
            }
        ));
        assert!(!close.publication_ready());
    }
}

#[test]
fn last_budgeted_page_proving_absence_still_parks_before_dispose() {
    let now = Instant::now();
    let mut close = core(now, None);
    for ordinal in 1_u8..32 {
        step(
            &mut close,
            now + QUERY_SPACING * u32::from(ordinal - 1),
            NonZeroU64::MIN,
            TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::StaleCursor),
        );
    }
    let at = now + QUERY_SPACING * 31;
    step(&mut close, at, NonZeroU64::MIN, final_page(None));
    assert_eq!(close.next(at), CloseContinuationNext::Parked);
    assert!(matches!(
        close.status(),
        WorkbenchTabCloseStatus::CleanupBlocked {
            reason: BlockedReason::ContinuationBudgetExhausted,
            resumable: true
        }
    ));
    assert!(!close.publication_ready());
    resume(&mut close, at);
    assert!(matches!(
        close.next(at),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::Dispose,
            ..
        }
    ));
}

#[test]
fn host_unavailable_before_handoff_preserves_the_admitted_server_step_charge() {
    let now = Instant::now();
    for phase in [
        None,
        Some(LifecycleTerminalPhase::AwaitingStart),
        Some(LifecycleTerminalPhase::Running {
            terminate_requested: false,
        }),
    ] {
        let mut close = core(now, phase);
        step(
            &mut close,
            now,
            NonZeroU64::MIN,
            TerminalLifecycleOutcome::HostUnavailableBeforeHandoff,
        );
        assert_eq!(close.submitted_steps, 1);
        assert!(!close.publication_ready());
    }
}

/// A retryable step failure keeps the attempt running and takes the same stage again after the
/// spacing; the budget park reports the last such cause.
#[test]
fn retryable_failure_retries_the_same_stage_and_the_budget_park_reports_its_cause() {
    let now = Instant::now();
    for (phase, outcome, reason) in [
        (
            None,
            TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
            BlockedReason::InventoryIncomplete,
        ),
        (
            None,
            TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::PageRejected),
            BlockedReason::InventoryIncomplete,
        ),
        (
            Some(LifecycleTerminalPhase::AwaitingStart),
            TerminalLifecycleOutcome::HostUnavailableBeforeHandoff,
            BlockedReason::WorkerUnavailable,
        ),
        (
            Some(LifecycleTerminalPhase::AwaitingStart),
            TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::HostRefused),
            BlockedReason::HostRefused,
        ),
    ] {
        let mut close = core(now, phase);
        let CloseContinuationNext::Lifecycle { action, .. } = close.next(now) else {
            panic!("first step");
        };
        step(&mut close, now, NonZeroU64::MIN, outcome);
        assert!(matches!(
            close.status(),
            WorkbenchTabCloseStatus::Closing { .. }
        ));
        assert!(!close.is_resumable());
        assert_eq!(
            close.next(now),
            CloseContinuationNext::WaitUntil(now + QUERY_SPACING)
        );
        let retry = now + QUERY_SPACING;
        assert!(matches!(
            close.next(retry),
            CloseContinuationNext::Lifecycle { action: again, .. } if again == action
        ));
        step(&mut close, retry, NonZeroU64::MIN, outcome);
        assert_eq!(
            close.next(now + ATTEMPT_DURATION),
            CloseContinuationNext::Parked
        );
        assert_eq!(
            close.status(),
            WorkbenchTabCloseStatus::CleanupBlocked {
                reason,
                resumable: true
            }
        );
    }
}
