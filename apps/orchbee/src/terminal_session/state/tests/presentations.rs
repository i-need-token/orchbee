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
    });
    source
}

fn choose(state: &mut NativeSessionState, generation: u64) -> AttachmentKey {
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
    assert_eq!(target.binding().terminal_id(), &terminal_id(9002));
    assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
    let key = target.selected().key().anchor().clone();
    assert!(state.begin_switch(target));
    key
}

fn finish(state: &mut NativeSessionState) {
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
    let mut state = ready_with_cache(project_native_standalone_tabs(&source()).unwrap());
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
    assert_eq!(target.binding().terminal_id(), &terminal_id(9002));
    assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
    assert!(state.invariant_holds());
}

#[test]
fn declined_child_switch_restores_primary_frame_and_member_choice() {
    let mut state = ready_with_cache(project_native_standalone_tabs(&source()).unwrap());
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
    let mut state = ready_with_cache(project_native_standalone_tabs(&source()).unwrap());
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
