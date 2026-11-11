use super::*;

fn source() -> Snapshot {
    let mut source = snapshot(2, 1, None);
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    child_presentations.push(Presentation {
        presentation_id: presentation_id(9000),
        label: "child".into(),
        custom_label: None,
        binding: Some(PresentationBinding {
            worker_id: worker_id(9001),
            terminal_id: terminal_id(9002),
        }),
        launch_failure: None,
        agent_not_resumed: None,
        agent_start_failure: None,
        launch_program: None,
    });
    source
}

pub(super) fn choose(state: &mut WorkbenchClientState, generation: u64) -> AttachmentKey {
    let (index, panel, presentation) = state.next_presentation(true).unwrap();
    let target = state
        .target_for_presentation(
            index,
            &panel,
            &presentation,
            generation,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert_eq!(target.binding().unwrap().terminal_id(), &terminal_id(9002));
    assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
    let key = target.selected().key().anchor().clone();
    assert!(state.begin_switch(target));
    key
}

pub(super) fn finish(state: &mut WorkbenchClientState) {
    let pane = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&pane));
    assert!(state.record_setup_configuration_t(&pane, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame_t(&pane, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted_t(&pane, FrameIdentity::new(2)));
    assert!(state.publish_ready_t(
        &pane,
        FrameIdentity::new(2),
        controlling(),
        Some(geometry(80))
    ));
}

#[test]
fn child_choice_uses_fresh_cleanup_and_remembers_binding_on_tab_return() {
    let mut state = ready_with_cache(project_preferred_workspace(&source()).unwrap());
    // A successful primary Create is not the child's terminal provenance.
    state.provenance[0] = CreateTerminalProvenance::Started;
    let old = state.desired().unwrap().clone();
    let old_ref = state.pane_ref_for(&old).unwrap();
    let marker = choose(&mut state, 2);
    assert!(state.projection_key().is_none());
    let installed = install_switch_candidate(&mut state, &marker);
    assert_eq!(
        state.focus_record().unwrap().binding().terminal_id(),
        &terminal_id(9002)
    );
    assert!(!state.stage_drawn_frame(&old_ref, FrameIdentity::new(99)));
    finish(&mut state);
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(1));
    assert_eq!(state.tab(0).unwrap().panel_count(), 1);
    let next = AttachmentKey::new(
        installed.generation() + 1,
        state.tab(1).unwrap().tab_id().clone(),
    );
    let target = state
        .target_for_switch(
            1,
            next.clone(),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(state.begin_switch(target));
    let next = install_switch_candidate(&mut state, &next);
    finish(&mut state);
    let returning = AttachmentKey::new(
        next.generation() + 1,
        state.tab(0).unwrap().tab_id().clone(),
    );
    let target = state
        .target_for_switch(
            0,
            returning,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert_eq!(target.binding().unwrap().terminal_id(), &terminal_id(9002));
    assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
    assert!(state.invariant_holds());
}

#[test]
fn declined_child_switch_restores_primary_frame_and_member_choice() {
    let mut state = ready_with_cache(project_preferred_workspace(&source()).unwrap());
    let old = state.desired().unwrap().clone();
    let frame = state.focus_accepted_frame();
    let marker = choose(&mut state, 2);
    state
        .prepare_switch_restore_k(&marker, Some(old.anchor()))
        .unwrap()
        .commit();
    assert_eq!(state.desired(), Some(&old));
    assert_eq!(state.focus_accepted_frame(), frame);
    let panel = state.tab(0).unwrap().panel(0).unwrap();
    assert_eq!(state.selected_presentation(panel), panel.primary());
    assert!(state.invariant_holds());
}

#[test]
fn removed_selected_child_requires_cleanup_and_falls_back_to_primary() {
    let mut state = ready_with_cache(project_preferred_workspace(&source()).unwrap());
    let marker = choose(&mut state, 2);
    install_switch_candidate(&mut state, &marker);
    finish(&mut state);
    let old = state.desired().unwrap().clone();
    let mut candidate = source();
    candidate.revision = WorkbenchRevision::new(2);
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut candidate.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    child_presentations.clear();
    assert!(state.start_pending_refresh(10));
    let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
        10,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    ) else {
        panic!("child removal requires membership cleanup")
    };
    assert!(!prepared.preserves_selection());
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let cleanup = state
        .stage_refresh_membership_cleanup(10, prepared, deadlines)
        .unwrap();
    assert_eq!(cleanup.old(), old.anchor());
    let candidate = state
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
        .unwrap();
    let desired = AttachmentKey::new(old.generation() + 1, old.anchor().tab_id().clone());
    let MembershipInstallDecision::Attach(target) = state
        .prepare_cleaned_membership_install(&cleanup, candidate, Some(desired))
        .unwrap()
        .commit()
    else {
        panic!("reattach primary")
    };
    assert_eq!(
        target.binding(),
        state.tab(0).unwrap().panel(0).unwrap().binding()
    );
    assert_eq!(state.next_presentation(true), None);
    assert!(state.invariant_holds());
}

fn empty_source() -> Snapshot {
    let mut source = source();
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = &mut source.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    primary_presentation.binding = None;
    child_presentations[0].binding = None;
    source
}

#[test]
fn unbound_startup_choice_round_trip_and_later_binding_keep_exact_child() {
    let source = empty_source();
    let (mut state, bootstrap) = WorkbenchClientState::starting(
        project_preferred_workspace(&source).unwrap(),
        1,
        geometry(80),
    )
    .unwrap();
    assert!(bootstrap.binding().is_none());
    assert_eq!(
        state.phase_kind(),
        ClientPhaseKind::SelectedWithoutAttachment
    );
    assert!(state.panes.is_empty() && state.invariant_holds());
    let panel_id = state.tab(0).unwrap().panel_id().clone();
    let member = presentation_id(9000);
    let target = state
        .target_for_presentation(
            0,
            &panel_id,
            &member,
            2,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(target.binding().is_none());
    let marker = target.selected().key().anchor().clone();
    assert!(state.begin_switch(target));
    install_switch_candidate(&mut state, &marker);
    assert_eq!(
        state.phase_kind(),
        ClientPhaseKind::SelectedWithoutAttachment
    );
    assert!(state.panes.is_empty());
    assert_eq!(
        state
            .selected_presentation(state.tab(0).unwrap().panel(0).unwrap())
            .presentation_id(),
        &member
    );
    let target = state
        .target_for_switch(
            1,
            AttachmentKey::new(4, state.tab(1).unwrap().tab_id().clone()),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    let marker = target.selected().key().anchor().clone();
    assert!(state.begin_switch(target));
    install_switch_candidate(&mut state, &marker);
    finish(&mut state);
    let target = state
        .target_for_switch(
            0,
            AttachmentKey::new(6, state.tab(0).unwrap().tab_id().clone()),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    let marker = target.selected().key().anchor().clone();
    assert!(target.binding().is_none());
    assert!(state.begin_switch(target));
    install_switch_candidate(&mut state, &marker);
    assert_eq!(
        state
            .selected_presentation(state.tab(0).unwrap().panel(0).unwrap())
            .presentation_id(),
        &member
    );
    let mut bound = source.clone();
    bound.revision = WorkbenchRevision::new(2);
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut bound.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    child_presentations[0].binding = Some(PresentationBinding {
        worker_id: worker_id(9001),
        terminal_id: terminal_id(9002),
    });
    assert!(state.start_pending_refresh(10));
    let PreparedRefresh::Membership { prepared, .. } =
        state.prepare_refresh(10, WorkbenchSnapshotOutcome::Available { snapshot: bound })
    else {
        panic!("binding needs new membership epoch")
    };
    assert!(!prepared.preserves_selection());
    let desired = AttachmentKey::new(8, state.tab(0).unwrap().tab_id().clone());
    let MembershipInstallDecision::Attach(target) = state
        .prepare_membership_install(
            prepared,
            Some(desired),
            OperationDeadlines::from_sample(Instant::now()),
        )
        .unwrap()
        .commit()
    else {
        panic!("real bound child attaches")
    };
    assert_eq!(target.binding().unwrap().terminal_id(), &terminal_id(9002));
    assert!(state.tab(0).unwrap().binding().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn unbound_panel_focus_preserves_live_siblings_and_empty_close_targets() {
    let mut source = split_panel_baseline();
    let PanelNode::Split { children, .. } = &mut source.terminal_contexts[0].tabs[0].root else {
        unreachable!()
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[1].node
    else {
        unreachable!()
    };
    primary_presentation.binding = None;
    let mut state = ready_with_cache(project_preferred_workspace(&source).unwrap());
    let old = state.focus_pane_ref().unwrap();
    let empty = state.tab(0).unwrap().panel(1).unwrap().panel_id().clone();
    let pane = state.plan_panel_focus_local_at(&empty).unwrap();
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(
        state.phase_kind(),
        ClientPhaseKind::SelectedWithoutAttachment
    );
    assert!(state.focus_pane_ref().is_none() && state.ready().is_none());
    assert_eq!(state.pollable_panes(), vec![old.clone()]);
    assert!(state.invariant_holds());
    state.open_panel_close_confirmation().unwrap();
    assert!(state.panel_close.confirmation().is_some());
    state.cancel_panel_close_confirmation();
    state.open_close_confirmation().unwrap();
    assert!(state.close.confirmation().is_some());
    state.close.cancel_confirmation();
    let pane = state.plan_panel_focus_local_at(old.pane().panel()).unwrap();
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(state.focus_pane_ref(), Some(old));
    assert!(state.ready().is_some() && state.invariant_holds());
}

#[test]
fn unbound_focus_with_live_sibling_cleans_the_real_epoch_on_switch_and_binding_refresh() {
    for (bind, rollback) in [(false, false), (true, false), (false, true)] {
        let mut source = split_panel_baseline();
        source.terminal_contexts[0]
            .tabs
            .push(snapshot(2, 1, None).terminal_contexts[0].tabs[1].clone());
        let bound = source.clone();
        let PanelNode::Split { children, .. } = &mut source.terminal_contexts[0].tabs[0].root
        else {
            unreachable!()
        };
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut children[1].node
        else {
            unreachable!()
        };
        primary_presentation.binding = None;
        let mut state = ready_with_cache(project_preferred_workspace(&source).unwrap());
        let old = state.desired().unwrap().anchor().clone();
        let sibling = state.focus_pane_ref().unwrap();
        let empty = state.tab(0).unwrap().panel(1).unwrap().panel_id().clone();
        assert!(state.move_panel_focus_local(state.plan_panel_focus_local_at(&empty).unwrap()));
        assert!(state.has_attachments() && state.invariant_holds());
        if bind {
            let mut candidate = bound;
            candidate.revision = WorkbenchRevision::new(2);
            assert!(state.start_pending_refresh(10));
            let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
                10,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: candidate,
                },
            ) else {
                panic!("binding refresh")
            };
            let cleanup = state
                .stage_refresh_membership_cleanup(
                    10,
                    prepared,
                    OperationDeadlines::from_sample(Instant::now()).unwrap(),
                )
                .unwrap();
            assert!(state.invariant_holds());
            assert_eq!(cleanup.old(), &old);
            let candidate = state
                .prepare_cleaned_membership_candidate(&cleanup, &old)
                .unwrap();
            let next = AttachmentKey::new(2, old.tab_id().clone());
            let MembershipInstallDecision::Attach(target) = state
                .prepare_cleaned_membership_install(&cleanup, candidate, Some(next))
                .unwrap()
                .commit()
            else {
                panic!("new binding")
            };
            assert_eq!(target.binding().unwrap().terminal_id(), &terminal_id(3012));
        } else {
            let marker = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
            let target = state
                .target_for_switch(
                    1,
                    marker.clone(),
                    OperationDeadlines::from_sample(Instant::now()).unwrap(),
                )
                .unwrap();
            assert!(state.begin_switch(target));
            assert!(state.invariant_holds());
            if rollback {
                let requested = state.desired().unwrap().clone();
                assert!(state.prepare_switch_restore(&requested, &[]).is_err());
                state
                    .prepare_switch_restore(&requested, std::slice::from_ref(&sibling))
                    .unwrap()
                    .commit();
                assert!(state.selected_is_unbound());
                assert_eq!(state.pollable_panes(), vec![sibling.clone()]);
                assert!(
                    state.move_panel_focus_local(
                        state
                            .plan_panel_focus_local_at(sibling.pane().panel())
                            .unwrap()
                    )
                );
                assert!(state.ready().is_some());
            } else {
                install_switch_candidate(&mut state, &marker);
                assert_eq!(state.active().unwrap().index(), 1);
            }
        }
        assert!(state.invariant_holds());
    }
}
