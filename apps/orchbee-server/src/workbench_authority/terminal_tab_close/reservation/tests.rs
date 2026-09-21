use super::super::tests::{accept, close_command, fixture, request_id, status};
use super::super::{
    WorkbenchTabCloseBegin, WorkbenchTabClosePreparation, WorkbenchTabCloseResolution,
};
use super::*;
use std::time::Instant;

#[test]
fn reservation_covers_simultaneous_maximum_tombstone_status_and_resume_receipt() {
    let (mut authority, grant) = fixture(1);
    let close = close_command(&authority, 10000, 0);
    let acknowledgment = accept(&mut authority, &grant, close.clone(), Instant::now());
    let reserved = authority.ledger_bytes;
    let record = authority.close_record_mut(&close.request_id).unwrap();
    record.generation = WorkbenchTabCloseAttemptGeneration::new(u64::MAX).unwrap();
    record
        .continuation_mut(0)
        .unwrap()
        .commit_closed(WorkbenchRevision::new(u64::MAX));
    let WorkbenchAction::TabClose {
        terminal_context_id,
        tab_id,
    } = &close.action
    else {
        panic!()
    };
    let resume = WorkbenchTabCloseResume {
        protocol_version: ProtocolVersion,
        request_id: request_id(11000),
        site_id: close.site_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        close_request_id: close.request_id.clone(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(u64::MAX).unwrap(),
    };
    let response = super::super::control::resume_result(
        &resume,
        WorkbenchTabCloseResumeOutcome::Resumed {
            attempt_generation: resume.expected_attempt_generation,
        },
    );
    let control = receipt_charge(&resume, &response).unwrap();
    let read = serialize_workbench_response(&WorkbenchResponse::TabCloseRead(
        WorkbenchTabCloseReadResult {
            protocol_version: ProtocolVersion,
            request_id: request_id(12000),
            outcome: status(&authority, &grant, &close),
        },
    ))
    .unwrap()
    .len();
    assert!(reserved >= ledger_size(&close, &acknowledgment).unwrap() + read + control);
    assert_eq!(
        maximum_primary_charge(&close, &acknowledgment, 1),
        Some(reserved)
    );
}

#[test]
fn primary_byte_exhaustion_is_transient_and_aliases_pay_their_own_fixed_entry() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    let acknowledgment = accept(&mut authority, &grant, close.clone(), now);
    let charge = authority.ledger_bytes;
    let alias = close_command(&authority, 10001, 0);
    let alias_charge = ledger_size(
        &alias,
        &WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: alias.request_id.clone(),
            outcome: acknowledgment.outcome.clone(),
        },
    )
    .unwrap();
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), alias)
    else {
        panic!()
    };
    assert!(matches!(
        admission.resolve_after_composition(),
        WorkbenchTabCloseResolution::Ready(_)
    ));
    assert_eq!(authority.ledger_bytes, charge + alias_charge);
    let (mut constrained, grant) = fixture(1);
    constrained.ledger_bytes = super::super::super::MAXIMUM_LEDGER_BYTES - charge + 1;
    let close = close_command(&constrained, 10000, 0);
    let primary = super::super::tests::primary(&mut constrained, &grant, close);
    let runtime = super::super::tests::runtime(primary.member());
    assert!(matches!(
        primary.finish(runtime, None, now),
        WorkbenchTabClosePreparation::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason:
                    orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::RequestLedgerFull
            },
            ..
        })
    ));
    assert!(constrained.ledger.is_empty());
    assert!(!constrained.retired);
}
