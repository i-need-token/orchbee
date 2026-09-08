use super::*;

#[test]
fn empty_create_anchor_retains_known_commit_and_refusal_restores_no_viewer_origin() {
    for committed in [false, true] {
        let mut state = state(1);
        state.phase = SessionPhase::SelectedWithoutAttachment(state.active().unwrap().clone());
        let prepared = state
            .prepare_membership_candidate(projected(0, 2), None, None, None)
            .unwrap();
        state
            .prepare_membership_install(prepared, None, None)
            .unwrap()
            .commit();
        let operation = create_operation(&state, 4, 30, "1").with_origin(CreateOriginKind::Empty);
        let key = operation.key().clone();
        assert!(!state.begin_create(operation.clone().with_origin(CreateOriginKind::Ready)));
        assert!(state.begin_create(operation));
        assert!(state.invariant_holds());
        assert_eq!(state.active(), None);
        assert_eq!(state.controller_mode(), None);
        assert_eq!(state.desired(), None);
        assert!(!state.restore_precommit(&key, Some(geometry(80))));
        if committed {
            let candidate = snapshot(1, 3, Some("1"));
            let created = &candidate.terminal_contexts[0].tabs[0];
            let PanelNode::Panel {
                panel_id,
                primary_presentation,
                ..
            } = &created.root
            else {
                panic!("simple tab");
            };
            assert!(state.retain_create_commit(
                &key,
                WorkbenchTabCreateCommit::Pending {
                    revision: WorkbenchRevision::new(3),
                    terminal_context_id: state.cache().terminal_context_id().clone(),
                    tab_id: created.tab_id.clone(),
                    panel_id: panel_id.clone(),
                    presentation_id: primary_presentation.presentation_id.clone(),
                }
            ));
            assert!(state.invariant_holds());
            state.retire(CarrierFailure::OutcomeUnknown);
            assert!(state.invariant_holds());
            assert_eq!(
                state.committed_target().unwrap().operation().origin(),
                CreateOriginKind::Empty
            );
            assert_eq!(state.desired(), None);
        } else {
            assert!(state.restore_precommit(&key, None));
            assert_eq!(state.phase_kind(), SessionPhaseKind::Empty);
            assert!(state.invariant_holds());
        }
    }
}

#[test]
fn active_refresh_removal_prepares_membership_instead_of_authority_divergence() {
    let mut state = ready(1);
    assert!(state.start_pending_refresh(1));
    assert!(matches!(
        state.prepare_refresh(
            1,
            WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(0, 2, None)
            }
        ),
        PreparedRefresh::Membership { .. }
    ));
}

#[test]
fn staged_cleanup_rejects_wrong_identity_and_dropped_install_preserves_all_old_facts() {
    let mut state = ready(1);
    let old = state.desired().unwrap().clone();
    let baseline = state.cache().clone();
    let frame = state.accepted_frame.clone();
    assert!(state.start_pending_refresh(9));
    let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
        9,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(0, 2, None),
        },
    ) else {
        panic!("membership candidate");
    };
    let cleanup = state
        .stage_refresh_membership_cleanup(
            9,
            prepared,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    let wrong = AttachmentKey::new(old.generation() + 1, old.tab_id().clone());
    assert!(
        state
            .prepare_cleaned_membership_candidate(&cleanup, &wrong)
            .is_err()
    );
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, &old)
        .unwrap();
    drop(
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, None)
            .unwrap(),
    );
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.attached_key(), Some(&old));
    assert_eq!(state.accepted_frame, frame);
    assert!(state.pending_membership.is_some());
    assert!(state.invariant_holds());
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, &old)
        .unwrap();
    assert_eq!(
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, None)
            .unwrap()
            .commit(),
        MembershipInstallDecision::Empty
    );
    assert!(state.pending_membership.is_none());
    assert!(state.invariant_holds());
}

#[test]
fn live_empty_install_keeps_canonical_identity_and_retires_without_a_fictitious_key() {
    let mut state = state(1);
    let selected = state.active().unwrap().clone();
    state.phase = SessionPhase::SelectedWithoutAttachment(selected);
    state.provenance[0] = CreateTerminalProvenance::OutcomeUnknown;
    let expected = projected(0, 2);
    let prepared: PreparedMembershipCandidate = state
        .prepare_membership_candidate(expected.clone(), None, Some(0), None)
        .unwrap();
    let plan: PreparedMembershipInstall<'_> = state
        .prepare_membership_install(prepared, None, None)
        .unwrap();
    assert_eq!(plan.commit(), MembershipInstallDecision::Empty);
    assert_eq!(state.cache(), &expected);
    assert_eq!(state.phase_kind(), SessionPhaseKind::Empty);
    assert_eq!(state.active(), None);
    assert_eq!(state.desired(), None);
    assert!(state.invariant_holds());
    assert!(NativeSessionState::starting(expected, 8, geometry(80)).is_none());
    state.retire(CarrierFailure::Closed);
    assert!(state.invariant_holds());
}

#[test]
fn attached_member_cannot_be_removed_by_an_install_plan_without_cleanup() {
    let mut state = ready(1);
    let key = state.desired().unwrap().clone();
    let cache = state.cache().clone();
    let prepared = state
        .prepare_membership_candidate(projected(0, 2), None, Some(0), None)
        .unwrap();
    assert!(matches!(
        state.prepare_membership_install(prepared, None, None),
        Err(CarrierFailure::Protocol)
    ));
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.desired(), Some(&key));
    assert!(state.ready().is_some());
    assert!(state.invariant_holds());
}

#[test]
fn inactive_removal_reindexes_the_exact_ready_member_without_changing_its_proof() {
    let mut state = ready(2);
    let desired = state.desired().unwrap().clone();
    let frame = state.accepted_frame.clone();
    let geometry = state.last_accepted_geometry.clone();
    let prepared = state
        .prepare_membership_candidate(projected(1, 2), Some(desired.tab_id()), Some(0), None)
        .unwrap();
    assert_eq!(
        state
            .prepare_membership_install(prepared, Some(desired.clone()), None)
            .unwrap()
            .commit(),
        MembershipInstallDecision::Preserved
    );
    assert_eq!(state.desired(), Some(&desired));
    assert_eq!(state.accepted_frame, frame);
    assert_eq!(state.last_accepted_geometry, geometry);
    assert!(state.ready().is_some());
    assert!(state.invariant_holds());
}

#[test]
fn dropping_a_prepared_state_install_leaves_its_entire_visible_baseline_unchanged() {
    let mut state = ready(2);
    let desired = state.desired().cloned();
    let cache = state.cache().clone();
    let prepared = state
        .prepare_membership_candidate(
            projected(1, 2),
            desired.as_ref().map(AttachmentKey::tab_id),
            Some(0),
            None,
        )
        .unwrap();
    drop(
        state
            .prepare_membership_install(prepared, desired.clone(), None)
            .unwrap(),
    );
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.desired(), desired.as_ref());
    assert!(state.ready().is_some());
    assert!(state.invariant_holds());
}
