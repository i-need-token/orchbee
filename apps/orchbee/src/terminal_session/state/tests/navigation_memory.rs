use super::context_selection::{contexts, finish_selection};
use super::*;
use crate::terminal_session::workbench_projection::project_native_context_tabs;

#[test]
fn query_active_tabs_uses_private_surviving_memory_without_visiting() {
    let snapshot = contexts(2, 1);
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    let other = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    assert!(state.start_context_selection_at(10, 1, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 10);
    switch(&mut state, 1, 30);
    let remembered = state.active().unwrap().key().anchor().tab_id().clone();
    let location = state.cache.location().clone();
    assert!(state.start_context_selection_at(40, 0, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 40);
    let before = state.active().unwrap().key().clone();
    let candidate = project_native_context_tabs(&snapshot, &location).unwrap();
    assert_eq!(
        state.query_active_tab_id(&candidate),
        Ok(Some(remembered.clone()))
    );
    assert_ne!(
        other.query_active_tab_id(&candidate),
        Ok(Some(remembered.clone()))
    );
    let mut reordered = snapshot.clone();
    reordered.terminal_contexts[1].tabs.reverse();
    let candidate = project_native_context_tabs(&reordered, &location).unwrap();
    assert_eq!(
        state.query_active_tab_id(&candidate),
        Ok(Some(remembered.clone()))
    );
    reordered.terminal_contexts[1]
        .tabs
        .retain(|tab| tab.tab_id != remembered);
    let candidate = project_native_context_tabs(&reordered, &location).unwrap();
    assert_eq!(
        state.query_active_tab_id(&candidate),
        Ok(candidate.tabs().first().map(|tab| tab.tab_id().clone()))
    );
    reordered.terminal_contexts[1].tabs.clear();
    let candidate = project_native_context_tabs(&reordered, &location).unwrap();
    assert_eq!(state.query_active_tab_id(&candidate), Ok(None));
    assert_eq!(state.active().unwrap().key(), &before);
    assert_eq!(state.navigation_memory.tab(&location), Some(&remembered));
}

fn deadlines() -> OperationDeadlines {
    OperationDeadlines::from_sample(Instant::now()).unwrap()
}

fn settle_ready(state: &mut NativeSessionState, frame: u64) {
    let pane = state.desired().unwrap().clone();
    let frame = FrameIdentity::new(frame);
    assert!(state.install_projection_t(&pane));
    assert!(state.record_setup_configuration_t(&pane, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&pane, frame));
    assert!(state.mark_feedback_accepted_t(&pane, frame));
    assert!(state.publish_ready_t(&pane, frame, ControllerMode::ReadOnly, None));
    assert!(state.invariant_holds());
}

fn switch(state: &mut NativeSessionState, index: usize, generation: u64) {
    let key = AttachmentKey::new(generation, state.tab(index).unwrap().tab_id().clone());
    let target = state
        .target_for_switch(index, key.clone(), deadlines())
        .unwrap();
    assert!(state.begin_switch(target));
    install_switch_candidate(state, &key);
    settle_ready(state, generation);
}

fn split_contexts() -> Snapshot {
    let mut snapshot = contexts(2, 1);
    snapshot.terminal_contexts[0].tabs[0].root = split_panel_baseline().terminal_contexts[0].tabs
        [0]
    .root
    .clone();
    snapshot
}

fn focus_second_panel(state: &mut NativeSessionState) -> WorkbenchPanelId {
    let peer = attach_nonfirst_leaf(state);
    let frame = FrameIdentity::new(50);
    assert!(state.stage_drawn_frame_t(&peer, frame));
    assert!(state.mark_feedback_accepted_t(&peer, frame));
    assert!(state.move_panel_focus_local(peer.pane().clone()));
    assert!(state.invariant_holds());
    peer.pane().panel().clone()
}

#[test]
fn navigation_memory_restores_nonfirst_tabs_per_context_and_per_client() {
    let snapshot = contexts(2, 1);
    let mut first = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    let second = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    switch(&mut first, 1, 2);
    let standalone_tab = first.active().unwrap().key().anchor().tab_id().clone();
    assert!(first.start_context_selection(10, true, deadlines()));
    finish_selection(&mut first, snapshot.clone(), 10);
    switch(&mut first, 1, 30);
    let workspace_tab = first.active().unwrap().key().anchor().tab_id().clone();
    assert!(first.start_context_selection(40, true, deadlines()));
    finish_selection(&mut first, snapshot.clone(), 40);
    assert!(first.desired().is_none());
    assert!(first.start_context_selection(50, true, deadlines()));
    finish_selection(&mut first, snapshot.clone(), 50);
    assert_eq!(
        first.active().unwrap().key().anchor().tab_id(),
        &standalone_tab
    );
    assert!(first.start_context_selection_at(60, 1, deadlines()));
    finish_selection(&mut first, snapshot, 60);
    assert_eq!(
        first.active().unwrap().key().anchor().tab_id(),
        &workspace_tab
    );
    assert_eq!(second.active().unwrap().index(), 0);
    assert!(
        second
            .navigation_memory
            .tab(second.cache.location())
            .is_none()
    );
}

#[test]
fn navigation_memory_restores_panel_through_tab_and_context_departure() {
    let snapshot = split_contexts();
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    let panel = focus_second_panel(&mut state);
    switch(&mut state, 1, 2);
    assert_eq!(
        state.layout_focus_panel_id(state.cache.location(), &state.cache.tabs()[0]),
        Some(panel.clone())
    );
    assert_eq!(state.active().unwrap().index(), 1);
    switch(&mut state, 0, 4);
    assert_eq!(state.desired().unwrap().panel(), &panel);
    assert_eq!(state.active().unwrap().panel_index(), 1);
    assert!(state.start_context_selection(10, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 10);
    assert!(state.start_context_selection(20, false, deadlines()));
    finish_selection(&mut state, snapshot, 20);
    assert_eq!(state.desired().unwrap().panel(), &panel);
    assert_eq!(state.active().unwrap().panel_index(), 1);
}

#[test]
fn navigation_memory_reentry_resolves_reordered_ids_and_removed_tab_to_empty() {
    let mut snapshot = contexts(2, 1);
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    switch(&mut state, 1, 2);
    let remembered = state.active().unwrap().key().anchor().tab_id().clone();
    assert!(state.start_context_selection(10, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 10);
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.terminal_contexts[0].tabs.swap(0, 1);
    assert!(state.start_context_selection(20, false, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 20);
    assert_eq!(state.active().unwrap().index(), 0);
    assert_eq!(state.desired().unwrap().anchor().tab_id(), &remembered);
    assert!(state.start_context_selection(30, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 30);
    snapshot.revision = WorkbenchRevision::new(3);
    snapshot.terminal_contexts[0].tabs.remove(0);
    assert!(state.start_context_selection(40, false, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 40);
    assert_ne!(state.desired().unwrap().anchor().tab_id(), &remembered);
    // Committing the surviving Presentation remembers the destination actually
    // entered. The removed Tab must never be revived by the older context cache.
    assert_eq!(
        state.navigation_memory.tab(state.cache.location()),
        Some(state.desired().unwrap().anchor().tab_id())
    );
    assert!(state.start_context_selection(50, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 50);
    snapshot.revision = WorkbenchRevision::new(4);
    snapshot.terminal_contexts[0].tabs.clear();
    assert!(state.start_context_selection(60, false, deadlines()));
    finish_selection(&mut state, snapshot, 60);
    assert!(state.desired().is_none());
    assert!(
        state
            .navigation_memory
            .tab(state.cache.location())
            .is_none()
    );
}

#[test]
fn navigation_memory_explicit_panel_overrides_history_and_supersession_does_not_visit_target() {
    let snapshot = split_contexts();
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    let remembered = focus_second_panel(&mut state);
    switch(&mut state, 1, 2);
    let first_panel = state.tab(0).unwrap().panel(0).unwrap().panel_id().clone();
    let tab_id = state.tab(0).unwrap().tab_id().clone();
    let target = state
        .target_for_switch_for_panel(
            0,
            AttachmentKey::new(4, tab_id.clone()),
            deadlines(),
            &first_panel,
        )
        .unwrap();
    assert_eq!(target.selected().key().panel(), &first_panel);
    assert!(state.begin_switch(target));
    let back = state
        .target_for_switch(
            1,
            AttachmentKey::new(5, state.tab(1).unwrap().tab_id().clone()),
            deadlines(),
        )
        .unwrap();
    assert!(state.begin_switch(back));
    let again = state
        .target_for_switch(0, AttachmentKey::new(6, tab_id), deadlines())
        .unwrap();
    assert_eq!(again.selected().key().panel(), &remembered);
    assert_eq!(
        state.navigation_memory.tab(state.cache.location()),
        Some(state.tab(1).unwrap().tab_id())
    );
}

#[test]
fn navigation_memory_reordered_panel_uses_identity_and_removed_panel_uses_first() {
    for remove in [false, true] {
        let mut snapshot = split_contexts();
        let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
        let remembered = focus_second_panel(&mut state);
        assert!(state.start_context_selection(10, true, deadlines()));
        finish_selection(&mut state, snapshot.clone(), 10);
        let root = &mut snapshot.terminal_contexts[0].tabs[0].root;
        let PanelNode::Split {
            children, split_id, ..
        } = root
        else {
            panic!("split fixture")
        };
        if remove {
            *root = children[0].node.clone();
        } else {
            children.swap(0, 1);
            *split_id = WorkbenchSplitId::parse(&id(4_020)).unwrap();
            children[0].slot_id = WorkbenchPanelSlotId::parse(&id(4_001)).unwrap();
            children[1].slot_id = WorkbenchPanelSlotId::parse(&id(4_014)).unwrap();
        }
        snapshot.revision = WorkbenchRevision::new(2);
        assert!(state.start_context_selection(20, false, deadlines()));
        finish_selection(&mut state, snapshot, 20);
        assert_eq!(state.active().unwrap().panel_index(), 0);
        assert_eq!(state.desired().unwrap().panel() == &remembered, !remove);
    }
}

#[test]
fn navigation_memory_does_not_turn_nonattachable_create_into_permission() {
    let snapshot = contexts(2, 1);
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    switch(&mut state, 1, 2);
    state.provenance[1] = CreateTerminalProvenance::OutcomeUnknown;
    let tab = state.desired().unwrap().anchor().tab_id().clone();
    assert!(state.start_context_selection(10, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 10);
    assert!(state.start_context_selection(20, false, deadlines()));
    finish_selection(&mut state, snapshot, 20);
    assert_eq!(state.desired().unwrap().anchor().tab_id(), &tab);
    assert_eq!(
        state.provenance(1),
        Some(&CreateTerminalProvenance::OutcomeUnknown)
    );
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::SelectedWithoutAttachment
    );
    assert!(!state.has_attachments());
}

#[test]
fn navigation_memory_switch_install_resolves_captured_panel_in_new_candidate() {
    for remove in [false, true] {
        let mut snapshot = split_contexts();
        let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
        let panel = focus_second_panel(&mut state);
        switch(&mut state, 1, 2);
        let tab = state.tab(0).unwrap().tab_id().clone();
        let target = state
            .target_for_switch(0, AttachmentKey::new(4, tab.clone()), deadlines())
            .unwrap();
        let marker = target.selected().key().clone();
        assert!(state.begin_switch(target));
        let root = &mut snapshot.terminal_contexts[0].tabs[0].root;
        let PanelNode::Split {
            children, split_id, ..
        } = root
        else {
            panic!("split fixture")
        };
        if remove {
            *root = children[0].node.clone();
        } else {
            children.swap(0, 1);
            *split_id = WorkbenchSplitId::parse(&id(4_020)).unwrap();
            children[0].slot_id = WorkbenchPanelSlotId::parse(&id(4_001)).unwrap();
            children[1].slot_id = WorkbenchPanelSlotId::parse(&id(4_014)).unwrap();
        }
        snapshot.revision = WorkbenchRevision::new(2);
        let candidate = state
            .prepare_switch_candidate(&marker, WorkbenchSnapshotOutcome::Available { snapshot });
        if remove {
            assert!(matches!(
                candidate,
                Err(CarrierFailure::AuthorityDivergence)
            ));
            assert!(state.pending_membership.is_none());
            assert!(state.has_attachments());
            continue;
        }
        let candidate = candidate.unwrap();
        let budget = state.switch_deadlines(&marker).unwrap();
        let cleanup = state
            .stage_switch_membership_cleanup(&marker, candidate, budget)
            .unwrap();
        let candidate = state
            .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
            .unwrap();
        let plan = state.prepare_cleaned_membership_install(
            &cleanup,
            candidate,
            Some(AttachmentKey::new(5, tab)),
        );
        let MembershipInstallDecision::Attach(target) = plan.unwrap().commit() else {
            panic!("attach")
        };
        assert_eq!(target.selected().key().panel(), &panel);
        assert_eq!(target.selected().panel_index(), 0);
        settle_ready(&mut state, 6);
        assert_eq!(state.desired().unwrap().panel(), &panel);
    }
}

#[test]
fn navigation_memory_prunes_removed_context_even_when_create_facts_are_retained() {
    let mut snapshot = contexts(2, 1);
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    assert!(state.start_context_selection(10, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 10);
    let location = state.cache.location().clone();
    state.provenance[0] = CreateTerminalProvenance::OutcomeUnknown;
    assert!(state.start_context_selection(20, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 20);
    assert!(state.navigation_memory.tab(&location).is_some());
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.workspaces.remove(0);
    snapshot.terminal_contexts.remove(1);
    assert!(state.start_context_selection(30, true, deadlines()));
    finish_selection(&mut state, snapshot, 30);
    assert!(
        state
            .retained_contexts
            .iter()
            .any(|context| context.location() == &location)
    );
    assert!(state.navigation_memory.tab(&location).is_none());
}

#[test]
fn navigation_memory_creation_admission_keeps_the_departure_not_the_future_member() {
    let snapshot = split_contexts();
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    focus_second_panel(&mut state);
    let operation = create_operation(&state, 2, 20, "New Tab");
    assert!(state.begin_create(operation));
    assert_eq!(
        state.navigation_memory.tab(state.cache.location()),
        Some(state.tab(0).unwrap().tab_id())
    );
    assert_eq!(
        state
            .navigation_memory
            .panel_index(state.cache.location(), state.tab(0).unwrap()),
        1
    );
    assert!(state.invariant_holds());
}

#[test]
fn navigation_memory_remembers_ready_focus_across_a_transient_frame_proof_gap() {
    let snapshot = split_contexts();
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    let panel = focus_second_panel(&mut state);
    let selected = state.desired().unwrap().clone();
    assert!(state.invalidate_projection_frame_t(&selected));
    assert!(!state.feedback_accepted());
    assert!(state.ready().is_some());
    switch(&mut state, 1, 2);
    switch(&mut state, 0, 4);
    assert_eq!(state.desired().unwrap().panel(), &panel);
    assert_eq!(state.active().unwrap().panel_index(), 1);
}
