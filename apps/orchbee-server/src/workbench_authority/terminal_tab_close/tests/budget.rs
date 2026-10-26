//! Budget fixtures for the actual controlled-payload accounting: positive
//! fixed heap on an empty scope, Vec-capacity slack, receipt coexistence
//! inside the two reserved slots, oversized-candidate denial with zero
//! effect, checked overflow, the exact ±1 ledger boundary, and the same
//! accounting on the leaf Tab path.

use std::mem::size_of;
use std::time::Instant;

use super::super::continuation::CloseContinuation;
use super::super::reservation::{
    entry_reservation, maximum_wire_charge, record_controlled_heap, tab_receipt_owned_bytes,
    workspace_receipt_owned_bytes,
};
use super::workspace::workspace_fixture;
use super::*;
use crate::workbench_authority::{
    RetainedRequest, WorkbenchWorkspaceCloseBegin, WorkbenchWorkspaceCloseResumeBegin,
    WorkbenchWorkspaceCloseResumePreparation,
};
use orchbee_workbench_protocol::{
    WorkbenchTabCloseUnavailableReason, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome,
};

fn workspace_close_command(
    authority: &WorkbenchAuthority,
    id: u128,
    project_index: usize,
    workspace_index: usize,
) -> WorkbenchCommand {
    let project = &authority.projection.snapshot().navigation.projects[project_index];
    command(
        authority,
        id,
        WorkbenchAction::WorkspaceClose {
            workspace_id: authority
                .snapshot()
                .navigation
                .workspaces_in_project(Some(&project.project_id))
                .nth(workspace_index)
                .unwrap()
                .workspace_id
                .clone(),
        },
    )
}

fn primary_workspace<'a>(
    authority: &'a mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchTabClosePrimaryAdmission<'a> {
    let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
        authority.begin_workspace_close(Some(grant), command)
    else {
        panic!("Workspace Close admission")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("primary Workspace Close")
    };
    *primary
}

fn accept_workspace(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
    now: Instant,
) -> WorkbenchCommandResult {
    let primary = primary_workspace(authority, grant, command);
    let runtimes = primary
        .members()
        .iter()
        .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(runtimes, now)
    else {
        panic!("Workspace Close preparation")
    };
    prepared.commit()
}

fn workspace_resume_request(
    close: &WorkbenchCommand,
    id: u128,
    generation: u64,
) -> WorkbenchWorkspaceCloseResume {
    let WorkbenchAction::WorkspaceClose { workspace_id } = &close.action else {
        panic!()
    };
    WorkbenchWorkspaceCloseResume {
        protocol_version: ProtocolVersion,
        request_id: request_id(id),
        site_id: close.site_id.clone(),
        workspace_id: workspace_id.clone(),
        close_request_id: close.request_id.clone(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(generation).unwrap(),
    }
}

fn workspace_resume(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    request: WorkbenchWorkspaceCloseResume,
    now: Instant,
) -> orchbee_workbench_protocol::WorkbenchWorkspaceCloseResumeResult {
    match authority.begin_workspace_close_resume(Some(grant), request) {
        WorkbenchWorkspaceCloseResumeBegin::Ready(result) => result,
        WorkbenchWorkspaceCloseResumeBegin::Admitted(admission) => match admission.finish(now) {
            WorkbenchWorkspaceCloseResumePreparation::Ready(result) => result,
            WorkbenchWorkspaceCloseResumePreparation::Prepared(prepared) => prepared.commit(),
        },
    }
}

fn leaf_resume_request(
    close: &WorkbenchCommand,
    id: u128,
    generation: u64,
) -> WorkbenchTabCloseResume {
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
        site_id: close.site_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        close_request_id: close.request_id.clone(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(generation).unwrap(),
    }
}

fn leaf_resume(
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

/// N=0: the fixed heap charge stays positive — the boxed record, the ledger
/// key, and every owned identity buffer are all counted.
#[test]
fn empty_workspace_close_reserves_positive_fixed_heap_for_record_key_and_identities() {
    let (mut authority, grant) = workspace_fixture(0);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let acknowledgment = accept_workspace(&mut authority, &grant, close.clone(), now);
    let reserved = authority.ledger_bytes;
    let record = authority.close_record(&close.request_id).unwrap();
    assert!(record.targets.is_empty());
    let heap = record_controlled_heap(record).unwrap();
    // Nine owned identity buffers ride the N=0 record — command 3,
    // accepted outcome 4, scope echo 1, retained context 1 — each a
    // full-capacity 32-hex `String`, so the buffers alone exceed the bare
    // struct.
    assert!(heap >= size_of::<CloseRecord>() + 9 * 32);
    let wire = maximum_wire_charge(&close, &acknowledgment, 0).unwrap();
    // The entry also charges the ledger key's inline head, its owned
    // buffer, the `RetainedRequest` enum cell, and both receipt slots on
    // top of wire + heap.
    assert!(
        reserved >= wire + size_of::<WorkbenchRequestId>() + size_of::<RetainedRequest>() + heap
    );
    assert_eq!(
        entry_reservation(&close, &acknowledgment, 0, record, &close.request_id),
        Some(reserved)
    );
    assert_eq!(record.reserved_bytes, reserved);
}

/// Vec slack: capacity above length is charged at actual capacity, not at
/// length and not at an assumed per-member figure.
#[test]
fn retained_vec_capacity_above_len_is_charged_at_actual_capacity() {
    let (mut authority, grant) = workspace_fixture(2);
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let acknowledgment = accept_workspace(&mut authority, &grant, close.clone(), Instant::now());
    let before =
        record_controlled_heap(authority.close_record(&close.request_id).unwrap()).unwrap();
    let record = authority.close_record_mut(&close.request_id).unwrap();
    record.targets.reserve(8);
    record.continuations.reserve(8);
    let target_slack =
        (record.targets.capacity() - record.targets.len()) * size_of::<WorkbenchTabCloseTarget>();
    let continuation_slack = (record.continuations.capacity() - record.continuations.len())
        * size_of::<CloseContinuation>();
    assert!(target_slack > 0);
    assert!(continuation_slack > 0);
    let after = record_controlled_heap(record).unwrap();
    assert_eq!(after - before, target_slack + continuation_slack);
    // The same entry holds wire, key head + buffer, enum cell, the grown
    // heap, and both receipt slots — never less than the known cells.
    assert!(
        entry_reservation(&close, &acknowledgment, 2, record, &close.request_id).unwrap()
            >= maximum_wire_charge(&close, &acknowledgment, 2).unwrap()
                + size_of::<WorkbenchRequestId>()
                + size_of::<RetainedRequest>()
                + after
    );
}

/// Receipt coexistence: the retained receipt and each new candidate share
/// the two reserved slots; a candidate over its slot is busy, never
/// malformed, and leaves record, receipt, generation, ledger, and status
/// untouched.
#[test]
fn resume_receipts_coexist_inside_two_slots_and_an_oversized_candidate_is_busy_without_effect() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    accept_workspace(&mut authority, &grant, close.clone(), now);
    let parked = now + Duration::from_secs(30);
    authority
        .next_terminal_tab_close_step_target(&close.request_id, parked)
        .unwrap();
    let first = workspace_resume(
        &mut authority,
        &grant,
        workspace_resume_request(&close, 11000, 1),
        parked,
    );
    assert!(matches!(
        first.outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
    ));
    // The real receipt's payload equals the reserved slot — template and
    // committed receipt share one shape.
    let record = authority.close_record(&close.request_id).unwrap();
    let request = workspace_resume_request(&close, 11001, 2);
    let result = super::super::control::workspace_resume_result(
        &request,
        WorkbenchWorkspaceCloseResumeOutcome::Resumed {
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(3).unwrap(),
        },
    );
    assert_eq!(
        workspace_receipt_owned_bytes(&request, &result),
        Some(record.receipt_slot_bytes)
    );
    // The second receipt commits while the first is still retained — the
    // two slots cover the coexistence window.
    let later = parked + Duration::from_secs(30);
    authority
        .next_terminal_tab_close_step_target(&close.request_id, later)
        .unwrap();
    let second = workspace_resume(
        &mut authority,
        &grant,
        workspace_resume_request(&close, 11001, 2),
        later,
    );
    assert!(matches!(
        second.outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
    ));
    let record = authority.close_record(&close.request_id).unwrap();
    assert!(record_controlled_heap(record).unwrap() <= record.reserved_bytes);

    // An oversized candidate is ServerBusy with zero retained effect.
    let even_later = later + Duration::from_secs(30);
    authority
        .next_terminal_tab_close_step_target(&close.request_id, even_later)
        .unwrap();
    let record = authority.close_record_mut(&close.request_id).unwrap();
    record.receipt_slot_bytes = 0;
    let generation = record.generation;
    let status = record.status();
    let ledger_bytes = authority.ledger_bytes;
    let ledger_entries = authority.ledger.len();
    let denied = workspace_resume(
        &mut authority,
        &grant,
        workspace_resume_request(&close, 11002, 3),
        even_later,
    );
    assert!(matches!(
        denied.outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Unavailable {
            reason: WorkbenchTabCloseUnavailableReason::ServerBusy
        }
    ));
    let record = authority.close_record(&close.request_id).unwrap();
    assert_eq!(record.generation, generation);
    assert_eq!(record.status(), status);
    assert!(matches!(
        &record.resume_receipt,
        Some(AcceptedResumeReceipt::Workspace { result, .. })
            if matches!(
                result.outcome,
                WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
            )
    ));
    assert_eq!(authority.ledger_bytes, ledger_bytes);
    assert_eq!(authority.ledger.len(), ledger_entries);
    assert!(!authority.retired);
}

/// Checked overflow: a synthetic member count overflows the reservation
/// into `None`, and an overdrawn ledger finishes as a zero-effect failure
/// with no retained record.
#[test]
fn synthetic_member_count_overflow_returns_none_and_an_overdrawn_ledger_inserts_nothing() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let acknowledgment = accept_workspace(&mut authority, &grant, close.clone(), now);
    let record = authority.close_record(&close.request_id).unwrap();
    assert_eq!(
        entry_reservation(
            &close,
            &acknowledgment,
            usize::MAX,
            record,
            &close.request_id
        ),
        None
    );

    let (mut authority, grant) = workspace_fixture(1);
    authority.ledger_bytes = usize::MAX;
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let primary = primary_workspace(&mut authority, &grant, close);
    let runtimes = primary
        .members()
        .iter()
        .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    assert!(matches!(
        primary.finish_members(runtimes, now),
        WorkbenchTabClosePreparation::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
            },
            ..
        })
    ));
    assert!(authority.ledger.is_empty());
    assert_eq!(authority.ledger_bytes, usize::MAX);
    assert!(authority.retired);
}

/// Exact boundary: `ledger_bytes + charge == MAXIMUM_LEDGER_BYTES` admits;
/// one byte past rejects with no ledger delta.
#[test]
fn ledger_bytes_plus_charge_at_maximum_admits_and_one_past_rejects() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    accept_workspace(&mut authority, &grant, close.clone(), now);
    let charge = authority.ledger_bytes;
    let maximum = crate::workbench_authority::MAXIMUM_LEDGER_BYTES;
    let runtimes = |primary: &WorkbenchTabClosePrimaryAdmission<'_>| {
        primary
            .members()
            .iter()
            .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
            .collect::<Vec<_>>()
    };

    let (mut constrained, grant) = workspace_fixture(1);
    constrained.ledger_bytes = maximum - charge;
    let close = workspace_close_command(&constrained, 10000, 1, 0);
    let primary = primary_workspace(&mut constrained, &grant, close);
    let member_runtimes = runtimes(&primary);
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish_members(member_runtimes, now)
    else {
        panic!("the exact boundary must admit")
    };
    drop(prepared.commit());
    assert_eq!(constrained.ledger_bytes, maximum);

    let (mut constrained, grant) = workspace_fixture(1);
    constrained.ledger_bytes = maximum - charge + 1;
    let close = workspace_close_command(&constrained, 10000, 1, 0);
    let primary = primary_workspace(&mut constrained, &grant, close);
    let member_runtimes = runtimes(&primary);
    assert!(matches!(
        primary.finish_members(member_runtimes, now),
        WorkbenchTabClosePreparation::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::RequestLedgerFull
            },
            ..
        })
    ));
    assert!(constrained.ledger.is_empty());
    assert_eq!(constrained.ledger_bytes, maximum - charge + 1);
}

/// The leaf Tab path carries the same actual accounting at N=1 — one future
/// lifecycle-request cell — and the same effect-free busy denial.
#[test]
fn leaf_close_uses_the_same_actual_accounting_and_busy_denial_is_effect_free() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    let acknowledgment = accept(&mut authority, &grant, close.clone(), now);
    let reserved = authority.ledger_bytes;
    let record = authority.close_record(&close.request_id).unwrap();
    assert_eq!(record.targets.len(), 1);
    assert!(
        record_controlled_heap(record).unwrap()
            >= size_of::<CloseRecord>() + size_of::<TerminalLifecycleRequest>()
    );
    assert_eq!(
        entry_reservation(&close, &acknowledgment, 1, record, &close.request_id),
        Some(reserved)
    );
    let request = leaf_resume_request(&close, 11000, 1);
    let result = super::super::control::resume_result(
        &request,
        WorkbenchTabCloseResumeOutcome::Resumed {
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
        },
    );
    assert_eq!(
        tab_receipt_owned_bytes(&request, &result),
        Some(record.receipt_slot_bytes)
    );

    let parked = now + Duration::from_secs(30);
    assert_eq!(
        authority
            .next_terminal_tab_close_step(&close.request_id, parked)
            .unwrap(),
        CloseContinuationNext::Parked
    );
    let record = authority.close_record_mut(&close.request_id).unwrap();
    record.receipt_slot_bytes = 0;
    let generation = record.generation;
    let status = record.status();
    let ledger_bytes = authority.ledger_bytes;
    let denied = leaf_resume(
        &mut authority,
        &grant,
        leaf_resume_request(&close, 11000, 1),
        parked,
    );
    assert!(matches!(
        denied.outcome,
        WorkbenchTabCloseResumeOutcome::Unavailable {
            reason: WorkbenchTabCloseUnavailableReason::ServerBusy
        }
    ));
    let record = authority.close_record(&close.request_id).unwrap();
    assert_eq!(record.generation, generation);
    assert_eq!(record.status(), status);
    assert!(record.resume_receipt.is_none());
    assert_eq!(authority.ledger_bytes, ledger_bytes);
    assert!(!authority.retired);
}
