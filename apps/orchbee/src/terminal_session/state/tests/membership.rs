use super::*;
use crate::terminal_session::workbench_projection::NativeContextLocation;
use orchbee_workbench_protocol::{Project, WorkbenchProjectId, WorkbenchWorkspaceId, Workspace};

fn removal_snapshot(tab_count: usize, revision: u64) -> Snapshot {
    Snapshot {
        navigation: Navigation {
            workspace_order: Vec::new(),
            projects: Vec::new(),
            standalone_terminal_context_id: context_id(9),
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(revision),
        site_id: site_id(1),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            tabs: (0..tab_count).map(removal_tab).collect(),
            terminal_context_id: context_id(9),
        }],
    }
}

fn removal_tab(index: usize) -> Tab {
    let seed = 5_000 + u128::try_from(index).unwrap() * 10;
    let label = (index + 1).to_string();
    Tab {
        automatic_label: false,
        group_id: None,
        label: label.clone(),
        pinned: false,
        root: PanelNode::Panel {
            right_click: Default::default(),
            child_presentations: Vec::new(),
            panel_id: panel_id(seed + 1),
            primary_presentation: Presentation {
                custom_label: None,
                binding: Some(PresentationBinding {
                    terminal_id: terminal_id(seed + 4),
                    worker_id: worker_id(seed + 3),
                }),
                label,
                presentation_id: presentation_id(seed + 2),
            },
        },
        tab_id: tab_id(seed),
    }
}

#[test]
fn removed_context_refresh_classifies_removed_and_mints_a_bound_permit() {
    let mut state = ready(1);
    let removed = state.cache().location().clone();
    assert!(state.start_pending_refresh(7));
    let PreparedRefresh::Membership {
        generation,
        prepared,
    } = state.prepare_refresh(
        7,
        WorkbenchSnapshotOutcome::Available {
            snapshot: removal_snapshot(2, 2),
        },
    )
    else {
        panic!("a peer workspace close classifies as a removal candidate")
    };
    assert_eq!(generation, 7);
    let permit = prepared.context_removal().expect("Removed mints a permit");
    assert_eq!(permit.generation(), 7);
    assert_eq!(permit.removed(), &removed);
    assert!(!prepared.preserves_selection());
    assert_eq!(prepared.selected_tab_id(), Some(&tab_id(5000)));
    assert!(state.invariant_holds());
}

#[test]
fn removed_context_with_live_viewers_stages_cleanup_before_landing() {
    let mut state = ready(1);
    let old = state.desired().unwrap().clone();
    assert!(state.start_pending_refresh(3));
    let PreparedRefresh::Membership {
        generation,
        prepared,
    } = state.prepare_refresh(
        3,
        WorkbenchSnapshotOutcome::Available {
            snapshot: removal_snapshot(1, 2),
        },
    )
    else {
        panic!("removal candidate")
    };
    let cleanup = state
        .stage_refresh_membership_cleanup(
            generation,
            prepared,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        state.phase,
        SessionPhase::ReconcilingMembership(_)
    ));
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
        .unwrap();
    assert!(prepared.context_removal().is_some());
    let landing = AttachmentKey::new(
        old.generation() + 1,
        prepared.selected_tab_id().unwrap().clone(),
    );
    state
        .prepare_cleaned_membership_install(&cleanup, prepared, Some(landing.clone()))
        .unwrap()
        .commit();
    assert_eq!(
        state.cache().location(),
        &NativeContextLocation::Standalone(context_id(9))
    );
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&landing));
    assert!(state.retained_contexts.is_empty());
    assert!(state.pending_membership.is_none());
    assert!(state.invariant_holds());
}

#[test]
fn removed_context_without_viewers_installs_directly_and_never_reinserts_the_removed_cache() {
    let mut state = state(1);
    state.phase = SessionPhase::SelectedWithoutAttachment(state.active().unwrap().clone());
    let old = state.desired().unwrap().clone();
    assert!(state.start_pending_refresh(4));
    let PreparedRefresh::Membership {
        generation,
        prepared,
    } = state.prepare_refresh(
        4,
        WorkbenchSnapshotOutcome::Available {
            snapshot: removal_snapshot(1, 2),
        },
    )
    else {
        panic!("removal candidate")
    };
    let landing = AttachmentKey::new(
        old.generation() + 1,
        prepared.selected_tab_id().unwrap().clone(),
    );
    state
        .prepare_refresh_membership_install(
            generation,
            prepared,
            Some(landing.clone()),
            Some(OperationDeadlines::from_sample(Instant::now()).unwrap()),
        )
        .unwrap()
        .commit();
    assert_eq!(
        state.cache().location(),
        &NativeContextLocation::Standalone(context_id(9))
    );
    // The deleted context is never parked back as a stale zombie entry.
    assert!(state.retained_contexts.is_empty());
    assert!(state.invariant_holds());
}

#[test]
fn removal_classification_diverges_on_uncertain_provenance_malformed_navigation_and_wrong_site() {
    let mut uncertain = ready(1);
    uncertain.provenance[0] = CreateTerminalProvenance::OutcomeUnknown;
    assert!(uncertain.start_pending_refresh(5));
    assert!(matches!(
        uncertain.prepare_refresh(
            5,
            WorkbenchSnapshotOutcome::Available {
                snapshot: removal_snapshot(1, 2)
            }
        ),
        PreparedRefresh::Divergent { generation: 5 }
    ));

    let mut malformed = ready(1);
    assert!(malformed.start_pending_refresh(5));
    let mut broken = removal_snapshot(1, 2);
    broken.navigation.standalone_terminal_context_id = context_id(2);
    assert!(matches!(
        malformed.prepare_refresh(5, WorkbenchSnapshotOutcome::Available { snapshot: broken }),
        PreparedRefresh::Divergent { generation: 5 }
    ));

    let mut wrong_site = ready(1);
    assert!(wrong_site.start_pending_refresh(5));
    let mut wrong = removal_snapshot(1, 2);
    wrong.site_id = site_id(77);
    assert!(matches!(
        wrong_site.prepare_refresh(5, WorkbenchSnapshotOutcome::Available { snapshot: wrong }),
        PreparedRefresh::Divergent { generation: 5 }
    ));

    let mut regressed = ready_with_cache(projected(1, 2));
    assert!(regressed.start_pending_refresh(5));
    assert!(matches!(
        regressed.prepare_refresh(
            5,
            WorkbenchSnapshotOutcome::Available {
                snapshot: removal_snapshot(1, 1)
            }
        ),
        PreparedRefresh::Divergent { generation: 5 }
    ));

    // A relocated-but-alive context is never a removal: the session's
    // context id is absent from the standalone slot yet still carried by a
    // workspace — the removed_context_landing workspace-carrier check must
    // refuse the permit. The snapshot is otherwise structurally valid so
    // this case cannot pass on an unrelated early rejection.
    let mut moved = removal_snapshot(1, 2);
    moved.navigation.projects.push(Project {
        directories: Vec::new(),
        label: "peer project".into(),
        project_id: WorkbenchProjectId::parse(&id(80)).unwrap(),
        workspaces: vec![Workspace {
            label: "moved workspace".into(),
            primary_directory_id: None,
            workspace_id: WorkbenchWorkspaceId::parse(&id(81)).unwrap(),
            terminal_context_id: context_id(2),
        }],
    });
    moved.terminal_contexts.push(TerminalContext {
        groups: Vec::new(),
        tabs: vec![removal_tab(40)],
        terminal_context_id: context_id(2),
    });
    let mut relocated = ready(1);
    assert!(relocated.start_pending_refresh(5));
    assert!(matches!(
        relocated.prepare_refresh(5, WorkbenchSnapshotOutcome::Available { snapshot: moved }),
        PreparedRefresh::Divergent { generation: 5 }
    ));
}

#[test]
fn busy_structural_guards_block_permit_minting() {
    let mut state = ready(1);
    state.open_panel_close_confirmation().unwrap();
    assert!(state.start_pending_refresh(6));
    assert!(matches!(
        state.prepare_refresh(
            6,
            WorkbenchSnapshotOutcome::Available {
                snapshot: removal_snapshot(1, 2)
            }
        ),
        PreparedRefresh::Divergent { generation: 6 }
    ));
}

#[test]
fn candidate_first_create_unavailable_keeps_exact_old_proof_but_is_never_ready() {
    let (mut state, operation) = committed_create_state();
    let cache = state.cache().clone();
    let desired = state.desired().cloned();
    let projection = state.projection_key().cloned();
    let frame = state.focus_accepted_frame().clone();
    let prepared = state.prepare_create_snapshot(
        &operation,
        WorkbenchSnapshotOutcome::Unavailable {
            reason:
                orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    );
    assert_eq!(
        state.apply_prepared_create_snapshot(prepared),
        CreateProjectionDecision::ProjectionUnavailable
    );
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.desired(), desired.as_ref());
    assert_eq!(state.projection_key(), projection.as_ref());
    assert_eq!(state.focus_accepted_frame(), frame);
    assert!(state.ready().is_none());
    assert_eq!(
        state.plan_selection(ClientAction::SelectNextTab),
        SelectionPlan::Busy
    );
    assert!(state.phase_allows_refresh());
    assert!(state.invariant_holds());
    let next = create_operation(&state, 3, 21, "3");
    assert!(!state.begin_create(next));
    state.retire(CarrierFailure::Deadline);
    assert!(state.committed_target().is_some());
    assert!(state.invariant_holds());
}

#[test]
fn known_create_candidate_is_validated_before_any_attachment_or_cache_change() {
    let (mut state, operation) = committed_create_state();
    let cache = state.cache().clone();
    let old = state.desired().cloned();
    let attached = state.attached_key().cloned();
    let prepared = state.prepare_create_snapshot(
        &operation,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2, Some("2")),
        },
    );
    assert!(matches!(
        prepared,
        create_membership::PreparedCreateSnapshot::Available {
            projection: super::super::super::workbench_projection::ValidatedCreatedProjection {
                created_index: Some(1),
                ..
            },
            ..
        }
    ));
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.desired(), old.as_ref());
    assert_eq!(state.attached_key(), attached.as_ref());
    let prepared = state.prepare_create_snapshot(
        &operation,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(0, 3, None),
        },
    );
    assert!(matches!(
        prepared,
        create_membership::PreparedCreateSnapshot::Available {
            projection: super::super::super::workbench_projection::ValidatedCreatedProjection {
                created_index: None,
                ..
            },
            ..
        }
    ));
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.attached_key(), attached.as_ref());

    let prepared = state.prepare_create_snapshot(
        &operation,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2, Some("2")),
        },
    );
    state.retire(CarrierFailure::Deadline);
    assert_eq!(
        state.apply_prepared_create_snapshot(prepared),
        CreateProjectionDecision::Retire
    );
    assert_eq!(state.cache(), &cache);
}

#[test]
fn empty_create_anchor_retains_known_commit_and_refusal_restores_no_viewer_origin() {
    for committed in [false, true] {
        let mut state = state(1);
        state.phase = SessionPhase::SelectedWithoutAttachment(state.active().unwrap().clone());
        let prepared = state
            .prepare_membership_candidate(projected(0, 2), None, None, None, None)
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
    let frame = state.focus_accepted_frame().clone();
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
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
        .unwrap();
    drop(
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, None)
            .unwrap(),
    );
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.attached_key(), Some(&old));
    assert_eq!(state.focus_accepted_frame(), frame);
    assert!(state.pending_membership.is_some());
    assert!(state.invariant_holds());
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
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
        .prepare_membership_candidate(expected.clone(), None, Some(0), None, None)
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
        .prepare_membership_candidate(projected(0, 2), None, Some(0), None, None)
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
    let frame = state.focus_accepted_frame().clone();
    let geometry = state.focus_last_accepted_geometry().clone();
    let prepared = state
        .prepare_membership_candidate(projected(1, 2), Some(desired.tab_id()), Some(0), None, None)
        .unwrap();
    assert_eq!(
        state
            .prepare_membership_install(prepared, Some(desired.anchor().clone()), None)
            .unwrap()
            .commit(),
        MembershipInstallDecision::Preserved
    );
    assert_eq!(state.desired(), Some(&desired));
    assert_eq!(state.focus_accepted_frame(), frame);
    assert_eq!(state.focus_last_accepted_geometry(), geometry);
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
            desired.as_ref().map(|pane| pane.anchor().tab_id()),
            Some(0),
            None,
            None,
        )
        .unwrap();
    drop(
        state
            .prepare_membership_install(
                prepared,
                desired.as_ref().map(|pane| pane.anchor().clone()),
                None,
            )
            .unwrap(),
    );
    assert_eq!(state.cache(), &cache);
    assert_eq!(state.desired(), desired.as_ref());
    assert!(state.ready().is_some());
    assert!(state.invariant_holds());
}
