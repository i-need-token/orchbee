//! Overlay return and close ownership use canonical membership, never stream loss.
use super::*;
use crate::workbench_projection::ClientContextLocation;
use orchbee_workbench_protocol::{ClientOverlayMember, ClientViewIntent};

fn add_leaf(root: &mut PanelNode, seed: u128) {
    *root = PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: root.clone(),
                slot_id: WorkbenchPanelSlotId::parse(&id(seed + 1)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: panel_id(seed + 10),
                    primary_presentation: Presentation {
                        custom_label: None,
                        binding: Some(PresentationBinding {
                            worker_id: worker_id(seed + 11),
                            terminal_id: terminal_id(seed + 12),
                        }),
                        label: "overlay".into(),
                        presentation_id: presentation_id(seed + 13),
                        launch_failure: None,
                        agent_not_resumed: None,
                        agent_start_failure: None,
                        launch_program: None,
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(seed + 14)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&id(seed + 20)).unwrap(),
    };
}

fn baseline() -> Snapshot {
    let mut snapshot = split_panel_baseline();
    add_leaf(&mut snapshot.terminal_contexts[0].tabs[0].root, 4_000);
    snapshot
}

fn attach(state: &mut WorkbenchClientState, index: usize) -> PaneRef {
    let active = state.active().unwrap();
    let panel = state
        .tab(active.index())
        .unwrap()
        .panel(index)
        .unwrap()
        .panel_id()
        .clone();
    let pane = PaneKey::new(state.desired().unwrap().anchor().clone(), panel);
    state.install_unattached_pane(&SelectedTab::new_panel(pane.clone(), active.index(), index));
    let pane = state.pane_ref_for(&pane).unwrap();
    assert!(state.settle_attached_event_t(&pane).is_some());
    assert!(state.record_setup_configuration_t(&pane, controlling(), Some(geometry(80))));
    pane
}

fn intent(state: &WorkbenchClientState, index: usize) -> ClientViewIntent {
    let active = state.active().unwrap();
    let tab = state.tab(active.index()).unwrap();
    let panel = tab.panel(index).unwrap();
    let source = tab.panel(active.panel_index()).unwrap();
    let member = state.selected_presentation(source);
    ClientViewIntent::Overlay {
        terminal_context_id: state.cache().terminal_context_id().unwrap().clone(),
        tab_id: tab.tab_id().clone(),
        panel_id: panel.panel_id().clone(),
        presentation_id: panel.presentation_id().clone(),
        expected_binding: PresentationBinding {
            worker_id: panel.binding().unwrap().worker_id().clone(),
            terminal_id: panel.binding().unwrap().terminal_id().clone(),
        },
        return_to: ClientOverlayMember {
            panel_id: source.panel_id().clone(),
            presentation_id: member.presentation_id().clone(),
            binding: member.binding().map(|b| PresentationBinding {
                worker_id: b.worker_id().clone(),
                terminal_id: b.terminal_id().clone(),
            }),
        },
        previously_zoomed: state.zoom_active(),
    }
}

fn open(state: &mut WorkbenchClientState, index: usize) -> PaneRef {
    let pane = attach(state, index);
    state
        .register_plugin_overlay(&intent(state, index))
        .unwrap();
    let target = state
        .plan_panel_focus_local_at(pane.pane().panel())
        .unwrap();
    assert!(state.move_panel_focus_local(target));
    if !state.zoom_active() {
        assert!(state.toggle_zoom());
    }
    state.arm_overlay_zoom(pane.pane().panel());
    pane
}

fn remove(snapshot: &Snapshot, ids: &[WorkbenchPanelId]) -> Snapshot {
    fn prune(node: &PanelNode, ids: &[WorkbenchPanelId]) -> Option<PanelNode> {
        match node {
            PanelNode::Panel { panel_id, .. } => (!ids.contains(panel_id)).then(|| node.clone()),
            PanelNode::Split {
                children,
                orientation,
                split_id,
                ..
            } => match (prune(&children[0].node, ids), prune(&children[1].node, ids)) {
                (Some(a), Some(b)) => Some(PanelNode::Split {
                    ratio: Default::default(),
                    children: Box::new([
                        PanelSlot {
                            node: a,
                            slot_id: children[0].slot_id.clone(),
                        },
                        PanelSlot {
                            node: b,
                            slot_id: children[1].slot_id.clone(),
                        },
                    ]),
                    orientation: *orientation,
                    split_id: split_id.clone(),
                }),
                (a, b) => a.or(b),
            },
        }
    }
    let mut fresh = snapshot.clone();
    fresh.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    fresh.terminal_contexts[0].tabs[0].root =
        prune(&snapshot.terminal_contexts[0].tabs[0].root, ids).unwrap();
    fresh
}

fn refresh(state: &mut WorkbenchClientState, snapshot: Snapshot) -> Option<AttachTarget> {
    let generation = snapshot.revision.value() + 100;
    assert!(state.start_pending_refresh(generation));
    match state.prepare_refresh(generation, WorkbenchSnapshotOutcome::Available { snapshot }) {
        PreparedRefresh::Membership {
            generation,
            prepared,
        } => {
            let old = state.desired().unwrap().anchor().clone();
            let cleanup = state
                .stage_refresh_membership_cleanup(
                    generation,
                    prepared,
                    OperationDeadlines::from_sample(Instant::now()).unwrap(),
                )
                .unwrap();
            let prepared = state
                .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
                .unwrap();
            let fresh = AttachmentKey::new(old.generation() + 1, old.tab_id().clone());
            match state
                .prepare_cleaned_membership_install(&cleanup, prepared, Some(fresh))
                .unwrap()
                .commit()
            {
                MembershipInstallDecision::Attach(target) => Some(target),
                _ => None,
            }
        }
        prepared => {
            assert!(matches!(
                state.apply_prepared_refresh(prepared, Instant::now()),
                RefreshDecision::Continue
            ));
            None
        }
    }
}

#[test]
fn overlay_removal_restores_the_source_instead_of_the_nearest_leaf_and_prior_zoom() {
    for zoomed in [false, true] {
        let baseline = baseline();
        let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
        let source = state.focus_panel_id().unwrap().clone();
        if zoomed {
            assert!(state.toggle_zoom());
        }
        let overlay = open(&mut state, 2);
        let target = refresh(
            &mut state,
            remove(&baseline, &[overlay.pane().panel().clone()]),
        )
        .unwrap();
        assert_eq!(target.selected().panel_index(), 0);
        assert_eq!(target.selected().key().panel(), &source);
        assert_eq!(state.zoom_active(), zoomed);
        assert!(state.plugin_overlays.is_empty());
        assert!(state.invariant_holds());
    }
}

#[test]
fn overlay_return_preserves_a_bound_child_presentation_through_membership_cleanup() {
    let mut baseline = baseline();
    let PanelNode::Split { children, .. } = &mut baseline.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    let PanelNode::Split { children, .. } = &mut children[0].node else {
        panic!()
    };
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut children[0].node
    else {
        panic!()
    };
    child_presentations.push(Presentation {
        presentation_id: presentation_id(9_000),
        label: "child".into(),
        custom_label: None,
        binding: Some(PresentationBinding {
            worker_id: worker_id(9_001),
            terminal_id: terminal_id(9_002),
        }),
        launch_failure: None,
        agent_not_resumed: None,
        agent_start_failure: None,
        launch_program: None,
    });
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let marker = super::presentations::choose(&mut state, 2);
    install_switch_candidate(&mut state, &marker);
    super::presentations::finish(&mut state);
    let overlay = open(&mut state, 2);
    let target = refresh(
        &mut state,
        remove(&baseline, &[overlay.pane().panel().clone()]),
    )
    .unwrap();
    assert_eq!(target.binding().unwrap().terminal_id(), &terminal_id(9_002));
    assert_eq!(
        state
            .selected_presentation(state.tab(0).unwrap().panel(0).unwrap())
            .presentation_id(),
        &presentation_id(9_000)
    );
}

#[test]
fn overlay_background_context_removal_restores_its_memory_without_switching_contexts() {
    let mut snapshot = super::context_selection::contexts(1, 1);
    snapshot.terminal_contexts[0].tabs[0].root =
        baseline().terminal_contexts[0].tabs[0].root.clone();
    let mut state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
    let source = state.focus_panel_id().unwrap().clone();
    let overlay = open(&mut state, 2);
    let overlay_tab = state.tab(0).unwrap().tab_id().clone();
    assert!(state.start_context_selection(
        1,
        true,
        OperationDeadlines::from_sample(Instant::now()).unwrap()
    ));
    super::context_selection::finish_selection(&mut state, snapshot.clone(), 1);
    let location = state.cache().location().clone();
    let focus = state.focus_panel_id().cloned();
    state.observe_overlay_snapshot(&remove(&snapshot, &[overlay.pane().panel().clone()]));
    assert_eq!(state.cache().location(), &location);
    assert_eq!(state.focus_panel_id(), focus.as_ref());
    assert!(state.plugin_overlays.is_empty());
    assert!(!state.zoomed_tabs.contains(&overlay_tab));
    assert_eq!(
        state.navigation_memory.overlay_focus(
            &{
                let fixture_context_id = snapshot.navigation.workspaces[0]
                    .terminal_context_id
                    .clone();
                ClientContextLocation::Workspace {
                    workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                        fixture_context_id.as_str(),
                    )
                    .unwrap(),
                    terminal_context_id: fixture_context_id,
                }
            },
            &overlay_tab
        ),
        Some(&source)
    );
}

#[test]
fn overlay_removal_preserves_manual_focus_and_zoom_even_with_stale_navigation_memory() {
    let baseline = baseline();
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let overlay = open(&mut state, 2);
    state.remember_current_navigation();
    let peer = attach_nonfirst_leaf(&mut state);
    let focus = state
        .plan_panel_focus_local_at(peer.pane().panel())
        .unwrap();
    assert!(state.move_panel_focus_local(focus));
    assert!(
        refresh(
            &mut state,
            remove(&baseline, &[overlay.pane().panel().clone()])
        )
        .is_none()
    );
    assert_eq!(state.focus_pane_ref(), Some(peer));
    assert!(state.zoom_active());
    assert!(state.plugin_overlays.is_empty());
}

#[test]
fn overlay_manual_zoom_changes_are_not_reverted_on_removal() {
    let baseline = baseline();
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    assert!(state.toggle_zoom());
    let overlay = open(&mut state, 2);
    assert!(state.toggle_zoom());
    refresh(
        &mut state,
        remove(&baseline, &[overlay.pane().panel().clone()]),
    );
    assert!(!state.zoom_active());
}

#[test]
fn overlay_parent_exiting_before_its_child_collapses_the_exact_return_chain() {
    let mut baseline = baseline();
    add_leaf(&mut baseline.terminal_contexts[0].tabs[0].root, 5_000);
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let source = state.focus_panel_id().unwrap().clone();
    let parent = open(&mut state, 2);
    let child = open(&mut state, 3);
    let parent_gone = remove(&baseline, &[parent.pane().panel().clone()]);
    assert!(refresh(&mut state, parent_gone.clone()).is_none());
    assert_eq!(state.focus_pane_ref(), Some(child.clone()));
    assert_eq!(state.plugin_overlays.len(), 1);
    let target = refresh(
        &mut state,
        remove(&parent_gone, &[child.pane().panel().clone()]),
    )
    .unwrap();
    assert_eq!(target.selected().key().panel(), &source);
    assert!(!state.zoom_active());
}

#[test]
fn overlay_parent_rebinding_preserves_the_child_return_chain_and_fences_stale_jobs() {
    use super::super::plugin_overlays::OverlayResult;
    let overlay_root = baseline().terminal_contexts[0].tabs[0].root.clone();
    let mut baseline = super::context_selection::contexts(1, 1);
    baseline.terminal_contexts[0].tabs[0].root = overlay_root;
    add_leaf(&mut baseline.terminal_contexts[0].tabs[0].root, 5_000);
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let source = state.focus_panel_id().unwrap().clone();
    let parent = open(&mut state, 2);
    let child = open(&mut state, 3);
    let overlay_tab = state.tab(0).unwrap().tab_id().clone();
    let deadline = Instant::now() + Duration::from_secs(1);
    let stale = state
        .begin_overlay_job(workbench_request_id(8_101), deadline)
        .unwrap();
    assert_eq!(stale.member.panel_id, *parent.pane().panel());
    assert!(state.start_context_selection(
        1,
        true,
        OperationDeadlines::from_sample(Instant::now()).unwrap()
    ));
    super::context_selection::finish_selection(&mut state, baseline.clone(), 1);
    let current_location = state.cache().location().clone();
    let current_focus = state.focus_panel_id().cloned();

    let mut rebound = baseline.clone();
    rebound.revision = WorkbenchRevision::new(2);
    let PanelNode::Split { children, .. } = &mut rebound.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    let PanelNode::Split { children, .. } = &mut children[0].node else {
        panic!()
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[1].node
    else {
        panic!()
    };
    primary_presentation.binding.as_mut().unwrap().terminal_id = terminal_id(9_999);
    state.observe_overlay_snapshot(&rebound);
    assert_eq!(state.cache().location(), &current_location);
    assert_eq!(state.focus_panel_id(), current_focus.as_ref());
    assert!(state.zoomed_tabs.contains(&overlay_tab));
    assert_eq!(state.plugin_overlays.len(), 1);

    state.finish_overlay_job(
        &stale,
        OverlayResult::CloseOwned {
            operation_id: stale.id.clone(),
        },
    );
    let pending = state
        .begin_overlay_job(workbench_request_id(8_102), deadline)
        .unwrap();
    assert_eq!(pending.member.panel_id, *child.pane().panel());
    let child_gone = remove(&rebound, &[child.pane().panel().clone()]);
    state.observe_overlay_snapshot(&child_gone);
    assert_eq!(state.cache().location(), &current_location);
    assert_eq!(state.focus_panel_id(), current_focus.as_ref());
    assert_eq!(
        state.navigation_memory.overlay_focus(
            &{
                let fixture_context_id = baseline.navigation.workspaces[0]
                    .terminal_context_id
                    .clone();
                ClientContextLocation::Workspace {
                    workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                        fixture_context_id.as_str(),
                    )
                    .unwrap(),
                    terminal_context_id: fixture_context_id,
                }
            },
            &overlay_tab
        ),
        Some(&source)
    );
    assert!(!state.zoomed_tabs.contains(&overlay_tab));
    assert!(state.plugin_overlays.is_empty());
    assert!(state.invariant_holds());
}

#[test]
fn overlay_source_loss_keeps_prior_zoom_while_normal_membership_repairs_focus() {
    let mut baseline = baseline();
    add_leaf(&mut baseline.terminal_contexts[0].tabs[0].root, 5_000);
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let source = state.focus_panel_id().unwrap().clone();
    let overlay = open(&mut state, 2);
    refresh(
        &mut state,
        remove(&baseline, &[source, overlay.pane().panel().clone()]),
    );
    assert_eq!(state.tab(0).unwrap().panel_count(), 2);
    assert!(!state.zoom_active());
    assert!(state.plugin_overlays.is_empty());
    assert!(state.invariant_holds());
}

#[test]
fn overlay_close_ownership_is_exact_fair_and_never_automatically_resubmitted() {
    use super::super::plugin_overlays::OverlayResult;
    let mut baseline = baseline();
    add_leaf(&mut baseline.terminal_contexts[0].tabs[0].root, 5_000);
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let parent = open(&mut state, 2);
    let child = open(&mut state, 3);
    let deadline = Instant::now() + Duration::from_secs(1);
    let first = state
        .begin_overlay_job(workbench_request_id(8_001), deadline)
        .unwrap();
    assert_eq!(first.member.panel_id, *parent.pane().panel());
    state.finish_overlay_job(&first, OverlayResult::Waiting);
    let second = state
        .begin_overlay_job(workbench_request_id(8_002), deadline)
        .unwrap();
    assert_eq!(second.member.panel_id, *child.pane().panel());
    state.finish_overlay_job(
        &first,
        OverlayResult::CloseOwned {
            operation_id: first.id.clone(),
        },
    );
    state.finish_overlay_job(
        &second,
        OverlayResult::CloseOwned {
            operation_id: second.id.clone(),
        },
    );
    let third = state
        .begin_overlay_job(workbench_request_id(8_003), deadline)
        .unwrap();
    assert_eq!(third.member.panel_id, *parent.pane().panel());
    state.finish_overlay_job(
        &third,
        OverlayResult::CloseOwned {
            operation_id: third.id.clone(),
        },
    );
    assert!(
        state
            .begin_overlay_job(workbench_request_id(8_004), deadline)
            .is_none()
    );
}

#[test]
fn overlay_render_draws_and_hit_tests_only_the_focused_overlay_incarnation() {
    use crate::client_view::ClientView;
    use crate::localization::ClientProjectionStatus;
    use crate::tui_hit_map::{HitMapBuilder, HitTarget};
    use crate::workbench_client::{KeyedProjection, WorkbenchClientView};
    use ratatui::{Terminal, backend::TestBackend};
    let baseline = baseline();
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let source = state.focus_pane_ref().unwrap();
    let overlay = open(&mut state, 2);
    let projections = [source, overlay.clone()].map(|pane| {
        KeyedProjection::for_test(
            pane,
            super::zoom::framed_projection(),
            ClientProjectionStatus::FrameAvailable,
        )
    });
    let view = WorkbenchClientView::test_view(
        &state,
        &projections,
        ClientProjectionStatus::FrameAvailable,
    );
    let mut terminal = Terminal::new(TestBackend::new(120, 34)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &ClientView::new(120),
                crate::localization::Locale::EnUs,
                Some(view),
                &crate::tui_theme::Theme::rgb_test_dark(),
                &mut hits,
            );
        })
        .unwrap();
    let map = hits.finish();
    let mut count = 0;
    for row in 0..34 {
        for column in 0..120 {
            if let Some(HitTarget::Pane { pane, .. }) = map.hit_target_at(column, row) {
                assert_eq!(pane, &overlay);
                count += 1;
            }
        }
    }
    assert!(count > 0);
}
