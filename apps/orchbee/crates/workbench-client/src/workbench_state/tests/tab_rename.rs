use super::super::rename_draft::RenameDraftEdit;
use super::super::tab_rename::{RenameOperation, RenamePreparation, RenameResult};
use super::*;

#[test]
fn automatic_rename_starts_empty_and_equal_text_is_a_commit() {
    let mut document = snapshot(2, 1, None);
    document.terminal_contexts[0].tabs[0].automatic_label = true;
    document.terminal_contexts[0].tabs[0].label = "old ordinal".into();
    let mut state = ready_with_cache(project_preferred_workspace(&document).unwrap());
    assert_eq!(state.view().tab(0).unwrap().tab_label(), "1");
    state.open_rename_draft().unwrap();
    let draft = state.rename_draft().unwrap();
    assert_eq!(draft.text(), "");
    assert_eq!(draft.target_label(), "1");
    state.cancel_rename_draft();
    let same_visible = rename_operation(&mut state, "1");
    assert!(
        same_visible
            .known_commit(WorkbenchRevision::new(2))
            .is_some()
    );
    assert!(matches!(
        same_visible
            .prepare(RenameResult::Unchanged(WorkbenchRevision::new(1)))
            .result(),
        RenamePreparation::Divergent { known: None }
    ));
    state.cancel_rename_draft();
    let same_raw = rename_operation(&mut state, "old ordinal");
    assert!(same_raw.known_commit(WorkbenchRevision::new(2)).is_some());
}

#[test]
fn automatic_rename_placeholder_tracks_the_tab_position_after_reordering() {
    let mut document = snapshot(2, 1, None);
    document.terminal_contexts[0].tabs[0].automatic_label = true;
    document.terminal_contexts[0].tabs[0].label = "stored ordinal".into();
    document.terminal_contexts[0].tabs.reverse();
    let cache = project_preferred_workspace(&document).unwrap();
    let second = &cache.tabs()[1];
    let selection = crate::workbench_projection::ClientMemberTarget {
        tab: second.tab_id().clone(),
        panel: second.panel_id().clone(),
        presentation: second.presentation_id().clone(),
    };
    let (mut state, _) =
        WorkbenchClientState::starting_at(cache, 1, geometry(80), Some(&selection)).unwrap();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(
        &key,
        FrameIdentity::new(1),
        controlling(),
        Some(geometry(80))
    ));

    state.open_rename_draft().unwrap();
    let draft = state.rename_draft().unwrap();
    assert!(draft.automatic_label());
    assert_eq!(draft.position(), 2);
    assert_eq!(draft.text(), "");
    assert_eq!(draft.target_label(), "2");
}

fn rename_operation(state: &mut WorkbenchClientState, text: &str) -> RenameOperation {
    let token = state.open_rename_draft().unwrap();
    state.edit_rename_draft(token, RenameDraftEdit::Clear);
    state.edit_rename_draft(token, RenameDraftEdit::Append(text));
    state.acknowledge_rename_draw(Some(token));
    RenameOperation::from_draft(
        state.plan_rename_draft(token).unwrap(),
        workbench_request_id(100),
        Instant::now() + OPERATION_TOTAL,
    )
    .unwrap()
}

#[test]
fn rename_operation_captures_exact_request_intent_and_original_deadline() {
    let mut state = ready(2);
    let operation = rename_operation(&mut state, " 雪e\u{301}\t ");
    let copy = operation.clone();
    state.cancel_rename_draft();
    assert_eq!(copy, operation);
    assert_eq!(operation.request().request_id(), &workbench_request_id(100));
    assert_eq!(operation.request_id(), &workbench_request_id(100));
    assert_eq!(
        operation.update(),
        &super::super::rename_draft::RenameUpdate::Tab(
            WorkbenchLabel::parse(" 雪e\u{301}\t ").unwrap()
        )
    );
    assert_eq!(operation.baseline(), state.cache());
    assert_eq!(
        Some(operation.target()),
        state.desired().map(|pane| pane.anchor())
    );
    assert_eq!(copy.deadline(), operation.deadline());
    assert_eq!(copy.draft_token(), operation.draft_token());
    let expected =
        crate::local_client::workbench::label_rename::WorkbenchLabelRenameRequest::try_from(
            orchbee_workbench_protocol::WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: workbench_request_id(100),
                session_id: state.cache().session_id().clone(),
                expected_revision: WorkbenchRevision::new(1),
                action: orchbee_workbench_protocol::WorkbenchAction::TabRename {
                    terminal_context_id: state.cache().terminal_context_id().unwrap().clone(),
                    tab_id: state.tab(0).unwrap().tab_id().clone(),
                    label: WorkbenchLabel::parse(" 雪e\u{301}\t ").unwrap(),
                },
            },
        )
        .unwrap();
    assert_eq!(operation.request(), &expected);
}

#[test]
fn panel_rename_uses_the_canonical_primary_override_set_and_clear_request() {
    use orchbee_workbench_protocol::{PanelLabelUpdate, WorkbenchAction, WorkbenchCommand};
    for text in ["  Review 雪  ", " \t "] {
        let mut state = ready(2);
        let panel = state.desired().unwrap().panel().clone();
        let token = state.open_panel_rename_draft().unwrap();
        assert!(
            state.plan_rename_draft(token).is_none(),
            "hidden drafts cannot submit"
        );
        state.acknowledge_rename_draw(Some(token));
        state.edit_rename_draft(token, RenameDraftEdit::Clear);
        state.edit_rename_draft(token, RenameDraftEdit::Append(text));
        assert!(state.rename_draft().unwrap().field_error().is_none());
        let intent = state.plan_rename_draft(token).unwrap();
        let operation =
            RenameOperation::from_draft(intent, workbench_request_id(100), Instant::now()).unwrap();
        let label = PanelLabelUpdate::parse(Some(text)).unwrap();
        let expected =
            crate::local_client::workbench::label_rename::WorkbenchLabelRenameRequest::try_from(
                WorkbenchCommand {
                    protocol_version: ProtocolVersion,
                    request_id: workbench_request_id(100),
                    session_id: state.cache().session_id().clone(),
                    expected_revision: state.cache().revision(),
                    action: WorkbenchAction::PanelRename {
                        terminal_context_id: state.cache().terminal_context_id().unwrap().clone(),
                        tab_id: state.tab(0).unwrap().tab_id().clone(),
                        panel_id: panel,
                        label,
                    },
                },
            )
            .unwrap();
        assert_eq!(operation.request(), &expected);
        state.cancel_rename_draft();
        assert!(state.plan_rename_draft(token).is_none());
        assert_eq!(
            state.cache(),
            operation.baseline(),
            "drafts never write canonical labels"
        );
    }
}

#[test]
fn rename_preparation_distinguishes_authoritative_noop_from_impossible_unchanged() {
    let mut state = ready(2);
    let original = state.tab(0).unwrap().tab_label().as_str().to_owned();
    let same = rename_operation(&mut state, &original);
    assert!(matches!(
        same.prepare(RenameResult::Unchanged(WorkbenchRevision::new(1)))
            .result(),
        RenamePreparation::Unchanged
    ));
    assert!(matches!(
        same.prepare(RenameResult::Unchanged(WorkbenchRevision::new(2)))
            .result(),
        RenamePreparation::Divergent { known: None }
    ));
    state.cancel_rename_draft();
    let changed = rename_operation(&mut state, "changed");
    assert!(matches!(
        changed
            .prepare(RenameResult::Unchanged(WorkbenchRevision::new(1)))
            .result(),
        RenamePreparation::Divergent { known: None }
    ));
    let mut maximum = ready_with_cache(projected(2, u64::MAX));
    let original = maximum.tab(0).unwrap().tab_label().as_str().to_owned();
    let same = rename_operation(&mut maximum, &original);
    assert!(matches!(
        same.prepare(RenameResult::Unchanged(WorkbenchRevision::new(u64::MAX)))
            .result(),
        RenamePreparation::Unchanged
    ));
}

#[test]
fn rename_known_revision_requires_changed_label_and_checked_successor() {
    let mut state = ready(2);
    let original = state.tab(0).unwrap().tab_label().as_str().to_owned();
    let same = rename_operation(&mut state, &original);
    assert!(same.known_commit(WorkbenchRevision::new(2)).is_none());
    state.cancel_rename_draft();
    let changed = rename_operation(&mut state, "changed");
    for wrong in [0, 1, 3, u64::MAX] {
        assert!(
            changed
                .known_commit(WorkbenchRevision::new(wrong))
                .is_none()
        );
    }
    let known = changed.known_commit(WorkbenchRevision::new(2)).unwrap();
    assert_eq!(known.operation(), &changed);
    assert_eq!(known.revision(), WorkbenchRevision::new(2));
    let mut malformed = snapshot(2, 2, None);
    malformed.terminal_contexts[0].tabs[0].label.clear();
    assert!(matches!(
        changed
            .prepare(RenameResult::Committed {
                revision: WorkbenchRevision::new(2),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: malformed
                }
            })
            .result(),
        RenamePreparation::Divergent { known: Some(_) }
    ));
    let mut maximum = ready_with_cache(projected(2, u64::MAX));
    let changed = rename_operation(&mut maximum, "changed");
    assert!(changed.known_commit(WorkbenchRevision::new(0)).is_none());
}

#[test]
fn rename_preparation_keeps_known_commit_when_exact_projection_is_invalid() {
    let mut state = ready(2);
    let operation = rename_operation(&mut state, " 雪 ");
    let mut candidate = snapshot(2, 2, None);
    candidate.terminal_contexts[0].tabs[0].label = " 雪 ".into();
    let prepared = operation.prepare(RenameResult::Committed {
        revision: WorkbenchRevision::new(2),
        outcome: WorkbenchSnapshotOutcome::Available {
            snapshot: candidate.clone(),
        },
    });
    assert_eq!(prepared.operation(), &operation);
    let RenamePreparation::Committed {
        known,
        projection: Some(projection),
    } = prepared.result()
    else {
        panic!("valid label-only snapshot")
    };
    assert_eq!(known.operation(), &operation);
    assert_eq!(projection.renamed_index, Some(0));
    candidate.terminal_contexts[0].tabs.swap(0, 1);
    let invalid = operation.prepare(RenameResult::Committed {
        revision: WorkbenchRevision::new(2),
        outcome: WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    });
    let RenamePreparation::Divergent { known: Some(known) } = invalid.result() else {
        panic!("invalid permutation retains known commit")
    };
    assert_eq!(known.operation(), &operation);
    assert_eq!(known.revision(), WorkbenchRevision::new(2));
}

#[test]
fn rename_preparation_allows_later_overwrite_removal_and_empty_without_cleanup_proof() {
    let mut state = ready(2);
    let operation = rename_operation(&mut state, "changed");
    for remaining in [2, 1, 0] {
        let mut candidate = snapshot(2, 3, None);
        if remaining == 2 {
            candidate.terminal_contexts[0].tabs[0].label = "overwritten later".into();
        } else {
            candidate.terminal_contexts[0].tabs.remove(0);
            if remaining == 0 {
                candidate.terminal_contexts[0].tabs.clear();
            }
        }
        let prepared = operation.prepare(RenameResult::Committed {
            revision: WorkbenchRevision::new(2),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: candidate,
            },
        });
        let RenamePreparation::Committed {
            known,
            projection: Some(projection),
        } = prepared.result()
        else {
            panic!("strict later membership")
        };
        assert_eq!(known.operation(), &operation);
        assert_eq!(projection.candidate.tabs().len(), remaining);
        assert_eq!(
            projection.renamed_index,
            if remaining == 2 { Some(0) } else { None }
        );
    }
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert!(state.invariant_holds());
}

#[test]
fn rename_preparation_separates_precommit_from_known_projection_unavailability() {
    use crate::local_client::workbench::label_rename::WorkbenchLabelRenamePrecommit;
    let mut state = ready(2);
    let operation = rename_operation(&mut state, "changed");
    assert!(matches!(
        operation.prepare(RenameResult::NotSubmitted).result(),
        RenamePreparation::NotSubmitted
    ));
    assert!(matches!(
        operation
            .prepare(RenameResult::Precommit(
                WorkbenchLabelRenamePrecommit::Denied
            ))
            .result(),
        RenamePreparation::Precommit(WorkbenchLabelRenamePrecommit::Denied)
    ));
    let prepared = operation.prepare(RenameResult::Committed {
        revision: WorkbenchRevision::new(2),
        outcome: WorkbenchSnapshotOutcome::Denied {},
    });
    let RenamePreparation::Committed {
        known,
        projection: None,
    } = prepared.result()
    else {
        panic!("known commit with unavailable projection")
    };
    assert_eq!(known.operation(), &operation);
    let (returned, _) = prepared.into_parts();
    assert_eq!(returned, operation);
    let unavailable = operation.prepare(RenameResult::Committed {
        revision: WorkbenchRevision::new(2),
        outcome: WorkbenchSnapshotOutcome::Unavailable {
            reason:
                orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    });
    assert!(matches!(
        unavailable.result(),
        RenamePreparation::Committed {
            known: _,
            projection: None
        }
    ));
}

#[test]
fn rename_latest_ui_known_receipt_survives_b_admission_and_replaces_before_fallible_install() {
    let mut state = ready(2);
    let first = rename_operation(&mut state, "A");
    assert!(state.begin_rename(first.clone()));
    let first_known = first.known_commit(WorkbenchRevision::new(2)).unwrap();
    assert!(state.retain_pending_rename(first_known.clone()));
    let mut snapshot = snapshot(2, 2, None);
    snapshot.terminal_contexts[0].tabs[0].label = "A".into();
    let candidate = project_preferred_workspace(&snapshot).unwrap();
    let candidate = state
        .prepare_membership_candidate(
            candidate,
            Some(first.target().tab_id()),
            Some(0),
            None,
            None,
        )
        .unwrap();
    state
        .prepare_rename_membership_install(
            first.request_id(),
            candidate,
            Some(first.target().clone()),
            None,
        )
        .unwrap()
        .commit();
    let second = rename_operation(&mut state, "B");
    let second = RenameOperation::from_draft(
        state.plan_rename_draft(second.draft_token()).unwrap(),
        workbench_request_id(101),
        second.deadline(),
    )
    .unwrap();
    assert!(state.begin_rename(second.clone()));
    let second_known = second.known_commit(WorkbenchRevision::new(3)).unwrap();
    // Worker-known B does not replace A before semantic UI consumption, and late A preserves B.
    assert_eq!(state.rename_receipts_for_test().1, Some(&first_known));
    assert!(state.retain_pending_rename(first_known.clone()));
    assert_eq!(state.pending_rename(), Some(&second));
    state
        .settle_rename_without_membership(&second, RenamePreparation::NotSubmitted)
        .unwrap();
    assert_eq!(state.rename_receipts_for_test().1, Some(&first_known));
    let second = rename_operation(&mut state, "B");
    let second = RenameOperation::from_draft(
        state.plan_rename_draft(second.draft_token()).unwrap(),
        workbench_request_id(102),
        second.deadline(),
    )
    .unwrap();
    assert!(state.begin_rename(second.clone()));
    assert!(!state.retain_pending_rename(second_known));
    let known = second.known_commit(WorkbenchRevision::new(3)).unwrap();
    assert!(state.retain_pending_rename(known.clone()));
    assert!(!state.retain_pending_rename(first_known));
    state
        .settle_rename_without_membership(
            &second,
            RenamePreparation::Committed {
                known: known.clone(),
                projection: None,
            },
        )
        .unwrap();
    assert_eq!(state.rename_receipts_for_test().1, Some(&known));
    assert_eq!(
        state.phase_kind(),
        ClientPhaseKind::RenameProjectionUnavailable
    );
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
    assert!(state.invariant_holds());
    state.retire(CarrierFailure::Deadline);
    assert_eq!(state.rename_receipts_for_test().1, Some(&known));
}
