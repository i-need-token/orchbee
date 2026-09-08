use super::*;

fn admit(state: &mut NativeSessionState, index: usize, generation: u64) -> AttachmentKey {
    let key = AttachmentKey::new(generation, state.tab(index).unwrap().tab_id().clone());
    let target = state
        .target_for_switch(
            index,
            key.clone(),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(state.begin_switch(target));
    key
}

#[test]
fn unavailable_restores_only_the_exact_ready_anchor_and_plan_drop_is_effect_free() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let frame = state.accepted_frame.clone();
    let accepted = state.last_accepted_geometry.clone();
    let marker = admit(&mut state, 1, 2);
    assert_eq!(state.projection_key(), None);
    {
        let plan = state.prepare_switch_restore(&marker, Some(&old)).unwrap();
        assert_eq!(plan.restoration().marker(), &marker);
        assert_eq!(plan.restoration().restored(), &old);
    }
    assert_eq!(state.desired(), Some(&marker));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    state
        .prepare_switch_restore(&marker, Some(&old))
        .unwrap()
        .commit();
    assert_eq!(state.desired(), Some(&old));
    assert!(state.ready().is_some());
    assert_eq!(state.accepted_frame, frame);
    assert_eq!(state.last_accepted_geometry, accepted);
    assert!(state.invariant_holds());
}

#[test]
fn unavailable_cannot_restore_missing_projection_or_ready_without_current_proof() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = admit(&mut state, 1, 2);
    assert!(state.prepare_switch_restore(&marker, None).is_err());
    assert!(
        state
            .prepare_switch_restore(&marker, Some(&marker))
            .is_err()
    );
    assert_eq!(state.desired(), Some(&marker));

    let mut unproven = ready(2);
    unproven.accepted_frame = None;
    let marker = admit(&mut unproven, 1, 2);
    assert!(
        unproven
            .prepare_switch_restore(&marker, Some(&old))
            .is_err()
    );
}

#[test]
fn accepted_old_resize_invalidates_anchor_without_rolling_back_its_result() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = admit(&mut state, 1, 2);
    assert!(matches!(
        state.settle_setup_configuration_event(&old, controlling(), Some(geometry(81))),
        Some(SetupEventDisposition::Superseded { .. })
    ));
    assert!(state.prepare_switch_restore(&marker, Some(&old)).is_err());
    assert_eq!(state.last_accepted_geometry(&old), Some(geometry(81)));
    assert_eq!(state.desired(), Some(&marker));
    assert!(state.invariant_holds());
}

#[test]
fn geometry_change_and_actual_abandoned_causal_proof_each_prevent_restoration() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = admit(&mut state, 1, 2);
    state.observe_geometry(geometry(81));
    assert!(state.prepare_switch_restore(&marker, Some(&old)).is_err());

    let mut abandoned = ready(2);
    let marker = admit(&mut abandoned, 1, 2);
    abandoned.invalidate_switch_anchor();
    assert!(
        abandoned
            .prepare_switch_restore(&marker, Some(&old))
            .is_err()
    );
}

#[test]
fn newer_marker_preserves_valid_old_anchor_but_stale_result_cannot_restore() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let stale = admit(&mut state, 1, 2);
    let current = admit(&mut state, 0, 3);
    assert!(state.prepare_switch_restore(&stale, Some(&old)).is_err());
    assert_eq!(state.desired(), Some(&current));
    state
        .prepare_switch_restore(&current, Some(&old))
        .unwrap()
        .commit();
    assert_eq!(state.desired(), Some(&old));
    assert!(state.ready().is_some());
    assert!(state.invariant_holds());
}

#[test]
fn real_selected_origin_restores_without_inventing_a_viewer() {
    let mut state = state(2);
    let selected = state.active().unwrap().clone();
    state.phase = SessionPhase::SelectedWithoutAttachment(selected.clone());
    let marker = admit(&mut state, 1, 2);
    state
        .prepare_switch_restore(&marker, None)
        .unwrap()
        .commit();
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert_eq!(state.desired(), Some(selected.key()));
    assert_eq!(state.attached_key, None);
    assert_eq!(state.projection_key(), None);
    assert!(state.invariant_holds());
}
