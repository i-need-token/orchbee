use super::workspace_setup::{finish, workspace_snapshot, workspace_state};
use super::*;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::terminal_session::state::workspace_setup::{
    NativeLocationLabelStatus as Status, NativeLocationLabelTarget as Target,
    NativeSetupEdit as Edit, NativeSetupIntent, NativeSetupStage, NativeSetupStatus,
    SetupOperation, SetupWorkerEvidence,
};
use crate::terminal_session::workbench_projection::{
    NativeContextLocation, project_native_context_tabs,
};
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchProjectId, WorkbenchSnapshotUnavailableReason, WorkbenchWorkspaceId,
};
fn location() -> NativeContextLocation {
    NativeContextLocation::Workspace {
        workspace_id: WorkbenchWorkspaceId::parse(&id(71)).unwrap(),
        terminal_context_id: context_id(72),
    }
}
fn live(revision: u64, count: usize) -> NativeSessionState {
    ready_with_cache(
        project_native_context_tabs(&workspace_snapshot(revision, count), &location()).unwrap(),
    )
}

#[test]
fn workspace_rename_refuses_blank_names_preserving_draft_and_canonical_label() {
    for value in ["", "   ", "\u{a0}\u{2003}\u{3000}"] {
        let mut state = live(1, 1);
        let baseline = state.cache().clone();
        let desired = state.desired().cloned();
        let token = state
            .open_setup_draft(NativeSetupIntent::RenameLocation(Target::Workspace))
            .unwrap();
        state.acknowledge_setup_draw(Some(token));
        state.edit_setup_draft(token, Edit::Clear);
        state.edit_setup_draft(token, Edit::Append(value));
        assert!(
            state
                .plan_setup(
                    token,
                    WorkbenchRequestId::parse(&id(999)).unwrap(),
                    1,
                    OperationDeadlines::from_sample(Instant::now()).unwrap()
                )
                .is_none()
        );
        assert_eq!(state.setup_draft().unwrap().text(), value);
        assert!(state.setup_draft().unwrap().interactive());
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.desired(), desired.as_ref());
        state.edit_setup_draft(token, Edit::Append("资料 Notes"));
        let accepted = state
            .plan_setup(
                token,
                WorkbenchRequestId::parse(&id(999)).unwrap(),
                1,
                OperationDeadlines::from_sample(Instant::now()).unwrap(),
            )
            .unwrap();
        assert!(
            matches!(accepted.command.action, WorkbenchAction::WorkspaceRename { label, .. }
            if label.as_str() == format!("{value}资料 Notes"))
        );
    }
}

fn operation(
    state: &mut NativeSessionState,
    target: Target,
    replacement: Option<&str>,
    generation: u64,
) -> SetupOperation {
    let token = state
        .open_setup_draft(NativeSetupIntent::RenameLocation(target))
        .unwrap();
    assert_eq!(
        state.setup_draft().unwrap().stage(),
        match target {
            Target::Project => NativeSetupStage::ProjectRename,
            Target::Workspace => NativeSetupStage::WorkspaceRename,
        }
    );
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let request = WorkbenchRequestId::parse(&id(900 + u128::from(generation))).unwrap();
    assert!(
        state
            .plan_setup(token, request.clone(), generation, deadlines)
            .is_none()
    );
    state.acknowledge_setup_draw(Some(token));
    if let Some(label) = replacement {
        state.edit_setup_draft(token, Edit::Clear);
        state.edit_setup_draft(token, Edit::Append(label));
    }
    state
        .plan_setup(token, request, generation, deadlines)
        .unwrap()
}
fn receipt(target: Target, revision: u64, unchanged: bool) -> WorkbenchSetupCommit {
    match target {
        Target::Project => WorkbenchSetupCommit::ProjectRenamed {
            revision: WorkbenchRevision::new(revision),
            project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
            unchanged,
        },
        Target::Workspace => WorkbenchSetupCommit::WorkspaceRenamed {
            revision: WorkbenchRevision::new(revision),
            workspace_id: WorkbenchWorkspaceId::parse(&id(71)).unwrap(),
            unchanged,
        },
    }
}
fn set_label(snapshot: &mut Snapshot, target: Target, label: &str) {
    match target {
        Target::Project => snapshot.navigation.projects[0].label = label.into(),
        Target::Workspace => snapshot.navigation.workspaces[0].label = label.into(),
    }
}
#[test]
fn location_rename_prefills_canonical_text_and_cancel_does_not_change_target_or_terminal() {
    for target in [Target::Project, Target::Workspace] {
        let mut state = live(1, 1);
        let baseline = state.cache().clone();
        let desired = state.desired().cloned();
        let frame = state.focus_accepted_frame().clone();
        let token = state
            .open_setup_draft(NativeSetupIntent::RenameLocation(target))
            .unwrap();
        let draft = state.setup_draft().unwrap();
        assert_eq!(
            draft.text(),
            if target == Target::Project {
                "Project"
            } else {
                "Workspace"
            }
        );
        state.acknowledge_setup_draw(Some(token));
        state.edit_setup_draft(token, Edit::Clear);
        state.edit_setup_draft(token, Edit::Append(&"🚀".repeat(512)));
        assert_eq!(state.setup_draft().unwrap().token(), token);
        assert!(!state.setup_draft().unwrap().faulted());
        state.edit_setup_draft(token, Edit::Append("x"));
        assert!(state.setup_draft().unwrap().faulted());
        state.cancel_setup_draft(token);
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.desired(), desired.as_ref());
        assert_eq!(state.focus_accepted_frame(), frame);
        assert!(!state.setup_blocks_structure());
    }
}
#[test]
fn standalone_location_rename_refuses_locally_with_the_requested_target_and_busy_does_not_claim_absence()
 {
    for target in [Target::Project, Target::Workspace] {
        let mut state = ready(1);
        assert!(
            state
                .open_setup_draft(NativeSetupIntent::RenameLocation(target))
                .is_none()
        );
        assert_eq!(
            state.setup_status(),
            Some(NativeSetupStatus::LocationLabel {
                target,
                status: Status::Unavailable
            })
        );
        assert!(state.pending_refresh_generation().is_none());
        let token = state
            .open_setup_draft(super::workspace_setup::group_intent(&state, "Group"))
            .unwrap();
        assert!(
            state
                .open_setup_draft(NativeSetupIntent::RenameLocation(target))
                .is_none()
        );
        assert_eq!(state.setup_status(), None);
        assert_eq!(state.setup_draft().unwrap().token(), token);
    }
}
#[test]
fn location_label_receipts_require_exact_owner_original_text_and_checked_revision_including_noop_at_max()
 {
    for target in [Target::Project, Target::Workspace] {
        for changed in [false, true] {
            let mut state = live(1, 1);
            let operation = operation(&mut state, target, changed.then_some("Changed"), 1);
            assert!(
                operation
                    .known_commit(receipt(target, if changed { 2 } else { 1 }, !changed))
                    .is_some()
            );
            assert!(
                operation
                    .known_commit(receipt(target, if changed { 1 } else { 2 }, changed))
                    .is_none()
            );
            assert!(
                operation
                    .known_commit(receipt(target, 3, !changed))
                    .is_none()
            );
            let other = if target == Target::Project {
                Target::Workspace
            } else {
                Target::Project
            };
            assert!(
                operation
                    .known_commit(receipt(other, if changed { 2 } else { 1 }, !changed))
                    .is_none()
            );
            let mut wrong = receipt(target, if changed { 2 } else { 1 }, !changed);
            match &mut wrong {
                WorkbenchSetupCommit::ProjectRenamed { project_id, .. } => {
                    *project_id = WorkbenchProjectId::parse(&id(99)).unwrap()
                }
                WorkbenchSetupCommit::WorkspaceRenamed { workspace_id, .. } => {
                    *workspace_id = WorkbenchWorkspaceId::parse(&id(99)).unwrap()
                }
                _ => panic!("rename receipt"),
            };
            assert!(operation.known_commit(wrong).is_none());
        }
        let mut state = live(u64::MAX, 1);
        let unchanged = operation(&mut state, target, None, 1);
        assert!(
            unchanged
                .known_commit(receipt(target, u64::MAX, true))
                .is_some()
        );
        let token = state.setup_draft().unwrap().token();
        state.cancel_setup_draft(token);
        let changed = operation(&mut state, target, Some("Changed"), 2);
        assert!(changed.known_commit(receipt(target, 0, false)).is_none());
    }
}
#[test]
fn exact_location_label_snapshot_changes_only_the_named_label_and_preserves_ready_viewer() {
    let label = "  A中𠮷🚀B  ";
    for target in [Target::Project, Target::Workspace] {
        for changed in [false, true] {
            let mut state = live(1, 1);
            let old = state.desired().cloned();
            let frame = state.focus_accepted_frame().clone();
            let operation = operation(&mut state, target, changed.then_some(label), 1);
            if changed {
                match &operation.command.action {
                    WorkbenchAction::ProjectRename { label: value, .. }
                    | WorkbenchAction::WorkspaceRename { label: value, .. } => {
                        assert_eq!(value.as_str(), label);
                    }
                    _ => panic!("a location rename request"),
                }
            }
            let known = operation
                .known_commit(receipt(target, if changed { 2 } else { 1 }, !changed))
                .unwrap();
            state.begin_setup(operation);
            state.retain_setup_worker(SetupWorkerEvidence::Known(known));
            let mut snapshot = workspace_snapshot(if changed { 2 } else { 1 }, 1);
            if changed {
                set_label(&mut snapshot, target, label);
            }
            assert_eq!(
                finish(&mut state, snapshot, 1),
                MembershipInstallDecision::Preserved
            );
            assert_eq!(state.desired(), old.as_ref());
            assert_eq!(state.attached_key(), old.as_ref());
            assert_eq!(state.focus_accepted_frame(), frame);
            assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
            assert_eq!(
                state.setup_status(),
                Some(NativeSetupStatus::LocationLabel {
                    target,
                    status: if changed {
                        Status::Renamed
                    } else {
                        Status::Unchanged
                    }
                })
            );
            assert!(state.invariant_holds());
        }
    }
}
#[test]
fn exact_location_label_snapshot_rejects_other_project_workspace_and_terminal_changes() {
    for target in [Target::Project, Target::Workspace] {
        for corruption in 0..5 {
            let mut state = live(1, 1);
            let operation = operation(&mut state, target, Some("Changed"), 1);
            let known = operation.known_commit(receipt(target, 2, false)).unwrap();
            state.begin_setup(operation);
            state.retain_setup_worker(SetupWorkerEvidence::Known(known));
            let mut snapshot = workspace_snapshot(2, 1);
            set_label(&mut snapshot, target, "Changed");
            match corruption {
                0 => set_label(&mut snapshot, target, "Wrong"),
                1 => set_label(
                    &mut snapshot,
                    if target == Target::Project {
                        Target::Workspace
                    } else {
                        Target::Project
                    },
                    "Unrelated",
                ),
                2 => snapshot.terminal_contexts[1].tabs[0].label = "Unrelated Tab".into(),
                3 => snapshot.navigation.workspaces[0].terminal_context_id = context_id(99),
                _ => snapshot.navigation.standalone_terminal_context_id = context_id(99),
            };
            assert!(matches!(
                state.prepare_refresh(1, WorkbenchSnapshotOutcome::Available { snapshot }),
                PreparedRefresh::Divergent { .. }
            ));
            assert!(state.has_attachments());
            assert_eq!(state.cache().revision(), WorkbenchRevision::new(1));
        }
    }
}
#[test]
fn later_location_label_snapshot_uses_peer_label_and_real_membership_through_cleanup() {
    for target in [Target::Project, Target::Workspace] {
        let mut state = live(1, 2);
        let operation = operation(&mut state, target, Some("Client"), 1);
        let known = operation.known_commit(receipt(target, 2, false)).unwrap();
        state.begin_setup(operation);
        state.retain_setup_worker(SetupWorkerEvidence::Known(known));
        let old = state.attached_key().unwrap().clone();
        let mut snapshot = workspace_snapshot(4, 2);
        set_label(&mut snapshot, target, "Peer");
        snapshot.terminal_contexts[1].tabs.remove(0);
        let MembershipInstallDecision::Attach(next) = finish(&mut state, snapshot, 1) else {
            panic!("peer removal must install remaining member after cleanup");
        };
        assert_ne!(next.selected().key(), &old);
        assert_eq!(next.selected().key().tab_id(), &tab(11).tab_id);
        assert_eq!(state.cache().location(), &location());
        assert!(state.attached_key().is_none());
        assert!(state.invariant_holds());
    }
}
#[test]
fn noop_result_unavailable_keeps_known_authority_and_explicit_refresh_never_opens_a_new_write_step()
{
    for target in [Target::Project, Target::Workspace] {
        let mut state = workspace_state();
        let operation = operation(&mut state, target, None, 2);
        let known = operation.known_commit(receipt(target, 1, true)).unwrap();
        state.begin_setup(operation.clone());
        assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known.clone())));
        let prepared = state.prepare_refresh(
            2,
            WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        );
        state.apply_prepared_refresh(prepared, std::time::Instant::now());
        state.clear_settled_setup_status();
        assert_eq!(
            state.setup_status(),
            Some(NativeSetupStatus::LocationLabel {
                target,
                status: Status::CommittedNeedRefresh
            })
        );
        assert!(state.setup_draft().is_none());
        assert!(state.start_pending_refresh(3));
        state.reset_setup_recovery_deadlines(operation.deadlines);
        assert_eq!(
            finish(&mut state, workspace_snapshot(1, 0), 3),
            MembershipInstallDecision::Preserved
        );
        assert!(state.setup_draft().is_none());
        assert_eq!(
            state.setup_status(),
            Some(NativeSetupStatus::LocationLabel {
                target,
                status: Status::Unchanged
            })
        );
        assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
        assert!(state.invariant_holds());
    }
}
#[test]
fn settled_noop_and_new_queued_rename_retirement_evidence_are_consumed_independently() {
    let mut state = live(1, 1);
    let first = operation(&mut state, Target::Workspace, None, 1);
    let known = first
        .known_commit(receipt(Target::Workspace, 1, true))
        .unwrap();
    state.begin_setup(first);
    state.retain_setup_worker(SetupWorkerEvidence::Known(known.clone()));
    assert_eq!(
        finish(&mut state, workspace_snapshot(1, 1), 1),
        MembershipInstallDecision::Preserved
    );
    let next = operation(&mut state, Target::Project, Some("Queued"), 2);
    state.begin_setup(next.clone());
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
    assert!(state.setup_pending_or_draft());
    assert!(state.retain_setup_worker(SetupWorkerEvidence::NotApplied(next.clone())));
    assert!(state.retain_setup_worker(SetupWorkerEvidence::NotApplied(next)));
    assert_eq!(
        state.setup_status(),
        Some(NativeSetupStatus::LocationLabel {
            target: Target::Project,
            status: Status::NotApplied
        })
    );
    assert!(!state.setup_blocks_structure());
    assert!(state.invariant_holds());
}

fn targeted_source(standalone: bool, empty: bool) -> (NativeSessionState, Snapshot) {
    let mut snapshot = workspace_snapshot(1, usize::from(!empty));
    if empty {
        snapshot.terminal_contexts[0].tabs.clear();
    }
    let mut target = snapshot.navigation.projects[0].clone();
    target.project_id = WorkbenchProjectId::parse(&id(79)).unwrap();
    snapshot.navigation.projects.push(target);
    let source = if standalone {
        NativeContextLocation::Standalone(
            snapshot.navigation.standalone_terminal_context_id.clone(),
        )
    } else {
        location()
    };
    let cache = project_native_context_tabs(&snapshot, &source).unwrap();
    let state = if empty {
        NativeSessionState::bootstrap(cache, 1, geometry(80))
            .unwrap()
            .0
    } else {
        ready_with_cache(cache)
    };
    (state, snapshot)
}

fn targeted_draft(state: &mut NativeSessionState) -> crate::terminal_session::NativeSetupToken {
    let request = crate::terminal_session::ProjectRenameRequest::capture(
        state,
        &state.cache().projects()[1],
        Instant::now() + OPERATION_TOTAL,
    )
    .unwrap();
    state
        .open_setup_draft(NativeSetupIntent::RenameProject(Box::new(request)))
        .unwrap()
}

fn targeted_operation(state: &mut NativeSessionState, changed: bool) -> SetupOperation {
    let token = targeted_draft(state);
    assert_eq!(
        state.setup_draft().unwrap().project_label(),
        Some("Project")
    );
    assert_eq!(state.setup_draft().unwrap().text(), "Project");
    state.acknowledge_setup_draw(Some(token));
    if changed {
        state.edit_setup_draft(token, Edit::Clear);
        state.edit_setup_draft(token, Edit::Append("Target 雪"));
    }
    state
        .plan_setup(
            token,
            workbench_request_id(99),
            12,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap()
}

#[test]
fn targeted_project_rename_preserves_source_and_validates_receipts_and_exact_deltas() {
    for standalone in [false, true] {
        for empty in [false, true] {
            for changed in [false, true] {
                let (mut state, snapshot) = targeted_source(standalone, empty);
                let source = state.cache().location().clone();
                let desired = state.desired().cloned();
                let focus = state.focus_pane_ref();
                let operation = targeted_operation(&mut state, changed);
                let target = snapshot.navigation.projects[1].project_id.clone();
                assert!(
                    matches!(&operation.command.action, WorkbenchAction::ProjectRename { project_id, .. } if project_id == &target)
                );
                let revision = WorkbenchRevision::new(if changed { 2 } else { 1 });
                let receipt = WorkbenchSetupCommit::ProjectRenamed {
                    revision,
                    project_id: target.clone(),
                    unchanged: !changed,
                };
                for (project_id, bad_revision, unchanged) in [
                    (
                        snapshot.navigation.projects[0].project_id.clone(),
                        revision,
                        !changed,
                    ),
                    (target.clone(), revision, changed),
                    (target.clone(), WorkbenchRevision::new(3), !changed),
                ] {
                    assert!(
                        operation
                            .known_commit(WorkbenchSetupCommit::ProjectRenamed {
                                revision: bad_revision,
                                project_id,
                                unchanged,
                            })
                            .is_none()
                    );
                }
                let known = operation.known_commit(receipt).unwrap();
                state.begin_setup(operation);
                assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
                let mut committed = snapshot.clone();
                committed.revision = revision;
                if changed {
                    committed.navigation.projects[1].label = "Target 雪".into();
                }
                for corruption in 0..4 {
                    let mut wrong = committed.clone();
                    match corruption {
                        0 => wrong.navigation.projects[0].label = "Unrelated".into(),
                        1 => wrong.navigation.projects[1].label = "Forged".into(),
                        2 => wrong.navigation.workspaces[0].label = "Other".into(),
                        _ => wrong.terminal_contexts[usize::from(!standalone)]
                            .tabs
                            .push(tab(91)),
                    }
                    assert!(matches!(
                        state.prepare_refresh(
                            12,
                            WorkbenchSnapshotOutcome::Available { snapshot: wrong }
                        ),
                        PreparedRefresh::Divergent { .. }
                    ));
                }
                assert_eq!(
                    finish(&mut state, committed, 12),
                    MembershipInstallDecision::Preserved
                );
                assert_eq!(state.cache().location(), &source);
                assert_eq!(state.desired(), desired.as_ref());
                assert_eq!(state.focus_pane_ref(), focus);
                assert_eq!(
                    state.setup_status(),
                    Some(NativeSetupStatus::LocationLabel {
                        target: Target::Project,
                        status: if changed {
                            Status::Renamed
                        } else {
                            Status::Unchanged
                        },
                    })
                );
                assert!(!state.settle_stale_project_rename());
                assert!(!state.take_project_quick_terminal_entry());
                assert!(!state.setup_blocks_structure());
                assert!(state.invariant_holds());
            }
        }
    }
}

#[test]
fn targeted_project_rename_releases_stale_drafts_and_keeps_committed_recovery() {
    for removed in [false, true] {
        let (mut state, mut snapshot) = targeted_source(false, false);
        let token = targeted_draft(&mut state);
        if removed {
            snapshot.navigation.projects.pop();
        } else {
            snapshot.navigation.projects[1].label = "Replacement".into();
        }
        state.cache = project_native_context_tabs(&snapshot, &location()).unwrap();
        assert!(state.setup_draft().is_none());
        assert!(state.settle_stale_project_rename());
        assert!(!state.setup_blocks_structure());
        assert_eq!(
            state.setup_status(),
            Some(NativeSetupStatus::LocationLabel {
                target: Target::Project,
                status: Status::NotApplied
            })
        );
        state.cancel_setup_draft(token);
        assert!(
            state
                .open_setup_draft(NativeSetupIntent::RenameLocation(Target::Project))
                .is_some()
        );
    }
    let (mut state, mut snapshot) = targeted_source(true, false);
    let operation = targeted_operation(&mut state, true);
    let known = operation
        .known_commit(WorkbenchSetupCommit::ProjectRenamed {
            revision: WorkbenchRevision::new(2),
            project_id: snapshot.navigation.projects[1].project_id.clone(),
            unchanged: false,
        })
        .unwrap();
    state.begin_setup(operation.clone());
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
    let prepared = state.prepare_refresh(
        12,
        WorkbenchSnapshotOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    );
    state.apply_prepared_refresh(prepared, Instant::now());
    assert!(state.setup_recovery());
    assert!(!state.settle_stale_project_rename());
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.projects[1].label = "Target 雪".into();
    assert!(state.start_pending_refresh(13));
    state.reset_setup_recovery_deadlines(operation.deadlines);
    assert_eq!(
        finish(&mut state, snapshot, 13),
        MembershipInstallDecision::Preserved
    );
    assert!(!state.setup_blocks_structure());
    assert!(state.invariant_holds());
}

#[test]
fn targeted_project_rename_preserves_nonfirst_tab_panel_and_settles_changed_focus() {
    for stale in [false, true] {
        let (_, mut snapshot) = targeted_source(false, false);
        snapshot.terminal_contexts[0].tabs.clear();
        snapshot.terminal_contexts[1].tabs = vec![
            tab(50),
            split_panel_baseline().terminal_contexts[0].tabs[0].clone(),
        ];
        let mut state =
            ready_with_cache(project_native_context_tabs(&snapshot, &location()).unwrap());
        let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
        let key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
        let next = state.target_for_switch(1, key.clone(), deadlines).unwrap();
        assert!(state.begin_switch(next));
        install_switch_candidate(&mut state, &key);
        let first = state.desired().unwrap().clone();
        assert!(state.install_projection_t(&first));
        assert!(state.record_setup_configuration_t(&first, controlling(), Some(geometry(80))));
        assert!(state.stage_drawn_frame_t(&first, FrameIdentity::new(2)));
        assert!(state.mark_feedback_accepted_t(&first, FrameIdentity::new(2)));
        assert!(state.publish_ready_t(
            &first,
            FrameIdentity::new(2),
            controlling(),
            Some(geometry(80))
        ));
        let panel = state.tab(1).unwrap().leaf_panel_ids()[1].clone();
        let peer = PaneKey::new(first.anchor().clone(), panel);
        state.install_unattached_pane(&SelectedTab::new_panel(peer.clone(), 1, 1));
        let peer = state.pane_ref_for(&peer).unwrap();
        assert!(state.settle_attached_event_t(&peer).is_some());
        assert!(state.record_setup_configuration_t(&peer, controlling(), Some(geometry(80))));
        assert!(state.stage_drawn_frame_t(&peer, FrameIdentity::new(3)));
        assert!(state.mark_feedback_accepted_t(&peer, FrameIdentity::new(3)));
        assert!(state.move_panel_focus_local(peer.pane().clone()));
        assert_eq!(state.active().unwrap().index(), 1);
        assert_eq!(state.active().unwrap().panel_index(), 1);
        let operation = targeted_operation(&mut state, true);
        if stale {
            let before = state.cache().clone();
            state.desired = Some(first.clone());
            state.phase = SessionPhase::Ready(ReadyAttachment {
                selected: SelectedTab::new_panel(first.clone(), 1, 0),
                mode: controlling(),
            });
            assert_eq!(state.cache(), &before);
            assert!(state.setup_draft().is_none());
            assert!(state.settle_stale_project_rename());
            assert!(!state.setup_blocks_structure());
            assert!(state.project_rename_available());
        } else {
            let known = operation
                .known_commit(WorkbenchSetupCommit::ProjectRenamed {
                    revision: WorkbenchRevision::new(2),
                    project_id: snapshot.navigation.projects[1].project_id.clone(),
                    unchanged: false,
                })
                .unwrap();
            state.begin_setup(operation);
            assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
            snapshot.revision = WorkbenchRevision::new(2);
            snapshot.navigation.projects[1].label = "Target 雪".into();
            assert_eq!(
                finish(&mut state, snapshot, 12),
                MembershipInstallDecision::Preserved
            );
            assert_eq!(state.focus_pane_ref(), Some(peer.clone()));
            assert_eq!(state.desired(), Some(peer.pane()));
            assert!(state.pane_attached(&peer));
            assert_eq!(state.active().unwrap().index(), 1);
            assert_eq!(state.active().unwrap().panel_index(), 1);
        }
        assert!(state.invariant_holds());
    }
}
/// DF-003: a peer change while a rename draft blocked refresh leaves captured publication
/// demand that the setup flight adopts without reading a snapshot. A definite refusal
/// returns that demand unread instead of parking it, so the next fair drain installs the
/// peer revision; a stale refusal is itself demand and keeps the typed text for the owner.
#[test]
fn rename_refusal_returns_unread_publication_demand_and_stale_keeps_typed_text() {
    for target in [Target::Project, Target::Workspace] {
        for (stale, captured) in [(true, true), (true, false), (false, true)] {
            let mut state = live(1, 1);
            let operation = operation(&mut state, target, Some("Typed name"), 2);
            if captured {
                state.note_publication_hint();
            }
            state.begin_setup(operation.clone());
            assert!(state.settle_setup_refusal(&operation, stale));
            let prepared = PreparedRefresh::DefiniteFailure {
                generation: 2,
                baseline_revision: state.cache().revision(),
            };
            state.apply_prepared_refresh(prepared, Instant::now());
            assert_eq!(
                state.setup_status(),
                Some(NativeSetupStatus::LocationLabel {
                    target,
                    status: if stale {
                        Status::Stale
                    } else {
                        Status::NotApplied
                    }
                })
            );
            assert!(
                state.publication_debt_pending(),
                "refused demand is never parked"
            );
            assert!(state.invariant_holds());

            assert!(state.start_publication_drain(3));
            let mut peer = workspace_snapshot(2, 1);
            set_label(&mut peer, target, "Peer name");
            let prepared =
                state.prepare_refresh(3, WorkbenchSnapshotOutcome::Available { snapshot: peer });
            assert!(matches!(
                state.apply_prepared_refresh(prepared, Instant::now()),
                RefreshDecision::Continue
            ));
            assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
            assert!(!state.publication_debt_pending());
            assert!(state.invariant_holds());

            let token = state
                .open_setup_draft(NativeSetupIntent::RenameLocation(target))
                .unwrap();
            assert_eq!(
                state.setup_draft().unwrap().text(),
                if stale { "Typed name" } else { "Peer name" }
            );
            state.cancel_setup_draft(token);
            state
                .open_setup_draft(NativeSetupIntent::RenameLocation(target))
                .unwrap();
            assert_eq!(state.setup_draft().unwrap().text(), "Peer name");
        }
    }
}
