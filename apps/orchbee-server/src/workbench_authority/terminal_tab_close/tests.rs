use crate::test_support::WorkbenchFixture;
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, LeaseExpiresAtUnixMilliseconds, LeaseId,
    LifecycleTerminalPhase, OperationId, TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
    TerminalLifecycleAuthorization, TerminalLifecycleOutcome, TerminalLifecycleRequest,
    TerminalLifecycleResult, TerminalOperationLease,
};
use orchbee_workbench_protocol::{
    PresentationBinding, ProtocolVersion, SessionId, WorkbenchAction, WorkbenchCommandOutcome,
    WorkbenchCommandUnavailableReason, WorkbenchLabel, WorkbenchRejectionReason, WorkbenchRequest,
    WorkbenchResponse, WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseResumeOutcome, WorkbenchTabCloseResumeRejectionReason,
    WorkbenchTabMoveDirection, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
};

use super::super::{
    MAXIMUM_LEDGER_ENTRIES, WorkbenchGrant, WorkbenchTabCreateBegin, WorkbenchTabCreatePreparation,
    WorkbenchTabMoveBegin, WorkbenchTabMovePreparation, identity,
};
use super::*;
use crate::workbench_projection::WorkbenchProjectionStore;

mod aggregate;
mod budget;
mod context;
mod credit;
mod panel;
mod publication;
mod resume;
mod workspace;

#[test]
fn receipt_identity_reads_the_exact_retained_close_and_checks_the_session_grant() {
    use orchbee_workbench_protocol::{
        CloseOperationOutcome, CloseOperationRead, CloseOperationTarget,
    };
    let (mut authority, grant) = fixture(1);
    let close = close_command(&authority, 980, 0);
    accept(&mut authority, &grant, close.clone(), Instant::now());
    let read = CloseOperationRead {
        protocol_version: ProtocolVersion,
        request_id: request_id(981),
        session_id: authority.session_id().clone(),
        operation_id: close.request_id,
    };
    let result = authority.read_close_operation(Some(&grant), read.clone());
    assert!(result.answers(&read));
    assert!(matches!(
        result.outcome,
        CloseOperationOutcome::Available {
            target: CloseOperationTarget::Tab { .. },
            ..
        }
    ));
    assert!(matches!(
        authority.read_close_operation(None, read.clone()).outcome,
        CloseOperationOutcome::Denied {}
    ));
    let missing = CloseOperationRead {
        operation_id: request_id(999),
        ..read
    };
    assert!(matches!(
        authority
            .read_close_operation(Some(&grant), missing)
            .outcome,
        CloseOperationOutcome::NotFound {}
    ));
}

pub(super) fn fixture(count: u128) -> (WorkbenchAuthority, WorkbenchGrant) {
    let mut authority =
        WorkbenchAuthority::workspace_fixture(SessionId::parse(&identity(100)).unwrap()).unwrap();
    let grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone())
            .with_terminal_lifecycle();
    if count > 0 {
        authority.append_fixture_tab(binding(700)).unwrap();
        // Close accounting starts like a restored Session: the first Tab exists without a
        // retained TabCreate, so only the scenario's own requests occupy the ledger.
        authority.ledger.clear();
        authority.ledger_bytes = 0;
    }
    for index in 1..count {
        let command = create_command(&authority, 500 + index, &format!("tab {index}"));
        let WorkbenchTabCreateBegin::Admitted(admission) =
            authority.begin_terminal_tab_create(Some(&grant), command)
        else {
            panic!("Create admission")
        };
        let WorkbenchTabCreatePreparation::Prepared(prepared) =
            admission.finish(binding(700 + index), None)
        else {
            panic!("Create preparation")
        };
        let pending = prepared.commit().into_pending();
        let _settled = authority
            .prepare_terminal_tab_settlement(
                pending,
                WorkbenchTerminalOutcome::Started {
                    geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                },
            )
            .unwrap()
            .commit();
    }
    (authority, grant)
}

fn binding(value: u128) -> PresentationBinding {
    PresentationBinding {
        worker_id: orchbee_workbench_protocol::WorkerId::parse(&identity(2000)).unwrap(),
        terminal_id: orchbee_workbench_protocol::TerminalId::parse(&identity(value)).unwrap(),
    }
}

pub(super) fn request_id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&identity(value)).unwrap()
}

fn command(authority: &WorkbenchAuthority, id: u128, action: WorkbenchAction) -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: request_id(id),
        session_id: authority.session_id().clone(),
        expected_revision: authority.projection.snapshot().revision,
        action,
    }
}

fn create_command(authority: &WorkbenchAuthority, id: u128, label: &str) -> WorkbenchCommand {
    command(
        authority,
        id,
        WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: authority.projection.snapshot().navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            label: WorkbenchLabel::parse(label).unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    )
}

pub(super) fn close_command(
    authority: &WorkbenchAuthority,
    id: u128,
    member: usize,
) -> WorkbenchCommand {
    let membership = authority.fixture_membership().unwrap();
    command(
        authority,
        id,
        WorkbenchAction::TabClose {
            terminal_context_id: membership.terminal_context_id().clone(),
            tab_id: membership.members()[member].tab_id().clone(),
        },
    )
}

pub(super) fn runtime(member: &WorkbenchTerminalTab) -> WorkbenchTabCloseRuntimeTarget {
    let terminal = u128::from_str_radix(member.binding().terminal_id.as_str(), 16).unwrap();
    WorkbenchTabCloseRuntimeTarget::new(
        WorkerId::from_bytes(2000_u128.to_be_bytes()).unwrap(),
        WorkerGeneration::new(NonZeroU64::MIN),
        TerminalId::from_bytes(terminal.to_be_bytes()).unwrap(),
        HostTerminalIdentity::new(
            HostCreationId::from_bytes((terminal + 1000).to_be_bytes()).unwrap(),
            HostEpoch::from_bytes(9000_u128.to_be_bytes()).unwrap(),
            HostTerminalId::from_bytes((terminal + 2000).to_be_bytes()).unwrap(),
        ),
        TerminalFence::new(NonZeroU64::MIN),
    )
}

pub(super) fn primary<'a>(
    authority: &'a mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchTabClosePrimaryAdmission<'a> {
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(grant), command)
    else {
        panic!("Close admission")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("primary Close")
    };
    *primary
}

pub(super) fn accept(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
    now: Instant,
) -> WorkbenchCommandResult {
    let primary = primary(authority, grant, command);
    let runtime = runtime(primary.member());
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish(runtime, Some(LifecycleTerminalPhase::AwaitingStart), now)
    else {
        panic!("Close preparation")
    };
    prepared.commit()
}

pub(super) fn status(
    authority: &WorkbenchAuthority,
    grant: &WorkbenchGrant,
    close: &WorkbenchCommand,
) -> WorkbenchTabCloseReadOutcome {
    let (terminal_context_id, tab_id) = match &close.action {
        WorkbenchAction::TabClose {
            terminal_context_id,
            tab_id,
        }
        | WorkbenchAction::PanelClose {
            terminal_context_id,
            tab_id,
            ..
        } => (terminal_context_id, tab_id),
        _ => panic!(),
    };
    authority
        .read_terminal_tab_close(
            Some(grant),
            WorkbenchTabCloseRead {
                protocol_version: ProtocolVersion,
                request_id: request_id(99000),
                session_id: close.session_id.clone(),
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
            },
        )
        .outcome
}

/// Starts from a close that has spent its automatic renewals, so its next park stays parked for
/// a person's Resume.
pub(super) fn spend_automatic_renewals(
    authority: &mut WorkbenchAuthority,
    close: &WorkbenchRequestId,
) {
    authority
        .close_record_mut(close)
        .unwrap()
        .automatic_renewals = 0;
}

fn submit(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchCommandResult {
    let WorkbenchResponse::Command(result) = authority.execute(
        Some(grant),
        WorkbenchRequest::Command(command).try_into().unwrap(),
    ) else {
        panic!()
    };
    result
}

fn step_request(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    now: Instant,
) -> TerminalLifecycleRequest {
    let CloseContinuationNext::Lifecycle {
        action,
        step_ordinal,
    } = authority
        .next_terminal_tab_close_step(close_id, now)
        .unwrap()
    else {
        panic!("lifecycle step")
    };
    let runtime = authority
        .terminal_tab_close_target(close_id)
        .unwrap()
        .runtime()
        .expect("created Host runtime");
    TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: runtime.terminal_id(),
            worker_id: runtime.worker_id(),
            worker_generation: runtime.worker_generation(),
            binding_fence: runtime.fence(),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes(8000_u128.to_be_bytes()).unwrap(),
                LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
            ),
        },
        close_operation_id: OperationId::from_bytes(
            u128::from_str_radix(close_id.as_str(), 16)
                .unwrap()
                .to_be_bytes(),
        )
        .unwrap(),
        request_id: OperationId::from_bytes((8100 + u128::from(step_ordinal.get())).to_be_bytes())
            .unwrap(),
        attempt_generation: NonZeroU64::new(
            authority
                .terminal_tab_close_generation(close_id)
                .unwrap()
                .value(),
        )
        .unwrap(),
        step_ordinal,
        target: runtime.host_identity(),
        action,
    }
}

fn dispose_and_cleanup(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    now: Instant,
) {
    let request = step_request(authority, close_id, now);
    let token = authority
        .reserve_terminal_tab_close_step(close_id, now, request)
        .unwrap();
    authority
        .confirm_terminal_tab_close_step_admitted(close_id, &token, now)
        .unwrap();
    authority
        .settle_terminal_tab_close_step(
            close_id,
            token,
            now,
            CloseStepObservation::Result(
                &TerminalLifecycleResult::new(request, TerminalLifecycleOutcome::Disposed).unwrap(),
            ),
        )
        .unwrap();
    let target = authority
        .terminal_tab_close_target(close_id)
        .unwrap()
        .runtime()
        .expect("created Host runtime");
    authority
        .complete_terminal_tab_close_target_cleanup(close_id, target)
        .unwrap();
}

fn set_revision(authority: &mut WorkbenchAuthority, revision: u64) {
    let mut snapshot = authority.projection.snapshot().clone();
    snapshot.revision = WorkbenchRevision::new(revision);
    authority.projection = WorkbenchProjectionStore::load(
        &orchbee_workbench_protocol::serialize_document(
            &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot),
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn admission_is_revision_free_drop_is_effect_free_and_lifecycle_authority_is_independent() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    let before = authority.projection.snapshot().clone();
    let denied =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone());
    assert!(matches!(
        authority.begin_terminal_tab_close(Some(&denied), close.clone()),
        WorkbenchTabCloseBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Denied {},
            ..
        })
    ));
    let lifecycle_only =
        WorkbenchGrant::read_only(authority.session_id().clone()).with_terminal_lifecycle();
    let primary = primary(&mut authority, &lifecycle_only, close.clone());
    let runtime = runtime(primary.member());
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish(runtime, Some(LifecycleTerminalPhase::AwaitingStart), now)
    else {
        panic!()
    };
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.ledger.len(), 0);
    accept(&mut authority, &grant, close.clone(), now);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
    assert!(
        authority
            .close_record(&close.request_id)
            .unwrap()
            .resume_receipt
            .is_none()
    );
}

#[test]
fn alias_keeps_primary_revision_and_replays_after_retirement_without_runtime_lookup() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    let accepted = accept(&mut authority, &grant, close.clone(), now);
    let rename = command(
        &authority,
        10001,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("later").unwrap(),
        },
    );
    let ordinary = submit(&mut authority, &grant, rename.clone());
    let alias = close_command(&authority, 10002, 0);
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), alias.clone())
    else {
        panic!()
    };
    let WorkbenchTabCloseResolution::Ready(joined) = admission.resolve_after_composition() else {
        panic!("alias must not query runtime")
    };
    assert_eq!(joined.outcome, accepted.outcome);
    assert_eq!(joined.request_id, alias.request_id);
    assert_ne!(alias.expected_revision, close.expected_revision);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
    authority.retire_terminal_membership();
    for (request, expected) in [(close.clone(), accepted), (alias, joined)] {
        let WorkbenchTabCloseBegin::Ready(replayed) =
            authority.begin_terminal_tab_close(Some(&grant), request)
        else {
            panic!()
        };
        assert_eq!(replayed, expected);
    }
    assert!(matches!(
        submit(&mut authority, &grant, rename).outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
        }
    ));
    assert!(matches!(
        ordinary.outcome,
        WorkbenchCommandOutcome::ProjectCreated { .. }
    ));
    assert!(matches!(
        status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available { .. }
    ));
}

#[test]
fn runtime_member_mismatch_retires_before_acceptance_and_busy_allocates_nothing() {
    let (mut authority, grant) = fixture(2);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    let initial_entries = authority.ledger.len();
    let refused = primary(&mut authority, &grant, close.clone())
        .runtime_unavailable(WorkbenchTabCloseRuntimeUnavailableReason::TerminalBusy);
    assert!(matches!(
        refused.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(authority.ledger.len(), initial_entries);
    let other = runtime(&authority.fixture_membership().unwrap().members()[1]);
    assert!(matches!(
        primary(&mut authority, &grant, close).finish(other, None, now),
        WorkbenchTabClosePreparation::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
            },
            ..
        })
    ));
    assert_eq!(authority.ledger.len(), initial_entries);
    assert!(authority.retired);
}

#[test]
fn create_and_move_entrypoints_cannot_replay_close_without_its_lifecycle_grant() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    accept(&mut authority, &grant, close.clone(), now);
    let other_grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone());
    assert!(matches!(
        authority.begin_terminal_tab_create(Some(&other_grant), close.clone()),
        WorkbenchTabCreateBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Denied {},
            ..
        })
    ));
    assert!(matches!(
        authority.begin_terminal_tab_move(Some(&other_grant), close),
        WorkbenchTabMoveBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Denied {},
            ..
        })
    ));
    assert_eq!(authority.ledger.len(), 1);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
}
