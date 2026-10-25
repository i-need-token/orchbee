use super::*;
use crate::test_support::WorkbenchFixture;

#[test]
fn close_publication_cleans_only_vacated_source_group_and_preserves_peer_empty_groups() {
    use orchbee_workbench_protocol::{
        TabGroup, TabGroupColor, WorkbenchProtocolDocument, WorkbenchTabGroupId, serialize_document,
    };
    let (mut authority, grant) = fixture(3);
    let group = |value, label: &str, color| TabGroup {
        group_id: WorkbenchTabGroupId::parse(&identity(value)).unwrap(),
        label: label.into(),
        color,
    };
    let source = group(91000, "source", TabGroupColor::Green);
    let other = group(91001, "survivor", TabGroupColor::Blue);
    let empty = group(91002, "already empty", TabGroupColor::Red);
    let peer = group(91003, "created during close", TabGroupColor::Purple);
    let mut snapshot = authority.projection.snapshot().clone();
    for tab in &mut snapshot.terminal_contexts[0].tabs[..2] {
        tab.group_id = Some(source.group_id.clone());
    }
    snapshot.terminal_contexts[0].tabs[2].group_id = Some(other.group_id.clone());
    snapshot.terminal_contexts[0].groups = vec![source.clone(), other.clone(), empty.clone()];
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
    let now = Instant::now();
    for index in 0..2 {
        let context = &authority.projection.snapshot().terminal_contexts[0];
        let target = context.tabs[0].tab_id.clone();
        // The legacy flat membership helper excludes group metadata. Address the canonical
        // Tab through the same explicit context/ID contract used by the native menu.
        let close = command(
            &authority,
            10000 + index,
            WorkbenchAction::TabClose {
                terminal_context_id: context.terminal_context_id.clone(),
                tab_id: target.clone(),
            },
        );
        accept(&mut authority, &grant, close.clone(), now);
        dispose_and_cleanup(&mut authority, &close.request_id, now);
        if index == 1 {
            // Publication acts on the latest document, which may contain an unrelated group
            // created after admission. The close owns only its vacated source group.
            let mut latest = authority.projection.snapshot().clone();
            latest.revision = WorkbenchRevision::new(latest.revision.value() + 1);
            latest.terminal_contexts[0].groups.push(peer.clone());
            authority.projection = WorkbenchProjectionStore::load(
                &serialize_document(&WorkbenchProtocolDocument::Snapshot(latest)).unwrap(),
            )
            .unwrap();
        }
        let mut expected = authority.projection.snapshot().clone();
        expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
        expected.terminal_contexts[0]
            .tabs
            .retain(|tab| tab.tab_id != target);
        if index == 1 {
            expected.terminal_contexts[0].groups.remove(0);
        }
        let _closed = authority
            .prepare_terminal_tab_close_publication(&close.request_id, now)
            .unwrap()
            .commit();
        assert_eq!(
            authority.projection.snapshot(),
            &expected,
            "surviving identities and metadata are exact"
        );
    }
    assert_eq!(
        authority.projection.snapshot().terminal_contexts[0].groups,
        vec![other, empty, peer]
    );
}

#[test]
fn finalization_removes_only_current_exact_member_after_create_move_and_rename() {
    let (mut authority, grant) = fixture(2);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    let accepted = accept(&mut authority, &grant, close.clone(), now);
    let admission_revision = authority.projection.snapshot().revision;
    dispose_and_cleanup(&mut authority, &close.request_id, now);
    let create = create_command(&authority, 10001, "C");
    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&grant), create)
    else {
        panic!()
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(702)) else {
        panic!()
    };
    let _pending = prepared.commit().into_pending();
    let membership = authority.fixture_membership().unwrap();
    let move_command = command(
        &authority,
        10002,
        WorkbenchAction::TabMove {
            terminal_context_id: membership.terminal_context_id().clone(),
            tab_id: membership.members()[1].tab_id().clone(),
            direction: WorkbenchTabMoveDirection::Last,
        },
    );
    let WorkbenchTabMoveBegin::Admitted(admission) =
        authority.begin_terminal_tab_move(Some(&grant), move_command)
    else {
        panic!()
    };
    let WorkbenchTabMovePreparation::Prepared(prepared) = admission.finish() else {
        panic!()
    };
    let _moved = prepared.commit();
    let rename = command(
        &authority,
        10003,
        WorkbenchAction::TabRename {
            terminal_context_id: membership.terminal_context_id().clone(),
            tab_id: membership.members()[1].tab_id().clone(),
            label: WorkbenchLabel::parse("changed B").unwrap(),
        },
    );
    submit(&mut authority, &grant, rename);
    let before = authority.projection.snapshot().clone();
    let bytes = authority.ledger_bytes;
    let entries = authority.ledger.len();
    let prepared = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap();
    assert_eq!(prepared.before_membership().members().len(), 3);
    assert_eq!(prepared.after_membership().members().len(), 2);
    let removed = prepared.exact_removed_members().first().unwrap().clone();
    assert_eq!(
        prepared.after_membership().members(),
        prepared
            .before_membership()
            .members()
            .iter()
            .filter(|member| *member != &removed)
            .cloned()
            .collect::<Vec<_>>()
    );
    assert!(prepared.revision() > admission_revision);
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
    let prepared = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap();
    let closed_revision = prepared.revision();
    let expected_status = prepared.status().clone();
    assert_eq!(prepared.commit(), expected_status);
    let mut expected = before;
    expected.revision = closed_revision;
    expected.terminal_contexts[0]
        .tabs
        .retain(|tab| &tab.tab_id != removed.tab_id());
    assert_eq!(authority.projection.snapshot(), &expected);
    assert_eq!(authority.ledger_bytes, bytes);
    assert_eq!(authority.ledger.len(), entries);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(0));
    assert!(authority.has_terminal_tab_close(removed.binding()));
    assert!(!authority.has_terminal_tab_close(&binding(701)));
    assert!(
        matches!(authority.begin_terminal_tab_close(Some(&grant), close.clone()),
        WorkbenchTabCloseBegin::Ready(response) if response == accepted)
    );
    assert!(
        matches!(status(&authority, &grant, &close), WorkbenchTabCloseReadOutcome::Available {
        status: WorkbenchTabCloseStatus::Closed { revision }, .. } if revision == closed_revision)
    );
    let mut removed_new = close.clone();
    removed_new.request_id = request_id(10004);
    removed_new.expected_revision = closed_revision;
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), removed_new)
    else {
        panic!()
    };
    assert!(matches!(
        admission.resolve_after_composition(),
        WorkbenchTabCloseResolution::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::TabUnavailable,
                ..
            },
            ..
        })
    ));
    authority.retire_terminal_membership();
    assert!(matches!(
        status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));
}

#[test]
fn publication_rechecks_deadline_and_retirement_preserves_disposal_membership_and_charge() {
    for retire in [false, true] {
        let (mut authority, grant) = fixture(1);
        let now = Instant::now();
        let close = close_command(&authority, 10000, 0);
        accept(&mut authority, &grant, close.clone(), now);
        dispose_and_cleanup(&mut authority, &close.request_id, now);
        let before = authority.projection.snapshot().clone();
        let bytes = authority.ledger_bytes;
        if retire {
            authority.retire_terminal_membership();
        }
        let result = authority.prepare_terminal_tab_close_publication(
            &close.request_id,
            if retire {
                now
            } else {
                now + Duration::from_secs(30)
            },
        );
        assert!(matches!(
            result,
            Err(WorkbenchTabClosePublicationError::AuthorityRetired
                | WorkbenchTabClosePublicationError::NotReady)
        ));
        assert_eq!(authority.projection.snapshot(), &before);
        assert_eq!(authority.ledger_bytes, bytes);
        assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
        assert!(matches!(
            status(&authority, &grant, &close),
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. },
                ..
            }
        ));
    }
}

#[test]
fn prepared_joint_site_failure_retains_disposal_and_never_removes_or_releases_credit() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let close = close_command(&authority, 10000, 0);
    accept(&mut authority, &grant, close.clone(), now);
    dispose_and_cleanup(&mut authority, &close.request_id, now);
    let before = authority.projection.snapshot().clone();
    authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap()
        .retire_composition_failure();
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
    assert!(matches!(
        status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason:
                    orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::AuthorityRetired,
                resumable: false
            },
            ..
        }
    ));
}

#[test]
fn foreign_runtime_and_step_origin_cannot_settle_another_close() {
    let (mut authority, grant) = fixture(2);
    let now = Instant::now();
    let first = close_command(&authority, 10000, 0);
    let second = close_command(&authority, 10001, 1);
    accept(&mut authority, &grant, first.clone(), now);
    accept(&mut authority, &grant, second.clone(), now);
    let mut request = step_request(&mut authority, &first.request_id, now);
    request.authorization.worker_generation = WorkerGeneration::new(NonZeroU64::new(2).unwrap());
    assert!(
        authority
            .reserve_terminal_tab_close_step(&first.request_id, now, request)
            .is_err()
    );
    let request = step_request(&mut authority, &first.request_id, now);
    let token = authority
        .reserve_terminal_tab_close_step(&first.request_id, now, request)
        .unwrap();
    assert!(
        authority
            .confirm_terminal_tab_close_step_admitted(&second.request_id, &token, now)
            .is_err()
    );
    authority
        .confirm_terminal_tab_close_step_admitted(&first.request_id, &token, now)
        .unwrap();
    authority
        .settle_terminal_tab_close_step(
            &first.request_id,
            token,
            now,
            CloseStepObservation::Result(
                &TerminalLifecycleResult::new(request, TerminalLifecycleOutcome::Disposed).unwrap(),
            ),
        )
        .unwrap();
    let second_target = authority
        .terminal_tab_close_target(&second.request_id)
        .unwrap()
        .runtime()
        .expect("created Host runtime");
    assert!(
        authority
            .complete_terminal_tab_close_target_cleanup(&first.request_id, second_target)
            .is_err()
    );
    assert!(
        authority
            .prepare_terminal_tab_close_publication(&first.request_id, now)
            .is_err()
    );
    assert_eq!(authority.outstanding_close_credits().ok(), Some(2));
}
