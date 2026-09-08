use super::super::{MAXIMUM_LEDGER_ENTRIES, WorkbenchReadOrNavigationRequest};
use super::*;
use crate::workbench_projection::WorkbenchProjectionStore;
use orchbee_workbench_protocol::{
    PanelNode, ProtocolVersion, SiteId, SnapshotReadIntent, TerminalId, WorkbenchLabel,
    WorkbenchProjectId, WorkbenchRequest, WorkbenchResponse, WorkbenchWorkspaceId, WorkerId,
};
use std::collections::BTreeMap;

fn identity(value: u128) -> String {
    format!("{value:032x}")
}

fn authority() -> (WorkbenchAuthority, WorkbenchGrant) {
    let site_id = SiteId::parse(&identity(100)).unwrap();
    (
        WorkbenchAuthority::new(site_id.clone()).unwrap(),
        WorkbenchGrant::manage_navigation_and_terminal_create(site_id),
    )
}

fn binding(value: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&identity(value)).unwrap(),
        worker_id: WorkerId::parse(&identity(value + 1000)).unwrap(),
    }
}

fn geometry(columns: u16) -> WorkbenchTerminalGeometry {
    WorkbenchTerminalGeometry::new(columns, 24, 8, 16).unwrap()
}

fn command(
    request: u128,
    revision: u64,
    context: WorkbenchTerminalContextId,
    label: &str,
    geometry: WorkbenchTerminalGeometry,
) -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(request)).unwrap(),
        site_id: SiteId::parse(&identity(100)).unwrap(),
        expected_revision: WorkbenchRevision::new(revision),
        action: WorkbenchAction::TabCreate {
            terminal_context_id: context,
            label: WorkbenchLabel::parse(label).unwrap(),
            geometry,
        },
    }
}

fn tab_command(
    authority: &WorkbenchAuthority,
    request: u128,
    revision: u64,
    label: &str,
) -> WorkbenchCommand {
    command(
        request,
        revision,
        authority
            .projection
            .snapshot()
            .navigation
            .standalone_terminal_context_id
            .clone(),
        label,
        geometry(80),
    )
}

fn commit(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
    binding: PresentationBinding,
) -> (
    PendingWorkbenchTabCreate,
    CommittedWorkbenchTabCreateMetadata,
) {
    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(grant), command)
    else {
        panic!("expected admission")
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding) else {
        panic!("expected prepared command")
    };
    let committed = prepared.commit();
    let metadata = committed.metadata().clone();
    (committed.into_pending(), metadata)
}

fn install_revision(authority: &mut WorkbenchAuthority, revision: u64) {
    let mut snapshot = authority.projection.snapshot().clone();
    snapshot.revision = WorkbenchRevision::new(revision);
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
}

fn assert_retained_rejection(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
    reason: WorkbenchRejectionReason,
) {
    let before = authority.projection.snapshot().clone();
    let cursor = authority.last_identity;
    let ledger_len = authority.ledger.len();
    let ledger_bytes = authority.ledger_bytes;
    let WorkbenchTabCreateBegin::Ready(first) =
        authority.begin_terminal_tab_create(Some(grant), command.clone())
    else {
        panic!("rejected command was admitted")
    };
    assert_eq!(
        first.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: before.revision,
            reason,
        }
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(authority.ledger.len(), ledger_len + 1);
    assert!(authority.ledger_bytes > ledger_bytes);
    assert!(!authority.retired);

    let retained_bytes = authority.ledger_bytes;
    let WorkbenchTabCreateBegin::Ready(replay) =
        authority.begin_terminal_tab_create(Some(grant), command)
    else {
        panic!("retained rejection replay was admitted")
    };
    assert_eq!(replay, first);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(authority.ledger.len(), ledger_len + 1);
    assert_eq!(authority.ledger_bytes, retained_bytes);
}

#[test]
fn site_revision_and_context_preflight_precedence_is_retained_and_atomic() {
    let (mut authority, grant) = authority();
    let wrong_context = WorkbenchTerminalContextId::parse(&identity(900)).unwrap();

    let mut wrong_site = command(1, 99, wrong_context.clone(), "one", geometry(80));
    wrong_site.site_id = SiteId::parse(&identity(101)).unwrap();
    assert_retained_rejection(
        &mut authority,
        &grant,
        wrong_site,
        WorkbenchRejectionReason::SiteMismatch,
    );

    assert_retained_rejection(
        &mut authority,
        &grant,
        command(2, 99, wrong_context.clone(), "two", geometry(80)),
        WorkbenchRejectionReason::RevisionMismatch,
    );

    assert_retained_rejection(
        &mut authority,
        &grant,
        command(3, 0, wrong_context, "three", geometry(80)),
        WorkbenchRejectionReason::TerminalContextUnavailable,
    );
}

#[test]
fn simple_tab_capacity_precedes_identity_and_revision_exhaustion_atomically() {
    let (mut authority, grant) = authority();
    let mut snapshot = authority.projection.snapshot().clone();
    let context = &mut snapshot.terminal_contexts[0];
    for index in 0..MAXIMUM_TABS_PER_CONTEXT {
        let offset = u128::try_from(index).unwrap();
        append_simple_tab(
            context,
            &parse_tab_ids(4 + offset * 3).unwrap(),
            "tab",
            binding(10_000 + offset),
        );
    }
    snapshot.revision = WorkbenchRevision::new(u64::MAX);
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    authority.last_identity = u128::MAX;
    let request = tab_command(&authority, 10, u64::MAX, "full");

    assert_retained_rejection(
        &mut authority,
        &grant,
        request,
        WorkbenchRejectionReason::ResourceLimit,
    );
}

#[test]
fn identity_allocation_precedes_revision_and_both_exhaustion_boundaries_are_atomic() {
    let (mut identity_limited, identity_grant) = authority();
    install_revision(&mut identity_limited, u64::MAX);
    identity_limited.last_identity = u128::MAX - 2;
    let identity_request = tab_command(&identity_limited, 20, u64::MAX, "identity");
    assert_retained_rejection(
        &mut identity_limited,
        &identity_grant,
        identity_request,
        WorkbenchRejectionReason::IdentityExhausted,
    );

    let (mut revision_limited, revision_grant) = authority();
    install_revision(&mut revision_limited, u64::MAX);
    revision_limited.last_identity = u128::MAX - 3;
    let revision_request = tab_command(&revision_limited, 21, u64::MAX, "revision");
    assert_retained_rejection(
        &mut revision_limited,
        &revision_grant,
        revision_request,
        WorkbenchRejectionReason::RevisionExhausted,
    );
}

#[test]
fn commit_publishes_exact_simple_tab_and_pending_then_final_replay_is_fixed() {
    let (mut authority, grant) = authority();
    let request = tab_command(&authority, 1, 0, " exact label ");
    let selected = binding(20);
    let (pending, metadata) = commit(&mut authority, &grant, request.clone(), selected.clone());

    let snapshot = authority.projection.snapshot();
    assert_eq!(snapshot.revision, WorkbenchRevision::new(1));
    let tab = &snapshot.terminal_contexts[0].tabs[0];
    assert_eq!(tab.label, " exact label ");
    assert!(!tab.pinned);
    assert_eq!(tab.group_id, None);
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        child_presentations,
    } = &tab.root
    else {
        panic!("expected simple panel")
    };
    assert_eq!(panel_id, metadata.panel_id());
    assert_eq!(primary_presentation.label, " exact label ");
    assert_eq!(primary_presentation.binding, Some(selected));
    assert!(child_presentations.is_empty());

    let WorkbenchTabCreateBegin::Ready(pending_replay) =
        authority.begin_terminal_tab_create(Some(&grant), request.clone())
    else {
        panic!("expected pending replay")
    };
    assert!(matches!(
        pending_replay.outcome,
        WorkbenchCommandOutcome::TabCreatePending { revision, .. }
            if revision == WorkbenchRevision::new(1)
    ));

    let started = WorkbenchTerminalOutcome::Started {
        geometry: geometry(80),
    };
    let prepared = authority
        .prepare_terminal_tab_settlement(pending, started.clone())
        .unwrap();
    assert!(matches!(
        prepared.result().outcome,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
    let final_result = prepared.commit();
    let WorkbenchTabCreateBegin::Ready(final_replay) =
        authority.begin_terminal_tab_create(Some(&grant), request)
    else {
        panic!("expected final replay")
    };
    assert_eq!(final_replay, final_result);
    assert_eq!(authority.projection.snapshot().revision.value(), 1);
}

#[test]
fn exact_pending_replay_and_every_semantic_difference_conflict_without_mutation() {
    let (mut authority, grant) = authority();
    let request = tab_command(&authority, 1, 0, "one");
    let (_pending, _metadata) = commit(&mut authority, &grant, request.clone(), binding(20));
    let before = authority.projection.snapshot().clone();
    let old_bytes = authority.ledger_bytes;

    let mut differences = Vec::new();
    let mut changed = request.clone();
    changed.expected_revision = WorkbenchRevision::new(1);
    differences.push(changed);
    let mut changed = request.clone();
    changed.site_id = SiteId::parse(&identity(101)).unwrap();
    differences.push(changed);
    let mut changed = request.clone();
    if let WorkbenchAction::TabCreate { label, .. } = &mut changed.action {
        *label = WorkbenchLabel::parse("two").unwrap();
    }
    differences.push(changed);
    let mut changed = request.clone();
    if let WorkbenchAction::TabCreate {
        geometry: changed_geometry,
        ..
    } = &mut changed.action
    {
        *changed_geometry = geometry(81);
    }
    differences.push(changed);
    let mut changed = request;
    if let WorkbenchAction::TabCreate {
        terminal_context_id,
        ..
    } = &mut changed.action
    {
        *terminal_context_id = WorkbenchTerminalContextId::parse(&identity(900)).unwrap();
    }
    differences.push(changed);

    for different in differences {
        let WorkbenchTabCreateBegin::Ready(result) =
            authority.begin_terminal_tab_create(Some(&grant), different)
        else {
            panic!("conflicting identity was admitted")
        };
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                revision,
            } if revision == WorkbenchRevision::new(1)
        ));
        assert_eq!(authority.projection.snapshot(), &before);
        assert_eq!(authority.ledger_bytes, old_bytes);
    }
}

#[test]
fn full_entry_ledger_still_replays_and_conflicts_before_refusing_a_new_tab_create() {
    let (mut authority, grant) = authority();
    let original = tab_command(&authority, 1, 0, "one");
    let (_pending, _metadata) = commit(&mut authority, &grant, original.clone(), binding(20));

    for request in 2..=u128::try_from(MAXIMUM_LEDGER_ENTRIES).unwrap() {
        let filler = tab_command(&authority, request, 0, "filler");
        let response = result(
            &filler,
            authority.rejected(WorkbenchRejectionReason::RevisionMismatch),
        );
        let charged_bytes = ledger_size(&filler, &response).unwrap();
        authority.ledger_bytes += charged_bytes;
        authority.ledger.insert(
            filler.request_id.clone(),
            RetainedRequest::Settled(Box::new(SettledRequest {
                command: filler,
                result: response,
            })),
        );
    }
    assert_eq!(authority.ledger.len(), MAXIMUM_LEDGER_ENTRIES);
    assert!(authority.ledger_bytes < MAXIMUM_LEDGER_BYTES);
    let before = authority.projection.snapshot().clone();
    let cursor = authority.last_identity;
    let ledger_bytes = authority.ledger_bytes;

    let WorkbenchTabCreateBegin::Ready(replay) =
        authority.begin_terminal_tab_create(Some(&grant), original.clone())
    else {
        panic!("exact pending replay was blocked by ledger count")
    };
    assert!(matches!(
        replay.outcome,
        WorkbenchCommandOutcome::TabCreatePending { .. }
    ));

    let mut conflict = original;
    let WorkbenchAction::TabCreate { label, .. } = &mut conflict.action else {
        unreachable!()
    };
    *label = WorkbenchLabel::parse("different").unwrap();
    let WorkbenchTabCreateBegin::Ready(conflict_result) =
        authority.begin_terminal_tab_create(Some(&grant), conflict)
    else {
        panic!("same-identity conflict was blocked by ledger count")
    };
    assert!(matches!(
        conflict_result.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));

    let mut new_request = command(
        10_000,
        99,
        WorkbenchTerminalContextId::parse(&identity(900)).unwrap(),
        "new",
        geometry(80),
    );
    new_request.site_id = SiteId::parse(&identity(101)).unwrap();
    let WorkbenchTabCreateBegin::Ready(full) =
        authority.begin_terminal_tab_create(Some(&grant), new_request)
    else {
        panic!("new command was admitted at the ledger entry limit")
    };
    assert_eq!(
        full.outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(authority.ledger.len(), MAXIMUM_LEDGER_ENTRIES);
    assert_eq!(authority.ledger_bytes, ledger_bytes);
}

#[test]
fn full_byte_ledger_refuses_pending_reservation_without_committing_membership() {
    let (mut authority, grant) = authority();
    let request = tab_command(&authority, 1, 0, "one");
    let before = authority.projection.snapshot().clone();
    let cursor = authority.last_identity;
    // Boundary injection: pending size is known only after the runtime binding is supplied.
    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&grant), request)
    else {
        panic!("byte capacity was interpreted before pending-result prebuild")
    };
    let WorkbenchTabCreatePreparation::Ready(full) = admission.finish(binding(20)) else {
        panic!("full byte ledger prepared a membership commit")
    };
    assert_eq!(
        full.outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert!(authority.ledger.is_empty());
    assert_eq!(authority.ledger_bytes, MAXIMUM_LEDGER_BYTES);
    assert!(!authority.retired);
}

#[test]
fn combined_grant_precedes_retirement_and_settlement_remains_valid_after_retirement() {
    let (mut authority, grant) = authority();
    let request = tab_command(&authority, 1, 0, "one");
    let (pending, _metadata) = commit(&mut authority, &grant, request.clone(), binding(20));
    authority.retire_standalone_composition();
    let navigation_only = WorkbenchGrant::manage_navigation(authority.site_id().clone());

    let WorkbenchTabCreateBegin::Ready(denied) =
        authority.begin_terminal_tab_create(Some(&navigation_only), request.clone())
    else {
        panic!("navigation grant admitted terminal creation")
    };
    assert_eq!(denied.outcome, WorkbenchCommandOutcome::Denied {});
    let WorkbenchTabCreateBegin::Ready(unavailable_result) =
        authority.begin_terminal_tab_create(Some(&grant), request.clone())
    else {
        panic!("retired authority admitted replay")
    };
    assert_eq!(
        unavailable_result.outcome,
        unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
    );

    let final_result = authority
        .prepare_terminal_tab_settlement(pending, WorkbenchTerminalOutcome::OutcomeUnknown {})
        .unwrap()
        .commit();
    assert!(matches!(
        final_result.outcome,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
            ..
        }
    ));
    let WorkbenchTabCreateBegin::Ready(still_retired) =
        authority.begin_terminal_tab_create(Some(&grant), request)
    else {
        panic!("retirement was cleared")
    };
    assert_eq!(
        still_retired.outcome,
        unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
    );
}

#[test]
fn foreign_same_state_token_and_wrong_started_geometry_are_rejected_before_mutation() {
    let (mut first, first_grant) = authority();
    let (mut second, second_grant) = authority();
    let first_request = tab_command(&first, 1, 0, "same");
    let second_request = tab_command(&second, 1, 0, "same");
    let (first_pending, _first_metadata) =
        commit(&mut first, &first_grant, first_request, binding(20));
    let (second_pending, _second_metadata) =
        commit(&mut second, &second_grant, second_request, binding(20));
    let before = second.projection.snapshot().clone();
    let old_bytes = second.ledger_bytes;
    assert!(
        second
            .prepare_terminal_tab_settlement(
                first_pending,
                WorkbenchTerminalOutcome::OutcomeUnknown {},
            )
            .is_err()
    );
    assert_eq!(second.projection.snapshot(), &before);
    assert_eq!(second.ledger_bytes, old_bytes);

    assert!(
        second
            .prepare_terminal_tab_settlement(
                second_pending,
                WorkbenchTerminalOutcome::Started {
                    geometry: geometry(81),
                },
            )
            .is_err()
    );
    assert_eq!(second.projection.snapshot(), &before);
    assert_eq!(second.ledger_bytes, old_bytes);
}

#[test]
fn stale_duplicate_wrong_seal_and_changed_metadata_tokens_cannot_settle_an_entry() {
    let (mut authority, grant) = authority();
    let request = tab_command(&authority, 1, 0, "one");
    let (pending, _metadata) = commit(&mut authority, &grant, request, binding(20));
    let duplicate = PendingWorkbenchTabCreate {
        metadata: pending.metadata.clone(),
        seal: Arc::clone(&pending.seal),
    };
    let wrong_seal = PendingWorkbenchTabCreate {
        metadata: pending.metadata.clone(),
        seal: Arc::new(SettlementSeal),
    };
    let mut changed_metadata = pending.metadata.clone();
    changed_metadata.first = !changed_metadata.first;
    let changed = PendingWorkbenchTabCreate {
        metadata: changed_metadata,
        seal: Arc::clone(&pending.seal),
    };
    let before = authority.projection.snapshot().clone();
    let old_bytes = authority.ledger_bytes;
    for invalid in [wrong_seal, changed] {
        assert!(
            authority
                .prepare_terminal_tab_settlement(
                    invalid,
                    WorkbenchTerminalOutcome::OutcomeUnknown {},
                )
                .is_err()
        );
        assert_eq!(authority.projection.snapshot(), &before);
        assert_eq!(authority.ledger_bytes, old_bytes);
    }

    let _final_result = authority
        .prepare_terminal_tab_settlement(pending, WorkbenchTerminalOutcome::OutcomeUnknown {})
        .unwrap()
        .commit();
    let settled = authority.projection.snapshot().clone();
    let settled_bytes = authority.ledger_bytes;
    assert!(
        authority
            .prepare_terminal_tab_settlement(
                duplicate,
                WorkbenchTerminalOutcome::OutcomeUnknown {},
            )
            .is_err()
    );
    assert_eq!(authority.projection.snapshot(), &settled);
    assert_eq!(authority.ledger_bytes, settled_bytes);
}

#[test]
fn two_pending_entries_settle_out_of_order_without_changing_membership_revision() {
    let (mut authority, grant) = authority();
    let first_request = tab_command(&authority, 1, 0, "one");
    let (first, first_metadata) = commit(&mut authority, &grant, first_request, binding(20));
    let second_request = tab_command(&authority, 2, 1, "two");
    let (second, second_metadata) = commit(&mut authority, &grant, second_request, binding(21));
    assert!(first_metadata.is_first());
    assert!(!second_metadata.is_first());
    let before = authority.projection.snapshot().clone();

    let _second_result = authority
        .prepare_terminal_tab_settlement(second, WorkbenchTerminalOutcome::OutcomeUnknown {})
        .unwrap()
        .commit();
    let _first_result = authority
        .prepare_terminal_tab_settlement(
            first,
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            },
        )
        .unwrap()
        .commit();
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.projection.snapshot().revision.value(), 2);
}

#[test]
fn pending_reserves_the_largest_prebuilt_result_and_settlement_only_releases_bytes() {
    let (mut authority, grant) = authority();
    let request = tab_command(&authority, 1, 0, "one");
    let (pending, _metadata) = commit(&mut authority, &grant, request, binding(20));
    let RetainedRequest::PendingTabCreate(entry) = authority.ledger.values().next().unwrap() else {
        panic!("expected pending entry")
    };
    let maximum = entry
        .final_results
        .finals
        .iter()
        .map(|(_outcome, result)| result.charged_bytes)
        .chain(std::iter::once(entry.final_results.pending.charged_bytes))
        .max()
        .unwrap();
    assert_eq!(entry.reserved_bytes, maximum);
    let reserved_total = authority.ledger_bytes;
    let _final_result = authority
        .prepare_terminal_tab_settlement(
            pending,
            WorkbenchTerminalOutcome::Failed {
                reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
            },
        )
        .unwrap()
        .commit();
    assert!(authority.ledger_bytes <= reserved_total);
}

#[test]
fn every_closed_terminal_outcome_selects_its_exact_prebuilt_final_result() {
    let outcomes = [
        WorkbenchTerminalOutcome::Started {
            geometry: geometry(80),
        },
        WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        },
        WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::ServerBusy,
        },
        WorkbenchTerminalOutcome::Failed {
            reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
        },
        WorkbenchTerminalOutcome::OutcomeUnknown {},
    ];
    for (index, expected) in outcomes.into_iter().enumerate() {
        let (mut authority, grant) = authority();
        let request = tab_command(&authority, 1, 0, "one");
        let (pending, _metadata) = commit(&mut authority, &grant, request, binding(20));
        let final_result = authority
            .prepare_terminal_tab_settlement(pending, expected.clone())
            .unwrap()
            .commit();
        let WorkbenchCommandOutcome::TabCreated {
            terminal_outcome, ..
        } = final_result.outcome
        else {
            panic!("case {index} did not return tab-created")
        };
        assert_eq!(terminal_outcome, expected);
    }
}

#[test]
fn transient_runtime_refusal_is_unretained_and_composition_failure_only_retires_workbench() {
    for reason in [
        WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker,
        WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy,
    ] {
        let (mut authority, grant) = authority();
        let request = tab_command(&authority, 1, 0, "one");
        let before = authority.projection.snapshot().clone();
        let WorkbenchTabCreateBegin::Admitted(admission) =
            authority.begin_terminal_tab_create(Some(&grant), request)
        else {
            panic!("expected admission")
        };
        assert!(matches!(
            admission.runtime_unavailable(reason).outcome,
            WorkbenchCommandOutcome::Unavailable { .. }
        ));
        assert_eq!(authority.projection.snapshot(), &before);
        assert_eq!(authority.last_identity, 1);
        assert!(authority.ledger.is_empty());
        assert!(!authority.retired);
    }

    let (mut authority, grant) = authority();
    let request = tab_command(&authority, 1, 0, "one");
    let before = authority.projection.snapshot().clone();
    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&grant), request)
    else {
        panic!("expected admission")
    };
    assert_eq!(
        admission.retire_composition_failure().outcome,
        unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
    );
    assert!(authority.retired);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, 1);
    assert!(authority.ledger.is_empty());
}

#[test]
fn request_classification_routes_tab_create_without_authorization_or_state_interpretation() {
    let (mut authority, grant) = authority();
    let stale = tab_command(&authority, 1, 99, "stale");
    let before = authority.projection.snapshot().clone();
    let request = WorkbenchRequest::Command(stale);
    let routed = WorkbenchReadOrNavigationRequest::try_from(request.clone());
    let Err(routed) = routed else {
        panic!("tab.create entered the pure executor")
    };
    assert_eq!(routed, request);
    assert_eq!(authority.projection.snapshot(), &before);
    assert!(authority.ledger.is_empty());

    let snapshot_read = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(50)).unwrap(),
    });
    assert!(WorkbenchReadOrNavigationRequest::try_from(snapshot_read).is_ok());

    let project_id = WorkbenchProjectId::parse(&identity(60)).unwrap();
    let workspace_id = WorkbenchWorkspaceId::parse(&identity(61)).unwrap();
    let pure_actions = [
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("project").unwrap(),
        },
        WorkbenchAction::ProjectRename {
            project_id: project_id.clone(),
            label: WorkbenchLabel::parse("renamed project").unwrap(),
        },
        WorkbenchAction::WorkspaceCreate {
            project_id: project_id.clone(),
            label: WorkbenchLabel::parse("workspace").unwrap(),
        },
        WorkbenchAction::WorkspaceRename {
            project_id,
            workspace_id,
            label: WorkbenchLabel::parse("renamed workspace").unwrap(),
        },
        WorkbenchAction::TabRename {
            terminal_context_id: before.navigation.standalone_terminal_context_id,
            tab_id: WorkbenchTabId::parse(&identity(62)).unwrap(),
            label: WorkbenchLabel::parse("renamed tab").unwrap(),
        },
    ];
    for (offset, action) in pure_actions.into_iter().enumerate() {
        let pure = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&identity(100 + offset as u128)).unwrap(),
            site_id: authority.site_id().clone(),
            expected_revision: WorkbenchRevision::new(0),
            action,
        };
        assert!(
            WorkbenchReadOrNavigationRequest::try_from(WorkbenchRequest::Command(pure)).is_ok()
        );
    }

    let pure = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(2)).unwrap(),
        site_id: authority.site_id().clone(),
        expected_revision: WorkbenchRevision::new(0),
        action: WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("project").unwrap(),
        },
    };
    let classified =
        WorkbenchReadOrNavigationRequest::try_from(WorkbenchRequest::Command(pure)).unwrap();
    assert!(matches!(
        authority.execute(Some(&grant), classified),
        WorkbenchResponse::Command(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::ProjectCreated { .. },
            ..
        })
    ));
}

#[test]
fn dropped_admission_and_prepared_plan_leave_projection_cursor_and_ledger_unchanged() {
    let (mut authority, grant) = authority();
    let request = tab_command(&authority, 1, 0, "one");
    let before = authority.projection.snapshot().clone();
    let cursor = authority.last_identity;
    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&grant), request.clone())
    else {
        panic!("expected admission")
    };
    drop(admission);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert!(authority.ledger.is_empty());

    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&grant), request)
    else {
        panic!("expected admission")
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(20)) else {
        panic!("expected prepared")
    };
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert!(authority.ledger.is_empty());
}

#[derive(Clone, Debug)]
enum TraceStep {
    Create(u8),
    Replay(u8),
    Conflict(u8),
    Settle(u8),
    Capacity(u8),
    Retire,
}

#[test]
fn generated_pending_traces_have_stable_shrinkable_replay_and_accounting() {
    for seed in 1_u64..=48 {
        let mut random = seed;
        let mut steps = vec![TraceStep::Create(0), TraceStep::Replay(0)];
        steps.extend((0..32).map(|_| {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let selector = random.to_le_bytes()[3];
            match random % 5 {
                0 => TraceStep::Create(selector),
                1 => TraceStep::Replay(selector),
                2 => TraceStep::Conflict(selector),
                3 => TraceStep::Settle(selector),
                _ => TraceStep::Capacity(selector),
            }
        }));
        steps.extend([
            TraceStep::Retire,
            TraceStep::Replay(0),
            TraceStep::Settle(0),
        ]);
        if let Err(failure) = run_trace(&steps) {
            let reduced = shrink_trace(steps);
            panic!(
                "seed={seed}; first={failure}; reduced={reduced:?}; replay={:?}",
                run_trace(&reduced)
            );
        }
    }
}

fn shrink_trace(mut steps: Vec<TraceStep>) -> Vec<TraceStep> {
    let mut index = 0;
    while index < steps.len() {
        let mut candidate = steps.clone();
        candidate.remove(index);
        if run_trace(&candidate).is_err() {
            steps = candidate;
            index = 0;
        } else {
            index += 1;
        }
    }
    steps
}

fn run_trace(steps: &[TraceStep]) -> Result<(), String> {
    let (mut authority, grant) = authority();
    let mut next_request = 1_u128;
    let mut pending = BTreeMap::<u128, PendingWorkbenchTabCreate>::new();
    let mut history = BTreeMap::<u128, (WorkbenchCommand, Option<WorkbenchCommandResult>)>::new();
    let mut committed_tabs = 0_usize;
    for step in steps {
        let before = authority.projection.snapshot().clone();
        let old_cursor = authority.last_identity;
        let old_ledger_len = authority.ledger.len();
        match *step {
            TraceStep::Create(selector) => {
                let request_id = next_request;
                next_request += 1;
                let request = trace_command(&authority, request_id, selector);
                match authority.begin_terminal_tab_create(Some(&grant), request.clone()) {
                    WorkbenchTabCreateBegin::Ready(response) => require_trace(
                        authority.retired
                            && response.outcome
                                == unavailable(
                                    WorkbenchCommandUnavailableReason::ServiceUnavailable,
                                ),
                        "active new command was not admitted",
                    )?,
                    WorkbenchTabCreateBegin::Admitted(admission) => {
                        let WorkbenchTabCreatePreparation::Prepared(prepared) =
                            admission.finish(binding(10_000 + request_id))
                        else {
                            return Err("admitted command did not prepare".to_owned());
                        };
                        let committed = prepared.commit();
                        pending.insert(request_id, committed.into_pending());
                        history.insert(request_id, (request, None));
                        committed_tabs += 1;
                        require_trace(
                            authority.projection.snapshot().revision.value()
                                == before.revision.value() + 1
                                && authority.last_identity == old_cursor + 3
                                && authority.ledger.len() == old_ledger_len + 1,
                            "create did not commit exactly once",
                        )?;
                    }
                }
            }
            TraceStep::Replay(selector) => {
                if let Some(request_id) = selected_key(&history, selector) {
                    let (request, final_result) = history
                        .get(&request_id)
                        .ok_or("replay selection disappeared")?;
                    let WorkbenchTabCreateBegin::Ready(response) =
                        authority.begin_terminal_tab_create(Some(&grant), request.clone())
                    else {
                        return Err("exact replay was admitted twice".to_owned());
                    };
                    if authority.retired {
                        require_trace(
                            response.outcome
                                == unavailable(
                                    WorkbenchCommandUnavailableReason::ServiceUnavailable,
                                ),
                            "retirement did not fence replay",
                        )?;
                    } else if pending.contains_key(&request_id) {
                        require_trace(
                            matches!(
                                response.outcome,
                                WorkbenchCommandOutcome::TabCreatePending { .. }
                            ),
                            "pending replay did not return pending",
                        )?;
                    } else {
                        require_trace(
                            final_result.as_ref() == Some(&response),
                            "final replay changed its result",
                        )?;
                    }
                }
            }
            TraceStep::Conflict(selector) => {
                if let Some(request_id) = selected_key(&history, selector) {
                    let mut request = history
                        .get(&request_id)
                        .ok_or("conflict selection disappeared")?
                        .0
                        .clone();
                    if let WorkbenchAction::TabCreate { label, .. } = &mut request.action {
                        *label = WorkbenchLabel::parse("different").unwrap();
                    }
                    let WorkbenchTabCreateBegin::Ready(response) =
                        authority.begin_terminal_tab_create(Some(&grant), request)
                    else {
                        return Err("semantic conflict was admitted".to_owned());
                    };
                    require_trace(
                        authority.retired
                            || matches!(
                                response.outcome,
                                WorkbenchCommandOutcome::Rejected {
                                    reason: WorkbenchRejectionReason::RequestIdentityConflict,
                                    ..
                                }
                            ),
                        "semantic difference did not conflict",
                    )?;
                }
            }
            TraceStep::Settle(selector) => {
                if let Some(request_id) = selected_pending_key(&pending, selector) {
                    let token = pending
                        .remove(&request_id)
                        .ok_or("pending selection disappeared")?;
                    let outcome = trace_outcome(selector, token.metadata().geometry());
                    let final_result = authority
                        .prepare_terminal_tab_settlement(token, outcome)
                        .map_err(|_error| "valid settlement token was rejected")?
                        .commit();
                    history
                        .get_mut(&request_id)
                        .ok_or("settled history disappeared")?
                        .1 = Some(final_result);
                    require_trace(
                        authority.projection.snapshot() == &before
                            && authority.last_identity == old_cursor
                            && authority.ledger.len() == old_ledger_len,
                        "settlement changed membership or allocation",
                    )?;
                }
            }
            TraceStep::Capacity(selector) => {
                if !authority.retired {
                    let request_id = next_request;
                    next_request += 1;
                    let request = trace_command(&authority, request_id, selector);
                    let actual_bytes = authority.ledger_bytes;
                    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
                    let WorkbenchTabCreateBegin::Admitted(admission) =
                        authority.begin_terminal_tab_create(Some(&grant), request)
                    else {
                        return Err(
                            "byte boundary was rejected before final-table sizing".to_owned()
                        );
                    };
                    let WorkbenchTabCreatePreparation::Ready(response) =
                        admission.finish(binding(20_000 + request_id))
                    else {
                        return Err("full byte ledger prepared a commit".to_owned());
                    };
                    authority.ledger_bytes = actual_bytes;
                    require_trace(
                        response.outcome
                            == unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
                            && authority.projection.snapshot() == &before
                            && authority.last_identity == old_cursor
                            && authority.ledger.len() == old_ledger_len,
                        "capacity refusal changed authority",
                    )?;
                }
            }
            TraceStep::Retire => authority.retire_standalone_composition(),
        }
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(
            authority.projection.snapshot().clone(),
        ))
        .map_err(|error| error.to_string())?;
        orchbee_workbench_protocol::parse_snapshot(&encoded).map_err(|error| error.to_string())?;
        require_trace(
            authority.projection.snapshot().terminal_contexts[0]
                .tabs
                .len()
                == committed_tabs,
            "replay or settlement changed membership",
        )?;
        let accounted = authority
            .ledger
            .values()
            .map(|entry| match entry {
                RetainedRequest::Settled(entry) => {
                    ledger_size(&entry.command, &entry.result).unwrap()
                }
                RetainedRequest::PendingTabCreate(entry) => entry.reserved_bytes,
                RetainedRequest::TabClose(_) => panic!("Create trace generated Close"),
            })
            .sum::<usize>();
        require_trace(
            accounted == authority.ledger_bytes,
            "pending/settled byte accounting diverged",
        )?;
    }
    Ok(())
}

fn trace_command(
    authority: &WorkbenchAuthority,
    request_id: u128,
    selector: u8,
) -> WorkbenchCommand {
    command(
        request_id,
        authority.projection.snapshot().revision.value(),
        authority
            .projection
            .snapshot()
            .navigation
            .standalone_terminal_context_id
            .clone(),
        &format!("tab-{selector}"),
        geometry(80 + u16::from(selector % 3)),
    )
}

fn trace_outcome(
    selector: u8,
    exact_geometry: WorkbenchTerminalGeometry,
) -> WorkbenchTerminalOutcome {
    match selector % 5 {
        0 => WorkbenchTerminalOutcome::Started {
            geometry: exact_geometry,
        },
        1 => WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        },
        2 => WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::ServerBusy,
        },
        3 => WorkbenchTerminalOutcome::Failed {
            reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
        },
        _ => WorkbenchTerminalOutcome::OutcomeUnknown {},
    }
}

fn selected_key(
    history: &BTreeMap<u128, (WorkbenchCommand, Option<WorkbenchCommandResult>)>,
    selector: u8,
) -> Option<u128> {
    if history.is_empty() {
        None
    } else {
        history
            .keys()
            .nth(usize::from(selector) % history.len())
            .copied()
    }
}

fn selected_pending_key(
    pending: &BTreeMap<u128, PendingWorkbenchTabCreate>,
    selector: u8,
) -> Option<u128> {
    if pending.is_empty() {
        None
    } else {
        pending
            .keys()
            .nth(usize::from(selector) % pending.len())
            .copied()
    }
}

fn require_trace(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned())
    }
}
