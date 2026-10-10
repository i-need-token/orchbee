use super::*;
use orchbee_workbench_protocol::{Project, WorkbenchProjectId, WorkbenchWorkspaceId, Workspace};

#[cfg(windows)]
#[test]
fn worktree_entry_exact_primary_override_survives_cleanup_and_does_not_change_ordinary_memory() {
    use crate::terminal_session::NativeMemberTarget;
    use crate::terminal_session::workbench_projection::project_native_context_tabs;
    for exact in [false, true] {
        let mut snapshot = contexts(1, 1);
        let child_id = presentation_id(9100);
        let PanelNode::Panel {
            child_presentations,
            ..
        } = &mut snapshot.terminal_contexts[1].tabs[0].root
        else {
            panic!("workspace panel");
        };
        child_presentations.push(Presentation {
            presentation_id: child_id.clone(),
            label: "child".into(),
            custom_label: None,
            binding: Some(PresentationBinding {
                terminal_id: terminal_id(9101),
                worker_id: worker_id(9102),
            }),
        });
        let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
        let location = state.cache.navigation()[1].location.clone();
        let target = project_native_context_tabs(&snapshot, &location).unwrap();
        let tab = &target.tabs()[0];
        let panel = tab.panel(0).unwrap();
        let member = NativeMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: panel.primary().presentation_id().clone(),
        };
        let primary_binding = panel.primary().binding().unwrap().clone();
        let remembered = SelectedTab::new_panel(
            PaneKey::new(
                AttachmentKey::new(99, member.tab.clone()),
                member.panel.clone(),
            ),
            0,
            0,
        );
        state
            .navigation_memory
            .remember_presentation(&target, &remembered, child_id.clone());
        assert_eq!(
            state
                .navigation_memory
                .presentation(&location, panel)
                .presentation_id(),
            &child_id
        );
        assert!(state.start_context_selection_at(1, 1, deadlines()));
        let PreparedRefresh::Membership { mut prepared, .. } =
            state.prepare_refresh(1, WorkbenchSnapshotOutcome::Available { snapshot })
        else {
            panic!("context candidate");
        };
        if exact {
            assert!(prepared.retarget_selected_member(&location, &member));
        }
        let desired = AttachmentKey::new(11, member.tab.clone());
        let cleanup = state
            .stage_refresh_membership_cleanup(1, prepared, deadlines())
            .unwrap();
        let prepared = state
            .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
            .unwrap();
        let MembershipInstallDecision::Attach(attach) = state
            .prepare_cleaned_membership_install(&cleanup, prepared, Some(desired))
            .unwrap()
            .commit()
        else {
            panic!("exact attach");
        };
        if exact {
            assert_eq!(*attach.presentation_id, member.presentation);
            assert_eq!(attach.binding(), Some(&primary_binding));
        } else {
            assert_eq!(*attach.presentation_id, child_id);
            assert_eq!(attach.binding().unwrap().terminal_id(), &terminal_id(9101));
        }
        assert!(state.invariant_holds());
    }
}

fn deadlines() -> OperationDeadlines {
    OperationDeadlines::from_sample(Instant::now()).unwrap()
}

pub(super) fn contexts(workspace_tabs: usize, revision: u64) -> Snapshot {
    let mut snapshot = snapshot(2, revision, None);
    snapshot.navigation.workspaces.extend(vec![
        Workspace {
            project_id: Some((WorkbenchProjectId::parse(&id(70)).unwrap()).clone()),
            label: "same label".into(),
            primary_directory_id: None,
            workspace_id: WorkbenchWorkspaceId::parse(&id(71)).unwrap(),
            terminal_context_id: context_id(72),
        },
        Workspace {
            project_id: Some((WorkbenchProjectId::parse(&id(70)).unwrap()).clone()),
            label: "same label".into(),
            primary_directory_id: None,
            workspace_id: WorkbenchWorkspaceId::parse(&id(73)).unwrap(),
            terminal_context_id: context_id(74),
        },
    ]);
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
        label: "same label".into(),
    });
    snapshot.terminal_contexts.push(TerminalContext {
        groups: Vec::new(),
        tabs: (10..10 + workspace_tabs).map(tab).collect(),
        terminal_context_id: context_id(72),
    });
    snapshot.terminal_contexts.push(TerminalContext {
        groups: Vec::new(),
        tabs: Vec::new(),
        terminal_context_id: context_id(74),
    });
    snapshot
}

fn context_state(workspace_tabs: usize) -> NativeSessionState {
    ready_with_cache(project_native_standalone_tabs(&contexts(workspace_tabs, 1)).unwrap())
}

pub(super) fn finish_selection(
    state: &mut NativeSessionState,
    snapshot: Snapshot,
    generation: u64,
) {
    let deadlines = state.context_selection_deadlines().unwrap();
    let PreparedRefresh::Membership { prepared, .. } =
        state.prepare_refresh(generation, WorkbenchSnapshotOutcome::Available { snapshot })
    else {
        panic!("context candidate");
    };
    assert!(!prepared.preserves_selection());
    let desired = prepared
        .selected_tab_id()
        .map(|tab| AttachmentKey::new(generation + 10, tab.clone()));
    let decision = if state.has_attachments() {
        let old = state.attached_key().unwrap().clone();
        let cleanup = state
            .stage_refresh_membership_cleanup(generation, prepared, deadlines)
            .unwrap();
        assert_eq!(cleanup.old(), &old);
        let prepared = state
            .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
            .unwrap();
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, desired)
            .unwrap()
            .commit()
    } else {
        state
            .prepare_refresh_membership_install(generation, prepared, desired, Some(deadlines))
            .unwrap()
            .commit()
    };
    if let MembershipInstallDecision::Attach(target) = decision {
        let key = target.selected().key();
        assert_eq!(target.deadlines(), deadlines);
        assert!(state.install_projection_t(key));
        assert!(state.record_setup_configuration_t(key, ControllerMode::ReadOnly, None));
        assert!(state.stage_drawn_frame_t(key, FrameIdentity::new(generation + 30)));
        assert!(state.mark_feedback_accepted_t(key, FrameIdentity::new(generation + 30)));
        assert!(state.publish_ready_t(
            key,
            FrameIdentity::new(generation + 30),
            ControllerMode::ReadOnly,
            None
        ));
    }
    assert!(state.context_selection_deadlines().is_none());
    assert!(state.pending_refresh_generation().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn context_selection_uses_typed_owner_order_and_empty_can_be_entered_and_left() {
    let mut state = context_state(1);
    let snapshot = contexts(1, 1);
    assert_eq!(state.view().context_count(), 3);
    assert_eq!(state.view().active_context_position(), Some(1));
    assert_eq!(state.view().context_at(0).unwrap().project_label(), None);
    for (generation, next, position, context) in [
        (1, true, 2, 72),
        (2, true, 3, 74),
        (3, true, 1, 2),
        (4, false, 3, 74),
        (5, false, 2, 72),
    ] {
        assert!(state.start_context_selection(generation, next, deadlines()));
        assert!(!state.start_context_selection(generation + 100, next, deadlines()));
        finish_selection(&mut state, snapshot.clone(), generation);
        assert_eq!(state.cache().terminal_context_id(), &context_id(context));
        assert_eq!(state.view().active_context_position(), Some(position));
        if context == 74 {
            assert_eq!(state.phase_kind(), SessionPhaseKind::Empty);
            assert_eq!(state.desired(), None);
        } else {
            assert_eq!(state.active().unwrap().index(), 0);
        }
    }
}

#[test]
fn context_selection_retains_every_known_create_provenance_on_reentry() {
    for provenance in [
        CreateTerminalProvenance::Pending,
        CreateTerminalProvenance::OutcomeUnknown,
        CreateTerminalProvenance::Started,
        CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed),
        CreateTerminalProvenance::Unavailable(
            WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        ),
    ] {
        let mut state = context_state(0);
        state.provenance[0] = provenance.clone();
        assert!(state.start_context_selection(1, true, deadlines()));
        finish_selection(&mut state, contexts(0, 1), 1);
        assert_eq!(state.phase_kind(), SessionPhaseKind::Empty);
        assert!(state.start_context_selection(2, false, deadlines()));
        finish_selection(&mut state, contexts(0, 1), 2);
        assert_eq!(state.provenance(0), Some(&provenance));
        assert_eq!(state.has_attachments(), provenance.attachable());
    }
}

#[test]
fn missing_target_is_refused_without_hiding_or_detaching_current_terminal() {
    let mut snapshot = contexts(1, 1);
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.workspaces.remove(0);
    snapshot.terminal_contexts.remove(1);
    let cache = state.cache().clone();
    let desired = state.desired().cloned();
    let attached = state.attached_key().cloned();
    assert!(state.start_context_selection(1, true, deadlines()));
    let prepared = state.prepare_refresh(1, WorkbenchSnapshotOutcome::Available { snapshot });
    assert!(matches!(prepared, PreparedRefresh::DefiniteFailure { .. }));
    assert_eq!(
        state.apply_prepared_refresh(prepared, std::time::Instant::now()),
        RefreshDecision::Continue
    );
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.desired(), desired.as_ref());
    assert_eq!(state.attached_key(), attached.as_ref());
    assert!(state.context_selection_deadlines().is_none());
    assert!(state.start_context_selection(2, false, deadlines()));
}

#[test]
fn context_selection_rejects_owner_rebinding_and_current_authority_regression() {
    for wrong_owner in [true, false] {
        let mut state = context_state(1);
        let old = state.cache().clone();
        assert!(state.start_context_selection(1, true, deadlines()));
        let mut snapshot = contexts(1, if wrong_owner { 2 } else { 0 });
        if wrong_owner {
            snapshot.navigation.workspaces[0].workspace_id =
                WorkbenchWorkspaceId::parse(&id(69)).unwrap();
        }
        let prepared = state.prepare_refresh(1, WorkbenchSnapshotOutcome::Available { snapshot });
        if wrong_owner {
            assert!(matches!(prepared, PreparedRefresh::DefiniteFailure { .. }));
        } else {
            assert!(matches!(prepared, PreparedRefresh::Divergent { .. }));
        }
        state.apply_prepared_refresh(prepared, std::time::Instant::now());
        assert_eq!(state.cache(), &old);
    }
}

#[test]
fn workspace_refresh_preserves_context_and_commits_navigation_from_the_same_revision() {
    let mut state = context_state(1);
    assert!(state.start_context_selection(1, true, deadlines()));
    finish_selection(&mut state, contexts(1, 1), 1);
    let mut snapshot = contexts(1, 2);
    snapshot.navigation.projects[0].label = "renamed project".into();
    snapshot.terminal_contexts[1].tabs[0].label = "renamed tab".into();
    assert!(state.start_pending_refresh(2));
    let prepared = state.prepare_refresh(2, WorkbenchSnapshotOutcome::Available { snapshot });
    assert_eq!(
        state.apply_prepared_refresh(prepared, std::time::Instant::now()),
        RefreshDecision::Continue
    );
    assert_eq!(state.cache().terminal_context_id(), &context_id(72));
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
    assert_eq!(
        state.view().context_at(1).unwrap().project_label(),
        Some("renamed project")
    );
    assert_eq!(state.view().tab(0).unwrap().tab_label(), "renamed tab");
}

#[test]
fn context_selection_close_confirmation_and_mutation_are_busy_without_allocating_intent() {
    let mut state = context_state(1);
    state.open_close_confirmation().unwrap();
    assert!(!state.start_context_selection(1, true, deadlines()));
    assert!(state.context_selection_deadlines().is_none());
    state.cancel_close_confirmation();
    let operation = state
        .plan_move(
            workbench_request_id(300),
            orchbee_workbench_protocol::WorkbenchTabMoveDirection::Next,
            Instant::now() + OPERATION_TOTAL,
        )
        .unwrap();
    assert!(state.begin_move(operation));
    assert!(!state.start_context_selection(1, true, deadlines()));
    assert!(state.context_selection_deadlines().is_none());
}

#[test]
fn stale_context_result_cannot_install_and_retirement_clears_intent() {
    let mut state = context_state(1);
    let old = state.cache().clone();
    assert!(state.start_context_selection(1, true, deadlines()));
    let prepared = state.prepare_refresh(
        2,
        WorkbenchSnapshotOutcome::Available {
            snapshot: contexts(1, 1),
        },
    );
    assert!(matches!(prepared, PreparedRefresh::Stale));
    assert_eq!(
        state.apply_prepared_refresh(prepared, std::time::Instant::now()),
        RefreshDecision::Continue
    );
    assert!(state.context_selection_deadlines().is_some());
    state.retire(CarrierFailure::Closed);
    assert!(state.context_selection_deadlines().is_none());
    assert_eq!(state.cache(), &old);
}

#[test]
fn context_selection_retains_exact_target_when_same_label_workspace_is_inserted_before_it() {
    let mut state = context_state(1);
    assert!(state.start_context_selection(1, true, deadlines()));
    let mut candidate = contexts(1, 2);
    candidate.navigation.workspaces.insert(
        0,
        Workspace {
            project_id: Some(candidate.navigation.projects[0].project_id.clone()),
            label: "same label".into(),
            primary_directory_id: None,
            workspace_id: WorkbenchWorkspaceId::parse(&id(69)).unwrap(),
            terminal_context_id: context_id(68),
        },
    );
    candidate.terminal_contexts.insert(
        1,
        TerminalContext {
            groups: Vec::new(),
            tabs: Vec::new(),
            terminal_context_id: context_id(68),
        },
    );
    finish_selection(&mut state, candidate, 1);
    assert_eq!(state.cache().terminal_context_id(), &context_id(72));
    assert_eq!(state.view().active_context_position(), Some(3));
    assert_eq!(state.view().tab_count(), 1);
}

#[test]
fn retained_context_reentry_rejects_same_revision_label_change_and_later_immutable_rebinding() {
    for rebind in [false, true] {
        let mut state = context_state(0);
        assert!(state.start_context_selection(1, true, deadlines()));
        finish_selection(&mut state, contexts(0, 1), 1);
        assert!(state.start_context_selection(2, false, deadlines()));
        let mut candidate = contexts(0, if rebind { 2 } else { 1 });
        if rebind {
            let PanelNode::Panel {
                primary_presentation,
                ..
            } = &mut candidate.terminal_contexts[0].tabs[0].root
            else {
                unreachable!()
            };
            primary_presentation.binding.as_mut().unwrap().terminal_id = terminal_id(999);
        } else {
            candidate.terminal_contexts[0].tabs[0].label =
                "contradictory same-revision label".into();
        }
        assert!(matches!(
            state.prepare_refresh(
                2,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: candidate
                }
            ),
            PreparedRefresh::Divergent { .. }
        ));
        assert_eq!(state.cache().terminal_context_id(), &context_id(72));
    }
}

#[test]
fn intermediate_context_switch_cannot_prune_known_facts_when_context_id_changes_owner() {
    for provenance in [
        CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed),
        CreateTerminalProvenance::Unavailable(
            WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        ),
    ] {
        let mut state = context_state(1);
        assert!(state.start_context_selection(1, true, deadlines()));
        finish_selection(&mut state, contexts(1, 1), 1);
        state.provenance[0] = provenance;
        assert!(state.start_context_selection(2, true, deadlines()));
        finish_selection(&mut state, contexts(1, 1), 2);
        let mut rebound = contexts(1, 2);
        rebound.navigation.workspaces[0].workspace_id =
            WorkbenchWorkspaceId::parse(&id(69)).unwrap();
        assert!(state.start_context_selection(3, true, deadlines()));
        finish_selection(&mut state, rebound.clone(), 3);
        assert_eq!(state.view().active_context_position(), Some(1));
        assert!(state.start_context_selection(4, true, deadlines()));
        assert!(matches!(
            state.prepare_refresh(4, WorkbenchSnapshotOutcome::Available { snapshot: rebound }),
            PreparedRefresh::Divergent { .. }
        ));
        assert_eq!(state.view().active_context_position(), Some(1));
    }
}

fn restored_workspace(revision: u64) -> Snapshot {
    let mut snapshot = contexts(1, revision);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut snapshot.terminal_contexts[1].tabs[0].root
    else {
        panic!("workspace panel")
    };
    primary_presentation.presentation_id =
        orchbee_workbench_protocol::TerminalPresentationId::parse(&id(9000)).unwrap();
    primary_presentation.binding.as_mut().unwrap().terminal_id = terminal_id(9001);
    snapshot
}

#[test]
fn restored_workspace_selected_panel_requires_cleanup_then_fresh_attachment_without_old_provenance()
{
    let mut state = context_state(1);
    assert!(state.start_context_selection(1, true, deadlines()));
    finish_selection(&mut state, contexts(1, 1), 1);
    let old = state.attached_key().unwrap().clone();
    let panel = state.cache().tabs()[0].panel(0).unwrap().panel_id().clone();
    assert!(state.start_pending_refresh(7));
    let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
        7,
        WorkbenchSnapshotOutcome::Available {
            snapshot: restored_workspace(2),
        },
    ) else {
        panic!("replacement requires membership cleanup")
    };
    assert!(!prepared.preserves_selection());
    assert_eq!(prepared.focus_panel_id(), Some(&panel));
    let desired = AttachmentKey::new(100, prepared.selected_tab_id().unwrap().clone());
    let cleanup = state
        .stage_refresh_membership_cleanup(7, prepared, deadlines())
        .unwrap();
    assert_eq!(cleanup.old(), &old);
    assert_eq!(
        state.cache().revision(),
        WorkbenchRevision::new(1),
        "old projection until cleanup proof"
    );
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
        .unwrap();
    let MembershipInstallDecision::Attach(target) = state
        .prepare_cleaned_membership_install(&cleanup, prepared, Some(desired))
        .unwrap()
        .commit()
    else {
        panic!("attach replacement")
    };
    assert_eq!(
        target.binding().expect("bound fixture").terminal_id(),
        &terminal_id(9001)
    );
    assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
    assert_ne!(target.selected().key(), &old);
    assert_eq!(state.cache().tabs()[0].panel(0).unwrap().panel_id(), &panel);
    assert!(
        !state.install_projection_t(&old),
        "old attachment generation cannot regain the pane"
    );
    assert!(state.install_projection_t(target.selected().key()));
    assert!(state.invariant_holds());
}

#[test]
fn restored_workspace_reentry_does_not_inherit_failed_launch_from_retained_context() {
    let mut state = context_state(1);
    assert!(state.start_context_selection(1, true, deadlines()));
    finish_selection(&mut state, contexts(1, 1), 1);
    state.provenance[0] =
        CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed);
    assert!(state.start_context_selection(2, false, deadlines()));
    finish_selection(&mut state, contexts(1, 1), 2);
    assert!(state.start_context_selection(3, true, deadlines()));
    finish_selection(&mut state, restored_workspace(2), 3);
    assert_eq!(state.provenance[0], CreateTerminalProvenance::BindingOnly);
    assert_eq!(
        state.cache().tabs()[0]
            .panel(0)
            .unwrap()
            .binding()
            .expect("bound fixture")
            .terminal_id(),
        &terminal_id(9001)
    );
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
}

#[test]
fn restored_workspace_background_tab_drops_old_launch_failure_without_detaching_selection() {
    let mut state = context_state(2);
    assert!(state.start_context_selection(1, true, deadlines()));
    finish_selection(&mut state, contexts(2, 1), 1);
    let old_key = state.attached_key().unwrap().clone();
    state.provenance[1] =
        CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed);
    let mut snapshot = contexts(2, 2);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut snapshot.terminal_contexts[1].tabs[1].root
    else {
        panic!("panel")
    };
    primary_presentation.presentation_id =
        orchbee_workbench_protocol::TerminalPresentationId::parse(&id(9000)).unwrap();
    primary_presentation.binding.as_mut().unwrap().terminal_id = terminal_id(9001);
    assert!(state.start_pending_refresh(7));
    let prepared = state.prepare_refresh(7, WorkbenchSnapshotOutcome::Available { snapshot });
    assert_eq!(
        state.apply_prepared_refresh(prepared, Instant::now()),
        RefreshDecision::Continue,
    );
    assert_eq!(state.attached_key(), Some(&old_key));
    assert_eq!(state.provenance[1], CreateTerminalProvenance::BindingOnly);
    let tab = state.cache().tabs()[1].tab_id().clone();
    let target = state
        .target_for_switch(1, AttachmentKey::new(100, tab), deadlines())
        .unwrap();
    assert_eq!(
        target.binding().expect("bound fixture").terminal_id(),
        &terminal_id(9001)
    );
    assert_eq!(target.provenance(), &CreateTerminalProvenance::BindingOnly);
    assert!(state.invariant_holds());
}

#[test]
fn restored_workspace_unselected_split_replaces_anchor_before_reusing_panel_slot() {
    let mut snapshot = contexts(1, 1);
    snapshot.terminal_contexts[0].tabs.remove(0);
    snapshot.terminal_contexts[1].tabs[0].root = split_panel_baseline().terminal_contexts[0].tabs
        [0]
    .root
    .clone();
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    assert!(state.start_context_selection(1, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 1);
    let old = state.attached_key().unwrap().clone();
    let leaves: Vec<_> = state
        .tab(0)
        .unwrap()
        .leaf_panel_ids()
        .into_iter()
        .enumerate()
        .map(|(index, panel)| (panel, index, geometry(80)))
        .collect();
    assert!(state.sync_pane_set(&leaves, Instant::now()));
    let old_peer = state.pane_by_panel(&leaves[1].0).unwrap().pane_ref();
    snapshot.revision = WorkbenchRevision::new(2);
    let PanelNode::Split { children, .. } = &mut snapshot.terminal_contexts[1].tabs[0].root else {
        panic!("split")
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[1].node
    else {
        panic!("peer panel")
    };
    primary_presentation.presentation_id =
        orchbee_workbench_protocol::TerminalPresentationId::parse(&id(9000)).unwrap();
    primary_presentation.binding.as_mut().unwrap().terminal_id = terminal_id(9001);
    assert!(state.start_pending_refresh(7));
    let PreparedRefresh::Membership { prepared, .. } =
        state.prepare_refresh(7, WorkbenchSnapshotOutcome::Available { snapshot })
    else {
        panic!("unselected split replacement requires anchor cleanup")
    };
    assert_eq!(prepared.focus_panel_id(), Some(old.panel()));
    let desired = AttachmentKey::new(100, prepared.selected_tab_id().unwrap().clone());
    let cleanup = state
        .stage_refresh_membership_cleanup(7, prepared, deadlines())
        .unwrap();
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
        .unwrap();
    let MembershipInstallDecision::Attach(target) = state
        .prepare_cleaned_membership_install(&cleanup, prepared, Some(desired))
        .unwrap()
        .commit()
    else {
        panic!("fresh anchor")
    };
    assert_eq!(target.selected().key().panel(), old.panel());
    assert_ne!(target.selected().key().anchor(), old.anchor());
    assert!(state.live_pane_record(&old_peer).is_none());
    assert!(state.sync_pane_set(&leaves, Instant::now()));
    let peer = state.pane_by_panel(&leaves[1].0).unwrap();
    assert_eq!(peer.binding.terminal_id(), &terminal_id(9001));
    assert_ne!(peer.pane_ref(), old_peer);
    assert!(state.invariant_holds());
}
