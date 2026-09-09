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
    WorkbenchProjectId, WorkbenchSnapshotUnavailableReason, WorkbenchWorkspaceId,
};
fn location() -> NativeContextLocation {
    NativeContextLocation::Workspace {
        project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&id(71)).unwrap(),
        terminal_context_id: context_id(72),
    }
}
fn live(revision: u64, count: usize) -> NativeSessionState {
    ready_with_cache(
        project_native_context_tabs(&workspace_snapshot(revision, count), &location()).unwrap(),
    )
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
    if let Some(label) = replacement {
        state.edit_setup_draft(token, Edit::Clear);
        state.edit_setup_draft(token, Edit::Append(label));
    }
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let request = WorkbenchRequestId::parse(&id(900 + u128::from(generation))).unwrap();
    assert!(
        state
            .plan_setup(token, request.clone(), generation, deadlines)
            .is_none()
    );
    state.acknowledge_setup_draw(Some(token));
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
            project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&id(71)).unwrap(),
            unchanged,
        },
    }
}
fn set_label(snapshot: &mut Snapshot, target: Target, label: &str) {
    match target {
        Target::Project => snapshot.navigation.projects[0].label = label.into(),
        Target::Workspace => snapshot.navigation.projects[0].workspaces[0].label = label.into(),
    }
}
#[test]
fn location_rename_prefills_canonical_text_and_cancel_does_not_change_target_or_terminal() {
    for target in [Target::Project, Target::Workspace] {
        let mut state = live(1, 1);
        let baseline = state.cache().clone();
        let desired = state.desired().cloned();
        let frame = state.accepted_frame.clone();
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
        state.edit_setup_draft(token, Edit::Append(&"雪".repeat(512)));
        assert_eq!(state.setup_draft().unwrap().token(), token);
        assert!(!state.setup_draft().unwrap().faulted());
        state.edit_setup_draft(token, Edit::Append("x"));
        assert!(state.setup_draft().unwrap().faulted());
        state.cancel_setup_draft(token);
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.desired(), desired.as_ref());
        assert_eq!(state.accepted_frame, frame);
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
            .open_setup_draft(NativeSetupIntent::CreateWorkspace)
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
                WorkbenchSetupCommit::ProjectRenamed { project_id, .. }
                | WorkbenchSetupCommit::WorkspaceRenamed { project_id, .. } => {
                    *project_id = WorkbenchProjectId::parse(&id(99)).unwrap()
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
    for target in [Target::Project, Target::Workspace] {
        for changed in [false, true] {
            let mut state = live(1, 1);
            let old = state.desired().cloned();
            let frame = state.accepted_frame.clone();
            let operation = operation(&mut state, target, changed.then_some("  renamed 雪  "), 1);
            let known = operation
                .known_commit(receipt(target, if changed { 2 } else { 1 }, !changed))
                .unwrap();
            state.begin_setup(operation);
            state.retain_setup_worker(SetupWorkerEvidence::Known(known));
            let mut snapshot = workspace_snapshot(if changed { 2 } else { 1 }, 1);
            if changed {
                set_label(&mut snapshot, target, "  renamed 雪  ");
            }
            assert_eq!(
                finish(&mut state, snapshot, 1),
                MembershipInstallDecision::Preserved
            );
            assert_eq!(state.desired(), old.as_ref());
            assert_eq!(state.attached_key(), old.as_ref());
            assert_eq!(state.accepted_frame, frame);
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
                3 => {
                    snapshot.navigation.projects[0].workspaces[0].terminal_context_id =
                        context_id(99)
                }
                _ => snapshot.navigation.standalone_terminal_context_id = context_id(99),
            };
            assert!(matches!(
                state.prepare_refresh(1, WorkbenchSnapshotOutcome::Available { snapshot }),
                PreparedRefresh::Divergent { .. }
            ));
            assert!(state.attached_key().is_some());
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
        state.apply_prepared_refresh(prepared);
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
