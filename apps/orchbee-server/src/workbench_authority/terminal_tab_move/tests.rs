use super::*;
mod index;
mod metadata;
mod stateful;
mod workspace;
use crate::workbench_authority::terminal_membership::{append_simple_tab, parse_tab_ids};
use crate::workbench_authority::{
    MAXIMUM_LEDGER_BYTES, MAXIMUM_LEDGER_ENTRIES, WorkbenchReadOrNavigationRequest, identity,
};
use crate::workbench_projection::WorkbenchProjectionStore;
use orchbee_workbench_protocol::{
    PresentationBinding, SiteId, TerminalId, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchRevision, WorkerId,
};

fn authority(count: usize) -> (WorkbenchAuthority, WorkbenchGrant) {
    let mut authority = WorkbenchAuthority::new(SiteId::parse(&identity(100)).unwrap()).unwrap();
    let grant = WorkbenchGrant::manage_navigation(authority.site_id().clone());
    let mut snapshot = authority.projection.snapshot().clone();
    for index in 0..count {
        append_simple_tab(
            &mut snapshot.terminal_contexts[0],
            &parse_tab_ids(4 + 3 * index as u128).unwrap(),
            &format!("tab {index}"),
            false,
            PresentationBinding {
                terminal_id: TerminalId::parse(&identity(1000 + index as u128)).unwrap(),
                worker_id: WorkerId::parse(&identity(2000)).unwrap(),
            },
        );
    }
    snapshot.revision = WorkbenchRevision::new(count as u64);
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
    authority.last_identity = 1 + 3 * count as u128;
    (authority, grant)
}

fn command(
    authority: &WorkbenchAuthority,
    request: u128,
    index: usize,
    direction: WorkbenchTabMoveDirection,
) -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: orchbee_workbench_protocol::ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(request)).unwrap(),
        site_id: authority.site_id().clone(),
        expected_revision: authority.projection.snapshot().revision,
        action: WorkbenchAction::TabMove {
            terminal_context_id: authority
                .projection
                .snapshot()
                .navigation
                .standalone_terminal_context_id
                .clone(),
            tab_id: WorkbenchTabId::parse(&identity(2 + 3 * index as u128)).unwrap(),
            direction,
        },
    }
}

fn submit(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchCommandResult {
    match authority.begin_terminal_tab_move(Some(grant), command) {
        WorkbenchTabMoveBegin::Ready(result) => result,
        WorkbenchTabMoveBegin::Admitted(admission) => match admission.finish() {
            WorkbenchTabMovePreparation::Ready(result) => result,
            WorkbenchTabMovePreparation::Prepared(prepared) => prepared.commit(),
        },
    }
}

#[test]
fn four_directions_move_whole_tabs_and_preserve_every_non_order_fact() {
    use WorkbenchTabMoveDirection::{First, Last, Next, Previous};
    for (source, direction, order) in [
        (0, Previous, [0, 1, 2]),
        (0, Next, [1, 0, 2]),
        (0, First, [0, 1, 2]),
        (0, Last, [1, 2, 0]),
        (1, Previous, [1, 0, 2]),
        (1, Next, [0, 2, 1]),
        (1, First, [1, 0, 2]),
        (1, Last, [0, 2, 1]),
        (2, Previous, [0, 2, 1]),
        (2, Next, [0, 1, 2]),
        (2, First, [2, 0, 1]),
        (2, Last, [0, 1, 2]),
    ] {
        let (mut authority, grant) = authority(3);
        authority.last_identity = u128::MAX;
        let before = authority.projection.snapshot().clone();
        let request = command(&authority, 10, source, direction);
        assert_eq!(
            WorkbenchReadOrNavigationRequest::try_from(WorkbenchRequest::Command(request.clone()))
                .err(),
            Some(WorkbenchRequest::Command(request.clone()))
        );
        let result = submit(&mut authority, &grant, request.clone());
        let changed = order != [0, 1, 2];
        let mut expected = before.clone();
        expected.terminal_contexts[0].tabs = order
            .map(|index| before.terminal_contexts[0].tabs[index].clone())
            .to_vec();
        expected.revision = WorkbenchRevision::new(3 + u64::from(changed));
        assert_eq!(
            authority.projection.snapshot(),
            &expected,
            "{source} {direction:?}"
        );
        let expected_outcome = if changed {
            WorkbenchCommandOutcome::TabMoved {
                revision: expected.revision,
                terminal_context_id: before.navigation.standalone_terminal_context_id,
                tab_id: before.terminal_contexts[0].tabs[source].tab_id.clone(),
            }
        } else {
            WorkbenchCommandOutcome::Unchanged {
                revision: before.revision,
            }
        };
        assert_eq!(result.outcome, expected_outcome);
        assert_eq!(submit(&mut authority, &grant, request), result);
        assert_eq!(authority.last_identity, u128::MAX);
        assert_eq!(authority.ledger.len(), 1);
    }
}

#[test]
fn singleton_noop_precedes_revision_exhaustion_and_missing_tab_never_becomes_noop() {
    for direction in [
        WorkbenchTabMoveDirection::Previous,
        WorkbenchTabMoveDirection::Next,
        WorkbenchTabMoveDirection::First,
        WorkbenchTabMoveDirection::Last,
    ] {
        let (mut authority, grant) = authority(1);
        let mut snapshot = authority.projection.snapshot().clone();
        snapshot.revision = WorkbenchRevision::new(u64::MAX);
        authority.projection = WorkbenchProjectionStore::load(
            &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
        )
        .unwrap();
        let request = command(&authority, 10, 0, direction);
        assert_eq!(
            submit(&mut authority, &grant, request).outcome,
            WorkbenchCommandOutcome::Unchanged {
                revision: snapshot.revision
            }
        );
        let absent = command(&authority, 11, 8, direction);
        assert!(matches!(
            submit(&mut authority, &grant, absent).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::TabUnavailable,
                ..
            }
        ));
        assert_eq!(authority.projection.snapshot(), &snapshot);
    }
    let (mut authority, grant) = authority(2);
    let mut snapshot = authority.projection.snapshot().clone();
    snapshot.revision = WorkbenchRevision::new(u64::MAX);
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
    )
    .unwrap();
    let request = command(&authority, 10, 0, WorkbenchTabMoveDirection::Next);
    assert!(matches!(
        submit(&mut authority, &grant, request).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionExhausted,
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &snapshot);
}

#[test]
fn exact_move_replay_keeps_its_revision_and_all_fingerprint_fields_conflict_after_later_moves() {
    let (mut authority, grant) = authority(3);
    let first = command(&authority, 10, 0, WorkbenchTabMoveDirection::Next);
    let result = submit(&mut authority, &grant, first.clone());
    let second = command(&authority, 11, 2, WorkbenchTabMoveDirection::First);
    submit(&mut authority, &grant, second);
    let before = authority.projection.snapshot().clone();
    assert_eq!(submit(&mut authority, &grant, first.clone()), result);
    for field in 0..5 {
        let mut changed = first.clone();
        let WorkbenchAction::TabMove {
            terminal_context_id,
            tab_id,
            direction,
        } = &mut changed.action
        else {
            unreachable!()
        };
        match field {
            0 => changed.expected_revision = WorkbenchRevision::new(99),
            1 => changed.site_id = SiteId::parse(&identity(99)).unwrap(),
            2 => {
                *terminal_context_id =
                    orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&identity(99))
                        .unwrap()
            }
            3 => *tab_id = WorkbenchTabId::parse(&identity(99)).unwrap(),
            _ => *direction = WorkbenchTabMoveDirection::Last,
        }
        assert!(matches!(
            submit(&mut authority, &grant, changed).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
    }
    assert_eq!(authority.projection.snapshot(), &before);
    authority.retire_standalone_composition();
    assert!(
        matches!(authority.begin_terminal_tab_move(None, first.clone()), WorkbenchTabMoveBegin::Ready(result) if result.outcome == WorkbenchCommandOutcome::Denied {})
    );
    assert_eq!(
        submit(&mut authority, &grant, first).outcome,
        unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
    );
    assert_eq!(authority.projection.snapshot(), &before);
}

#[test]
fn dropping_move_admission_or_prepared_commit_preserves_projection_cursor_and_ledger() {
    let (mut authority, grant) = authority(3);
    let before = authority.projection.snapshot().clone();
    let request = command(&authority, 10, 0, WorkbenchTabMoveDirection::Last);
    let WorkbenchTabMoveBegin::Admitted(admission) =
        authority.begin_terminal_tab_move(Some(&grant), request.clone())
    else {
        panic!("admission expected")
    };
    drop(admission);
    let WorkbenchTabMoveBegin::Admitted(admission) =
        authority.begin_terminal_tab_move(Some(&grant), request.clone())
    else {
        panic!("admission expected")
    };
    let WorkbenchTabMovePreparation::Prepared(prepared) = admission.finish() else {
        panic!("prepared expected")
    };
    assert!(matches!(
        prepared.result().outcome,
        WorkbenchCommandOutcome::TabMoved { .. }
    ));
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, 10);
    assert_eq!(authority.ledger_bytes, 0);
    assert!(authority.ledger.is_empty());
    assert!(matches!(
        submit(&mut authority, &grant, request).outcome,
        WorkbenchCommandOutcome::TabMoved { .. }
    ));
}

#[test]
fn move_entry_and_byte_capacity_refusal_is_unretained_and_preserves_prepared_order() {
    let (mut authority, grant) = authority(3);
    let before = authority.projection.snapshot().clone();
    let request = command(&authority, 10, 0, WorkbenchTabMoveDirection::Last);
    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    assert_eq!(
        submit(&mut authority, &grant, request.clone()).outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert!(authority.ledger.is_empty());
    assert_eq!(authority.last_identity, 10);
    authority.ledger_bytes = 0;
    let retained = submit(&mut authority, &grant, request.clone());
    for index in 1..MAXIMUM_LEDGER_ENTRIES {
        let noop = command(
            &authority,
            100 + index as u128,
            0,
            WorkbenchTabMoveDirection::Last,
        );
        assert!(matches!(
            submit(&mut authority, &grant, noop).outcome,
            WorkbenchCommandOutcome::Unchanged { .. }
        ));
    }
    let before = authority.projection.snapshot().clone();
    assert_eq!(submit(&mut authority, &grant, request), retained);
    let fresh = command(&authority, 9000, 1, WorkbenchTabMoveDirection::Last);
    assert_eq!(
        submit(&mut authority, &grant, fresh).outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.ledger.len(), MAXIMUM_LEDGER_ENTRIES);
    assert_eq!(authority.last_identity, 10);
}

#[test]
fn context_and_full_layout_validation_precede_missing_tab_or_boundary_noop() {
    for valid_context in [false, true] {
        for target in [0, 9] {
            let (mut authority, grant) = authority(2);
            let mut snapshot = authority.projection.snapshot().clone();
            let orchbee_workbench_protocol::PanelNode::Panel {
                primary_presentation,
                ..
            } = &mut snapshot.terminal_contexts[0].tabs[0].root
            else {
                unreachable!()
            };
            primary_presentation.binding = None;
            authority.projection = WorkbenchProjectionStore::load(
                &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
                    .unwrap(),
            )
            .unwrap();
            let mut request = command(&authority, 10, target, WorkbenchTabMoveDirection::First);
            if !valid_context {
                let WorkbenchAction::TabMove {
                    terminal_context_id,
                    ..
                } = &mut request.action
                else {
                    unreachable!()
                };
                *terminal_context_id =
                    orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&identity(99))
                        .unwrap();
            }
            let result = submit(&mut authority, &grant, request);
            if valid_context {
                assert_eq!(
                    result.outcome,
                    unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
                );
                assert!(authority.retired);
                assert!(authority.ledger.is_empty());
            } else {
                assert!(matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::Rejected {
                        reason: WorkbenchRejectionReason::TerminalContextUnavailable,
                        ..
                    }
                ));
                assert!(!authority.retired);
            }
            assert_eq!(authority.projection.snapshot(), &snapshot);
            assert_eq!(authority.last_identity, 7);
        }
    }
}

#[test]
fn maximum_membership_move_and_exact_byte_reservation_boundaries_preserve_order_truthfully() {
    let (mut maximum, grant) = authority(orchbee_workbench_protocol::MAXIMUM_TABS_PER_CONTEXT);
    let before = maximum.projection.snapshot().clone();
    let request = command(&maximum, 10, 255, WorkbenchTabMoveDirection::First);
    assert!(
        matches!(submit(&mut maximum, &grant, request).outcome, WorkbenchCommandOutcome::TabMoved { revision, .. } if revision == WorkbenchRevision::new(257))
    );
    let mut expected = before.clone();
    expected.revision = WorkbenchRevision::new(257);
    let last = expected.terminal_contexts[0].tabs.pop().unwrap();
    expected.terminal_contexts[0].tabs.insert(0, last);
    assert_eq!(maximum.projection.snapshot(), &expected);
    assert_eq!(maximum.last_identity, 769);

    let (mut sample, grant) = authority(2);
    let request = command(&sample, 10, 0, WorkbenchTabMoveDirection::Last);
    let result = submit(&mut sample, &grant, request.clone());
    let charge = orchbee_workbench_protocol::serialize_workbench_request(
        &WorkbenchRequest::Command(request),
    )
    .unwrap()
    .len()
        + orchbee_workbench_protocol::serialize_workbench_response(
            &orchbee_workbench_protocol::WorkbenchResponse::Command(result),
        )
        .unwrap()
        .len();
    for available in [charge - 1, charge, charge + 1] {
        let (mut authority, grant) = authority(2);
        let before = authority.projection.snapshot().clone();
        authority.ledger_bytes = MAXIMUM_LEDGER_BYTES - available;
        let request = command(&authority, 10, 0, WorkbenchTabMoveDirection::Last);
        let result = submit(&mut authority, &grant, request);
        if available < charge {
            assert_eq!(
                result.outcome,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
            );
            assert_eq!(authority.projection.snapshot(), &before);
            assert_eq!(authority.ledger_bytes, MAXIMUM_LEDGER_BYTES - available);
            assert!(authority.ledger.is_empty());
        } else {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::TabMoved { .. }
            ));
            assert_eq!(
                authority.ledger_bytes,
                MAXIMUM_LEDGER_BYTES - available + charge
            );
            assert_eq!(authority.ledger.len(), 1);
        }
        assert_eq!(authority.last_identity, 7);
        assert!(!authority.retired);
    }
}

#[test]
fn move_revision_growth_at_the_exact_projection_byte_bound_refuses_before_publication() {
    use orchbee_workbench_protocol::{
        MAXIMUM_ENCODED_PROJECTION_BYTES, Project, WorkbenchProjectId, WorkbenchTerminalContextId,
        WorkbenchWorkspaceId, Workspace,
    };
    let (mut authority, grant) = authority(2);
    let mut snapshot = authority.projection.snapshot().clone();
    snapshot.revision = WorkbenchRevision::new(9);
    for index in 0_u128..256 {
        let context = WorkbenchTerminalContextId::parse(&identity(3000 + index)).unwrap();
        snapshot.navigation.projects.push(Project {
            directories: Vec::new(),
            project_id: WorkbenchProjectId::parse(&identity(4000 + index)).unwrap(),
            label: "1".to_owned(),
            workspaces: vec![Workspace {
                primary_directory_id: None,
                workspace_id: WorkbenchWorkspaceId::parse(&identity(5000 + index)).unwrap(),
                terminal_context_id: context.clone(),
                label: "1".to_owned(),
            }],
        });
        snapshot
            .terminal_contexts
            .push(crate::workbench_authority::empty_context(context));
    }
    let base = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
        .unwrap()
        .len();
    let mut remaining = MAXIMUM_ENCODED_PROJECTION_BYTES - base;
    for project in &mut snapshot.navigation.projects {
        for label in [&mut project.label, &mut project.workspaces[0].label] {
            let increment = remaining.min(2047);
            let bytes = increment + 1;
            *label = "\u{1f41d}".repeat(bytes / 4);
            label.push_str(match bytes % 4 {
                0 => "",
                1 => "a",
                2 => "\u{00e9}",
                _ => "\u{4e2d}",
            });
            remaining -= increment;
        }
    }
    assert_eq!(remaining, 0);
    let encoded =
        serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap();
    assert_eq!(encoded.len(), MAXIMUM_ENCODED_PROJECTION_BYTES);
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    let request = command(&authority, 10, 0, WorkbenchTabMoveDirection::Next);
    let rejected = submit(&mut authority, &grant, request.clone());
    assert_eq!(
        rejected.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(9),
            reason: WorkbenchRejectionReason::ResourceLimit
        }
    );
    assert_eq!(submit(&mut authority, &grant, request), rejected);
    assert_eq!(authority.projection.snapshot(), &snapshot);
    assert_eq!(authority.last_identity, 7);
    assert_eq!(authority.ledger.len(), 1);
    assert!(!authority.retired);
}

#[test]
fn move_admission_cannot_use_its_navigation_grant_to_replay_a_terminal_create() {
    let (mut authority, move_grant) = authority(0);
    let create_grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone());
    let mut create = command(&authority, 10, 0, WorkbenchTabMoveDirection::First);
    create.action = WorkbenchAction::TabCreate {
        automatic_label: false,
        terminal_context_id: authority
            .projection
            .snapshot()
            .navigation
            .standalone_terminal_context_id
            .clone(),
        label: orchbee_workbench_protocol::WorkbenchLabel::parse("retained create").unwrap(),
        geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(80, 24, 8, 16)
            .unwrap(),
        launch: None,
        profile: None,
        options: Default::default(),
    };
    let crate::workbench_authority::WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&create_grant), create.clone())
    else {
        panic!("create admission expected")
    };
    let crate::workbench_authority::WorkbenchTabCreatePreparation::Prepared(prepared) = admission
        .finish(PresentationBinding {
            terminal_id: TerminalId::parse(&identity(1000)).unwrap(),
            worker_id: WorkerId::parse(&identity(2000)).unwrap(),
        })
    else {
        panic!("create preparation expected")
    };
    let _pending = prepared.commit().into_pending();
    assert!(
        matches!(authority.begin_terminal_tab_move(Some(&move_grant), create), WorkbenchTabMoveBegin::Ready(result) if result.outcome == WorkbenchCommandOutcome::Denied {})
    );
}
