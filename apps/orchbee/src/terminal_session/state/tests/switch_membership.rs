use super::*;
use crate::client_view::PanelDirection;

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
fn panel_focus_switch_keeps_the_tab_and_targets_the_selected_leaf_binding() {
    let mut source = super::snapshot(1, 1, None);
    source.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: super::tab(0).root.clone(),
                slot_id: WorkbenchPanelSlotId::parse(&super::id(2_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&super::id(2_010)).unwrap(),
                    primary_presentation: Presentation {
                        binding: Some(PresentationBinding {
                            terminal_id: TerminalId::parse(&super::id(2_012)).unwrap(),
                            worker_id: WorkerId::parse(&super::id(2_011)).unwrap(),
                        }),
                        label: "peer".into(),
                        presentation_id: TerminalPresentationId::parse(&super::id(2_013)).unwrap(),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&super::id(2_014)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&super::id(2_020)).unwrap(),
    };
    let mut state =
        super::ready_with_cache(super::project_native_standalone_tabs(&source).unwrap());
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let target = state
        .plan_panel_focus(PanelDirection::Right, 2, deadlines)
        .unwrap();
    assert_eq!(target.selected().index(), 0);
    assert_eq!(target.selected().panel_index(), 1);
    assert_eq!(
        target.binding().terminal_id(),
        &TerminalId::parse(&super::id(2_012)).unwrap()
    );
    assert!(state.begin_switch(target));
    let marker = state.desired().unwrap().clone();
    let fresh = super::install_switch_candidate(&mut state, &marker);
    assert_eq!(state.desired(), Some(&fresh));
    assert_eq!(state.active().unwrap().panel_index(), 1);
    assert!(state.invariant_holds());
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

#[test]
fn switch_intent_cannot_authorize_setup_before_candidate_installation() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = admit(&mut state, 1, 2);
    assert!(!state.install_projection(&marker));
    assert_eq!(state.attached_key(), Some(&old));
    assert!(state.prepare_switch_restore(&marker, Some(&old)).is_ok());

    let mut state = super::state(2);
    state.phase = SessionPhase::SelectedWithoutAttachment(state.active().unwrap().clone());
    let marker = admit(&mut state, 1, 2);
    assert!(!state.install_projection(&marker));
    assert_eq!(state.settle_attached_event(&marker), None);
    assert_eq!(
        state.settle_unattached_setup(&marker, 1, &CreateTerminalProvenance::BindingOnly),
        None
    );
    // Even independently supplied setup facts cannot promote an intent into attachment authority.
    state.attached_key = Some(marker.clone());
    state.projection_key = Some(marker.clone());
    assert!(!state.record_setup_configuration(&marker, controlling(), Some(geometry(80))));
    let SessionPhase::Switching(setup) = &mut state.phase else {
        panic!("Switch intent retained")
    };
    setup.mode = Some(controlling());
    state.accepted_frame = Some((marker.clone(), FrameIdentity::new(1)));
    state.last_accepted_geometry = Some((marker.clone(), geometry(80)));
    assert!(!state.publish_ready(
        &marker,
        FrameIdentity::new(1),
        controlling(),
        Some(geometry(80))
    ));
}
