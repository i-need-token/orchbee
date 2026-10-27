use super::*;
use crate::test_support::WorkbenchFixture;

fn resume_request(close: &WorkbenchCommand, id: u128, generation: u64) -> WorkbenchTabCloseResume {
    let WorkbenchAction::TabClose {
        terminal_context_id,
        tab_id,
    } = &close.action
    else {
        panic!()
    };
    WorkbenchTabCloseResume {
        protocol_version: ProtocolVersion,
        request_id: request_id(id),
        session_id: close.session_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        close_request_id: close.request_id.clone(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(generation).unwrap(),
    }
}

fn resume(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    request: WorkbenchTabCloseResume,
    now: Instant,
) -> WorkbenchTabCloseResumeResult {
    match authority.begin_terminal_tab_close_resume(Some(grant), request) {
        WorkbenchTabCloseResumeBegin::Ready(result) => result,
        WorkbenchTabCloseResumeBegin::Admitted(admission) => match admission.finish(now) {
            WorkbenchTabCloseResumePreparation::Ready(result) => result,
            WorkbenchTabCloseResumePreparation::Prepared(prepared) => prepared.commit(),
        },
    }
}

#[test]
fn full_ordinary_ledger_does_not_block_resume_receipt_replay_or_replacement() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    accept(&mut authority, &grant, close.clone(), now);
    let member = authority.fixture_membership().unwrap().members()[0].clone();
    let context = authority
        .fixture_membership()
        .unwrap()
        .terminal_context_id()
        .clone();
    for index in 1..MAXIMUM_LEDGER_ENTRIES {
        let noop = command(
            &authority,
            20000 + index as u128,
            WorkbenchAction::TabRename {
                terminal_context_id: context.clone(),
                tab_id: member.tab_id().clone(),
                label: WorkbenchLabel::parse("1").unwrap(),
            },
        );
        let outcome = submit(&mut authority, &grant, noop).outcome;
        // The fixture Tab already carries this explicit label, so every rename is retained as a no-op.
        assert!(matches!(outcome, WorkbenchCommandOutcome::Unchanged { .. }));
    }
    let bytes = authority.ledger_bytes;
    let revision = authority.projection.snapshot().revision;
    let expired = now + Duration::from_secs(30);
    assert_eq!(
        authority
            .next_terminal_tab_close_step(&close.request_id, expired)
            .unwrap(),
        CloseContinuationNext::Parked
    );
    let first = resume_request(&close, 11000, 1);
    let accepted = resume(&mut authority, &grant, first.clone(), expired);
    assert_eq!(
        accepted.outcome,
        WorkbenchTabCloseResumeOutcome::Resumed {
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
        }
    );
    assert_eq!(
        resume(
            &mut authority,
            &grant,
            first.clone(),
            expired + Duration::from_secs(1)
        ),
        accepted
    );
    let mut conflict = first.clone();
    conflict.expected_attempt_generation = WorkbenchTabCloseAttemptGeneration::new(2).unwrap();
    assert_eq!(
        resume(&mut authority, &grant, conflict, expired).outcome,
        WorkbenchTabCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::RequestIdentityConflict
        }
    );
    let later = expired + Duration::from_secs(30);
    authority
        .next_terminal_tab_close_step(&close.request_id, later)
        .unwrap();
    let second = resume_request(&close, 11001, 2);
    assert!(matches!(
        resume(&mut authority, &grant, second, later).outcome,
        WorkbenchTabCloseResumeOutcome::Resumed { .. }
    ));
    assert_eq!(
        resume(&mut authority, &grant, first, later).outcome,
        WorkbenchTabCloseResumeOutcome::Stale {
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(3).unwrap()
        }
    );
    assert_eq!(authority.ledger.len(), MAXIMUM_LEDGER_ENTRIES);
    assert_eq!(authority.ledger_bytes, bytes);
    assert_eq!(authority.projection.snapshot().revision, revision);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
}

#[test]
fn prepared_and_dispatched_receipts_fence_resume_and_unknown_aliases_do_not_resume() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    let accepted = accept(&mut authority, &grant, close.clone(), now);
    let request = step_request(&mut authority, &close.request_id, now);
    let token = authority
        .reserve_terminal_tab_close_step(&close.request_id, now, request)
        .unwrap();
    let late = now + Duration::from_secs(31);
    assert_eq!(
        resume(
            &mut authority,
            &grant,
            resume_request(&close, 11000, 1),
            late
        )
        .outcome,
        WorkbenchTabCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::NotResumable
        }
    );
    authority
        .confirm_terminal_tab_close_step_admitted(&close.request_id, &token, late)
        .unwrap();
    authority
        .settle_terminal_tab_close_step(
            &close.request_id,
            token,
            late,
            CloseStepObservation::OutcomeUnknown(
                orchbee_capability_node_protocol::LifecycleUnknownReason::TransportLost,
            ),
        )
        .unwrap();
    assert!(matches!(
        status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::OutcomeUnknown { .. },
            ..
        }
    ));
    let alias = close_command(&authority, 10001, 0);
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), alias)
    else {
        panic!()
    };
    let WorkbenchTabCloseResolution::Ready(joined) = admission.resolve_after_composition() else {
        panic!()
    };
    assert_eq!(joined.outcome, accepted.outcome);
    assert_eq!(
        resume(
            &mut authority,
            &grant,
            resume_request(&close, 11001, 1),
            late
        )
        .outcome,
        WorkbenchTabCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::NotResumable
        }
    );
}

#[test]
fn generation_exhaustion_keeps_known_facts_and_dropped_resume_keeps_receipt_and_clock() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    accept(&mut authority, &grant, close.clone(), now);
    let expired = now + Duration::from_secs(30);
    authority
        .next_terminal_tab_close_step(&close.request_id, expired)
        .unwrap();
    let before = status(&authority, &grant, &close);
    let WorkbenchTabCloseResumeBegin::Admitted(admission) =
        authority.begin_terminal_tab_close_resume(Some(&grant), resume_request(&close, 11000, 1))
    else {
        panic!()
    };
    let WorkbenchTabCloseResumePreparation::Prepared(prepared) = admission.finish(expired) else {
        panic!()
    };
    drop(prepared);
    assert_eq!(status(&authority, &grant, &close), before);
    assert!(
        authority
            .close_record(&close.request_id)
            .unwrap()
            .resume_receipt
            .is_none()
    );
    authority
        .close_record_mut(&close.request_id)
        .unwrap()
        .generation = WorkbenchTabCloseAttemptGeneration::new(u64::MAX).unwrap();
    assert_eq!(
        resume(
            &mut authority,
            &grant,
            resume_request(&close, 11001, u64::MAX),
            expired
        )
        .outcome,
        WorkbenchTabCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::GenerationExhausted
        }
    );
    assert!(matches!(status(&authority, &grant, &close), WorkbenchTabCloseReadOutcome::Available {
        status: WorkbenchTabCloseStatus::CleanupBlocked { reason: orchbee_workbench_protocol::WorkbenchTabCloseBlockedReason::ResumeGenerationExhausted, resumable: false }, .. }));
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
}

#[test]
fn resume_authorization_operation_and_generation_precedence_are_effect_free() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    accept(&mut authority, &grant, close.clone(), now);
    let parked = now + Duration::from_secs(30);
    authority
        .next_terminal_tab_close_step(&close.request_id, parked)
        .unwrap();
    let before = status(&authority, &grant, &close);
    let bytes = authority.ledger_bytes;
    let reader = WorkbenchGrant::read_only(authority.session_id().clone());
    let valid = resume_request(&close, 11000, 1);
    assert!(matches!(
        authority.begin_terminal_tab_close_resume(Some(&reader), valid.clone()),
        WorkbenchTabCloseResumeBegin::Ready(WorkbenchTabCloseResumeResult {
            outcome: WorkbenchTabCloseResumeOutcome::Denied {},
            ..
        })
    ));
    for (mut changed, expected) in [
        (
            valid.clone(),
            WorkbenchTabCloseResumeRejectionReason::OperationUnavailable,
        ),
        (
            valid.clone(),
            WorkbenchTabCloseResumeRejectionReason::OperationMismatch,
        ),
        (
            valid.clone(),
            WorkbenchTabCloseResumeRejectionReason::GenerationMismatch,
        ),
    ] {
        match expected {
            WorkbenchTabCloseResumeRejectionReason::OperationUnavailable => {
                changed.close_request_id = request_id(99999)
            }
            WorkbenchTabCloseResumeRejectionReason::OperationMismatch => {
                changed.tab_id =
                    orchbee_workbench_protocol::WorkbenchTabId::parse(&identity(99999)).unwrap()
            }
            _ => {
                changed.expected_attempt_generation =
                    WorkbenchTabCloseAttemptGeneration::new(2).unwrap()
            }
        }
        assert_eq!(
            resume(&mut authority, &grant, changed, parked).outcome,
            WorkbenchTabCloseResumeOutcome::Rejected { reason: expected }
        );
    }
    let WorkbenchTabCloseResumeBegin::Admitted(admission) =
        authority.begin_terminal_tab_close_resume(Some(&grant), valid)
    else {
        panic!()
    };
    assert!(matches!(
        admission.runtime_unavailable().outcome,
        WorkbenchTabCloseResumeOutcome::Unavailable { .. }
    ));
    assert_eq!(status(&authority, &grant, &close), before);
    assert_eq!(authority.ledger_bytes, bytes);
}
