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
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: super::tab(0).root.clone(),
                slot_id: WorkbenchPanelSlotId::parse(&super::id(2_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&super::id(2_010)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
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
    super::attach_nonfirst_leaf(&mut state);
    let pane = state.plan_panel_focus_local(PanelDirection::Right).unwrap();
    assert_eq!(pane.anchor(), state.desired().unwrap().anchor());
    assert_eq!(
        state
            .tab(0)
            .unwrap()
            .panel(1)
            .unwrap()
            .binding()
            .expect("bound fixture")
            .terminal_id(),
        &TerminalId::parse(&super::id(2_012)).unwrap()
    );
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(state.active().unwrap().panel_index(), 1);
    assert!(state.invariant_holds());
}

#[test]
fn refused_switch_restores_every_split_projection_and_local_input_target() {
    let mut source = split_panel_baseline();
    source.terminal_contexts[0].tabs.push(tab(1));
    let mut state = ready_with_cache(project_native_standalone_tabs(&source).unwrap());
    let focus = state.focus_pane_ref().unwrap();
    let sibling = attach_nonfirst_leaf(&mut state);
    let frame = FrameIdentity::new(7);
    assert!(state.stage_drawn_frame(&sibling, frame));
    assert!(state.mark_feedback_accepted(&sibling, frame));
    let original = state.pollable_panes();
    assert_eq!(original.len(), 2);
    let marker = admit(&mut state, 1, 2);
    let marker_pane = state.desired().unwrap().clone();
    state
        .prepare_switch_restore(&marker_pane, &[focus.clone(), sibling.clone()])
        .unwrap()
        .commit();
    assert_eq!(state.desired(), Some(focus.pane()));
    assert!(state.desired().unwrap().anchor().generation() < marker.generation());
    assert_eq!(state.pollable_panes(), original);
    assert!(
        state.move_panel_focus_local(
            state
                .plan_panel_focus_local_at(sibling.pane().panel())
                .unwrap()
        )
    );
    assert_eq!(state.focus_pane_ref(), Some(sibling));
    assert_eq!(
        state.focus_accepted_frame().map(|(_, frame)| frame),
        Some(frame)
    );
    assert!(state.ready().is_some() && state.invariant_holds());
}

#[test]
fn split_refusal_requires_every_exact_retained_viewer_and_causal_proof() {
    for loss in [
        "missing projection",
        "wrong incarnation",
        "detached",
        "abandoned",
        "unfinished frame",
    ] {
        let mut source = split_panel_baseline();
        source.terminal_contexts[0].tabs.push(tab(1));
        let mut state = ready_with_cache(project_native_standalone_tabs(&source).unwrap());
        let focus = state.focus_pane_ref().unwrap();
        let sibling = attach_nonfirst_leaf(&mut state);
        if loss != "unfinished frame" {
            assert!(state.stage_drawn_frame(&sibling, FrameIdentity::new(7)));
            assert!(state.mark_feedback_accepted(&sibling, FrameIdentity::new(7)));
        }
        admit(&mut state, 1, 2);
        let mut retained = vec![focus.clone(), sibling.clone()];
        match loss {
            "missing projection" => {
                retained.pop();
            }
            "wrong incarnation" => {
                retained[1] = PaneRef::new(sibling.pane().clone(), sibling.incarnation() + 1);
            }
            "detached" => assert!(state.settle_pane_detached(&sibling)),
            "abandoned" => state.invalidate_switch_anchor_for(sibling.pane()),
            "unfinished frame" => {}
            _ => unreachable!(),
        }
        let marker = state.desired().unwrap().clone();
        assert!(
            state.prepare_switch_restore(&marker, &retained).is_err(),
            "{loss}"
        );
        assert_eq!(state.desired(), Some(&marker));
        assert!(state.pollable_panes().is_empty());
        assert!(state.invariant_holds());
    }
}

#[test]
fn rollback_does_not_revive_a_sibling_that_was_already_departing() {
    let mut source = split_panel_baseline();
    source.terminal_contexts[0].tabs.push(tab(1));
    let mut state = ready_with_cache(project_native_standalone_tabs(&source).unwrap());
    let focus = state.focus_pane_ref().unwrap();
    let sibling = attach_nonfirst_leaf(&mut state);
    assert!(state.sync_pane_set(
        &[(focus.pane().panel().clone(), 0, geometry(80))],
        Instant::now()
    ));
    assert!(state.live_pane_record(&sibling).unwrap().departing);
    admit(&mut state, 1, 2);
    let marker = state.desired().unwrap().clone();
    {
        let _prepared = state
            .prepare_switch_restore(&marker, std::slice::from_ref(&focus))
            .unwrap();
    }
    assert_eq!(state.desired(), Some(&marker));
    state
        .prepare_switch_restore(&marker, std::slice::from_ref(&focus))
        .unwrap()
        .commit();
    assert_eq!(state.pollable_panes(), vec![focus]);
    assert!(state.live_pane_record(&sibling).unwrap().departing);
    assert!(state.invariant_holds());
}

#[test]
fn unavailable_restores_only_the_exact_ready_anchor_and_plan_drop_is_effect_free() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let frame = state.focus_accepted_frame().clone();
    let accepted = state.focus_last_accepted_geometry().clone();
    let marker = admit(&mut state, 1, 2);
    assert_eq!(state.projection_key(), None);
    {
        let plan = state
            .prepare_switch_restore_k(&marker, Some(old.anchor()))
            .unwrap();
        assert_eq!(plan.restoration().marker(), &marker);
        assert_eq!(plan.restoration().restored(), &old);
    }
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&marker));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Switching);
    state
        .prepare_switch_restore_k(&marker, Some(old.anchor()))
        .unwrap()
        .commit();
    assert_eq!(state.desired(), Some(&old));
    assert!(state.ready().is_some());
    assert_eq!(state.focus_accepted_frame(), frame);
    assert_eq!(state.focus_last_accepted_geometry(), accepted);
    assert!(state.invariant_holds());
}

#[test]
fn unavailable_cannot_restore_missing_projection_or_ready_without_current_proof() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = admit(&mut state, 1, 2);
    assert!(
        state
            .prepare_switch_restore_k(&marker, Option::<&AttachmentKey>::None)
            .is_err()
    );
    assert!(
        state
            .prepare_switch_restore_k(&marker, Some(&marker))
            .is_err()
    );
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&marker));

    let mut unproven = ready(2);
    if let Some(record) = unproven.focus_record_mut() {
        record.accepted_frame = None;
    }
    let marker = admit(&mut unproven, 1, 2);
    assert!(
        unproven
            .prepare_switch_restore_k(&marker, Some(old.anchor()))
            .is_err()
    );
}

#[test]
fn accepted_old_resize_invalidates_anchor_without_rolling_back_its_result() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = admit(&mut state, 1, 2);
    assert!(matches!(
        state.settle_setup_configuration_event_t(&old, controlling(), Some(geometry(81))),
        Some(SetupEventDisposition::Superseded { .. })
    ));
    assert!(
        state
            .prepare_switch_restore_k(&marker, Some(old.anchor()))
            .is_err()
    );
    assert_eq!(state.last_accepted_geometry_t(&old), Some(geometry(81)));
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&marker));
    assert!(state.invariant_holds());
}

#[test]
fn geometry_change_and_actual_abandoned_causal_proof_each_prevent_restoration() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = admit(&mut state, 1, 2);
    state.observe_geometry(geometry(81));
    assert!(
        state
            .prepare_switch_restore_k(&marker, Some(old.anchor()))
            .is_err()
    );

    let mut abandoned = ready(2);
    let marker = admit(&mut abandoned, 1, 2);
    abandoned.invalidate_switch_anchor();
    assert!(
        abandoned
            .prepare_switch_restore_k(&marker, Some(old.anchor()))
            .is_err()
    );
}

#[test]
fn newer_marker_preserves_valid_old_anchor_but_stale_result_cannot_restore() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let stale = admit(&mut state, 1, 2);
    let current = admit(&mut state, 0, 3);
    assert!(
        state
            .prepare_switch_restore_k(&stale, Some(old.anchor()))
            .is_err()
    );
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&current));
    state
        .prepare_switch_restore_k(&current, Some(old.anchor()))
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
        .prepare_switch_restore_k(&marker, Option::<&AttachmentKey>::None)
        .unwrap()
        .commit();
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert_eq!(state.desired(), Some(selected.key()));
    assert_eq!(state.attached_key(), None);
    assert_eq!(state.projection_key(), None);
    assert!(state.invariant_holds());
}

#[test]
fn switch_intent_cannot_authorize_setup_before_candidate_installation() {
    let mut state = ready(2);
    let old = state.desired().unwrap().clone();
    let marker = admit(&mut state, 1, 2);
    assert!(!state.install_projection_t(&marker));
    assert_eq!(state.attached_key(), Some(&old));
    assert!(
        state
            .prepare_switch_restore_k(&marker, Some(old.anchor()))
            .is_ok()
    );

    let mut state = super::state(2);
    state.phase = SessionPhase::SelectedWithoutAttachment(state.active().unwrap().clone());
    let marker = admit(&mut state, 1, 2);
    assert!(!state.install_projection_t(&marker));
    assert_eq!(state.settle_attached_event_t(&marker), None);
    assert_eq!(
        state.settle_unattached_setup_t(&marker, 1, &CreateTerminalProvenance::BindingOnly),
        None
    );
    // Even independently supplied setup facts cannot promote an intent into attachment authority.
    if let Some(record) = state
        .panes
        .iter_mut()
        .find(|record| record.key.anchor() == &marker)
    {
        record.attached = true;
        record.projected = true;
    }
    assert!(!state.record_setup_configuration_t(&marker, controlling(), Some(geometry(80))));
    let SessionPhase::Switching(setup) = &mut state.phase else {
        panic!("Switch intent retained")
    };
    setup.mode = Some(controlling());
    if let Some(record) = state
        .panes
        .iter_mut()
        .find(|record| record.key.anchor() == &marker)
    {
        record.accepted_frame = Some(FrameIdentity::new(1));
        record.last_accepted_geometry = Some(geometry(80));
    }
    assert!(!state.publish_ready_t(
        &marker,
        FrameIdentity::new(1),
        controlling(),
        Some(geometry(80))
    ));
}
