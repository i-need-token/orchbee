use super::*;
use orchbee_workbench_protocol::{
    WorkbenchLabel, WorkbenchWorkspaceId, WorktreeGitError, WorktreeGitStage,
    WorktreeOperationFailure, WorktreeWorkspaceAssociation,
};

pub(crate) fn fixture() -> (Launcher, Origin, addressed::SidebarDiscovery) {
    let mut snapshot = addressed::tests::fixture();
    snapshot.navigation.directories[0].path = path("C:/trees/猫");
    snapshot.navigation.workspaces[1].label = "Task 猫".into();
    let state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
    let origin = state.worktree_origin().unwrap();
    let workspace = &snapshot.navigation.workspaces[1];
    let addressed = addressed::AddressedWorktreeSource::capture(
        &snapshot,
        &addressed::SidebarWorktreeTarget::Workspace(workspace.workspace_id.clone()),
    )
    .unwrap();
    let mut discovery = discovery();
    discovery.worker = addressed.primary.as_ref().unwrap().worker_id().clone();
    discovery.source.workspace_id = Some(workspace.workspace_id.clone());
    discovery.entries[1]
        .workspaces
        .push(WorktreeWorkspaceAssociation {
            workspace_id: workspace.workspace_id.clone(),
            project_id: workspace.project_id.clone(),
            label: WorkbenchLabel::parse(&workspace.label).unwrap(),
        });
    let sidebar = addressed::SidebarDiscovery {
        addressed,
        observed: Some(path("C:/repo")),
        source: WorktreeSource::WorkspaceDirectory {
            workspace_id: workspace.workspace_id.clone(),
            path: path("C:/repo"),
        },
        discovery,
    };
    assert!(sidebar.valid(&snapshot));
    let mut launcher = Launcher::default();
    assert!(launcher.open_sidebar(origin.clone(), sidebar.clone(), WorktreeMode::Remove, None));
    (launcher, origin, sidebar)
}

pub(crate) fn failure(attempt: u32, resumable: bool) -> WorkbenchWorktreeOutcome {
    WorkbenchWorktreeOutcome::Available {
        project_id: None,
        attempt,
        status: Box::new(WorktreeOperationStatus::Failed {
            reason: WorktreeOperationFailure::Git {
                stage: WorktreeGitStage::DirtyCheck,
                error: WorktreeGitError::Dirty,
                status: None,
                diagnostics: Vec::new(),
            },
            progress: WorktreeProgress {
                path: Some(path("C:/trees/猫")),
                checkout: None,
                workspace: WorktreeWorkspaceDisposition::Open {
                    workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 80)).unwrap(),
                },
                git_effect: WorktreeGitEffect::None,
                restoration: None,
            },
            resumable,
        }),
    }
}

fn removed(attempt: u32, target: &str) -> WorkbenchWorktreeOutcome {
    WorkbenchWorktreeOutcome::Available {
        project_id: None,
        attempt,
        status: Box::new(WorktreeOperationStatus::Removed {
            path: path(target),
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 80)).unwrap(),
        }),
    }
}

#[test]
fn removal_requires_the_addressed_workspace_exact_linked_directory_and_association() {
    for fault in 0..6 {
        let (_, _, mut sidebar) = fixture();
        match fault {
            0 => sidebar.discovery.entries[1].linked = false,
            1 => sidebar.discovery.entries[1].checkout.bare = true,
            2 => {
                sidebar.discovery.entries[1].checkout.path =
                    WorktreeEntryText::Utf8("C:/other".into())
            }
            3 => sidebar.discovery.entries[1].workspaces.clear(),
            4 => {
                sidebar.addressed.target = addressed::SidebarWorktreeTarget::Project(
                    sidebar.addressed.cache.project_id().unwrap().clone(),
                )
            }
            5 => sidebar.discovery.worker = WorkerId::parse(&format!("{:032x}", 999)).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            Removal::capture(&sidebar).is_none(),
            "unsafe target {fault}"
        );
    }
}

#[test]
fn removal_matches_windows_checkout_spelling_but_never_another_directory() {
    let checkout = |sidebar: &mut addressed::SidebarDiscovery, value: &str| {
        sidebar.discovery.entries[1].checkout.path = WorktreeEntryText::Utf8(value.into());
    };
    for (value, admitted) in [
        (r"C:\trees\猫", true),
        (r"c:\TREES\猫\", true),
        ("C:/trees/猫/sub", false),
        ("C:/trees/猫x", false),
        ("C:/trees", false),
        (r"\\?\C:\trees\猫", false),
    ] {
        let (_, _, mut sidebar) = fixture();
        checkout(&mut sidebar, value);
        assert_eq!(Removal::capture(&sidebar).is_some(), admitted, "{value}");
    }
}

/// Git would refuse only after the Workspace closed, so the refusal must come before any plan.
#[test]
fn a_client_standing_in_the_checkout_is_refused_before_submission() {
    for (client, refused) in [(r"c:\TREES\猫\src", true), ("C:/trees/猫x", false)] {
        let (mut launcher, origin, _) = fixture();
        launcher.set_client_directory(Some(client.into()));
        assert_eq!(
            confirm(&mut launcher, &origin).is_none(),
            refused,
            "{client}"
        );
        if refused {
            assert_eq!(launcher.notice(), WorktreeNotice::ClientInCheckout);
            assert!(launcher.operation().is_none());
        }
    }
}

#[test]
fn removal_waits_for_exact_confirmation_draw_and_never_uses_create_or_open_auto_submit() {
    let (mut launcher, origin, _) = fixture();
    let token = launcher.token().unwrap();
    assert_eq!(launcher.view().unwrap().stage(), WorktreeStage::Remove);
    assert_eq!(
        launcher.view().unwrap().removal_target(),
        Some(("Task 猫", "C:/trees/猫", Some("refs/heads/same"), 2))
    );
    assert!(
        launcher
            .confirm(token, request(90), &origin, Locale::EnUs)
            .is_none()
    );
    assert!(!launcher.sidebar_open_ready());
    launcher.draw(Some(token));
    assert!(launcher.edit(token, WorktreeEdit::Paste("--force")));
    assert_eq!(launcher.token(), Some(token));
    let plan = launcher
        .confirm(token, request(90), &origin, Locale::EnUs)
        .unwrap();
    assert!(
        matches!(&plan.intent, WorktreeIntent::Remove { workspace_id, force: false, trust_repository: false }
        if workspace_id.as_str() == format!("{:032x}", 80))
    );
    assert!(plan.project.is_none());
    assert_start_roundtrip(&plan);
    assert!(
        launcher
            .confirm(token, request(91), &origin, Locale::EnUs)
            .is_none()
    );

    let (mut cancelled, origin, _) = fixture();
    let token = cancelled.token().unwrap();
    cancelled.draw(Some(token));
    cancelled.dismiss();
    assert!(cancelled.operation().is_none());
    assert!(!cancelled.reopen());
    assert!(
        cancelled
            .confirm(token, request(92), &origin, Locale::EnUs)
            .is_none()
    );
}

#[test]
fn changed_addressed_directory_and_first_blocked_geometry_cannot_confirm_removal() {
    let (mut launcher, mut origin, _) = fixture();
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    let mut snapshot = origin.cache.canonical_snapshot().clone();
    snapshot.navigation.directories[0].path = path("C:/other");
    origin.cache = project_preferred_workspace(&snapshot).unwrap();
    assert!(
        launcher
            .confirm(token, request(90), &origin, Locale::EnUs)
            .is_none()
    );
    assert_eq!(launcher.notice(), WorktreeNotice::SourceChanged);
    assert!(launcher.operation().is_none());

    let (mut launcher, origin, _) = fixture();
    let token = launcher.token().unwrap();
    assert_eq!(
        launcher.geometry_blocked(),
        Some(GeometrySettlement::Refused)
    );
    assert!(
        launcher
            .confirm(token, request(90), &origin, Locale::EnUs)
            .is_none()
    );
    assert!(launcher.operation().is_none());
}

#[test]
fn dirty_removal_retains_readable_failure_and_resume_requires_a_new_cancelable_confirmation() {
    let (mut launcher, origin, _) = fixture();
    let plan = confirm(&mut launcher, &origin).unwrap();
    assert!(launcher.start_settled(&plan.operation_id, Ok(failure(1, true))));
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    assert!(launcher.view().unwrap().can_resume_remove());
    assert!(launcher.prepare_remove_resume(token));
    let resume_token = launcher.token().unwrap();
    assert_ne!(resume_token, token);
    assert!(launcher.confirm_remove_resume(token).is_none());
    assert!(launcher.confirm_remove_resume(resume_token).is_none());
    launcher.draw(Some(resume_token));
    assert!(launcher.cancel_remove_recovery());
    assert_eq!(
        launcher.operation().unwrap().outcome,
        Some(failure(1, true))
    );
    assert_eq!(launcher.view().unwrap().stage(), WorktreeStage::Result);
    // The restored result needs a fresh token so input capture restarts after Esc.
    let restored = launcher.token().unwrap();
    assert_ne!(restored, resume_token);
    assert!(!launcher.prepare_remove_resume(restored));
    launcher.draw(Some(resume_token));
    assert!(!launcher.prepare_remove_resume(resume_token));
    launcher.draw(Some(restored));
    assert!(launcher.prepare_remove_resume(restored));
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    let (resumed, expected_attempt) = launcher.confirm_remove_resume(token).unwrap();
    assert_eq!(resumed.operation_id, plan.operation_id);
    assert_eq!(expected_attempt, 1);
    assert!(launcher.confirm_remove_resume(token).is_none());
    assert!(launcher.resume_settled(&plan.operation_id, Err(false)));
    assert_eq!(
        launcher.operation().unwrap().outcome,
        Some(failure(1, true))
    );
    assert!(launcher.view().unwrap().can_resume_remove());
}

#[test]
fn uncertain_resume_keeps_original_identity_and_fences_old_attempts_until_a_correlated_read() {
    let (mut launcher, origin, _) = fixture();
    let plan = confirm(&mut launcher, &origin).unwrap();
    assert!(launcher.start_settled(&plan.operation_id, Ok(failure(1, true))));
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    assert!(launcher.prepare_remove_resume(token));
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    launcher.confirm_remove_resume(token).unwrap();
    launcher.dismiss();
    assert!(launcher.resume_settled(&plan.operation_id, Err(true)));
    assert!(!launcher.visible());
    assert_eq!(launcher.notice(), WorktreeNotice::Unknown);
    assert!(launcher.observed(&plan.operation_id, Ok(failure(1, true))));
    assert_eq!(launcher.notice(), WorktreeNotice::Unknown);
    assert_eq!(
        launcher.operation().unwrap().outcome,
        Some(failure(1, true))
    );
    assert_eq!(
        launcher.begin_read().unwrap().operation_id,
        plan.operation_id
    );
    assert!(!launcher.observed(&request(999), Ok(removed(2, "C:/trees/猫"))));
    assert!(launcher.observed(&plan.operation_id, Ok(removed(2, "C:/trees/猫"))));
    assert_eq!(launcher.notice(), WorktreeNotice::Removed);
    assert!(!launcher.visible());
    assert!(!launcher.operation().unwrap().focus_allowed);
    assert!(launcher.reopen());
    assert!(!launcher.view().unwrap().can_resume_remove());
}

#[test]
fn removed_receipt_must_match_confirmed_path_and_workspace_and_attempt_limit_is_not_resumable() {
    for (receipt, workspace, notice) in [
        ("C:/trees/猫", 80, WorktreeNotice::Removed),
        // Git's spelling of a Workspace started from a Windows launch directory.
        (r"c:\TREES\猫\", 80, WorktreeNotice::Removed),
        ("C:/other", 80, WorktreeNotice::SourceChanged),
        ("C:/trees/猫", 81, WorktreeNotice::SourceChanged),
    ] {
        let (mut launcher, origin, _) = fixture();
        let plan = confirm(&mut launcher, &origin).unwrap();
        let mut outcome = removed(1, receipt);
        if let WorkbenchWorktreeOutcome::Available { status, .. } = &mut outcome
            && let WorktreeOperationStatus::Removed { workspace_id, .. } = status.as_mut()
        {
            *workspace_id = WorkbenchWorkspaceId::parse(&format!("{workspace:032x}")).unwrap();
        }
        assert!(launcher.start_settled(&plan.operation_id, Ok(outcome)));
        assert_eq!(launcher.notice(), notice, "{receipt} {workspace}");
    }
    let (mut launcher, origin, _) = fixture();
    let plan = confirm(&mut launcher, &origin).unwrap();
    assert!(launcher.start_settled(&plan.operation_id, Ok(failure(1, false))));
    assert!(!launcher.view().unwrap().can_resume_remove());
    if let Phase::Result(op) = &mut launcher.phase {
        op.attempt = orchbee_workbench_protocol::MAXIMUM_WORKTREE_OPERATION_ATTEMPTS;
        op.outcome = Some(failure(op.attempt, true));
    }
    assert!(!launcher.view().unwrap().can_resume_remove());
}

#[test]
fn unknown_physical_or_transport_failure_never_offers_resume() {
    for reason in [
        WorktreeOperationFailure::PhysicalUnknown {},
        WorktreeOperationFailure::TransportUnknown {},
    ] {
        let (mut launcher, origin, _) = fixture();
        let plan = confirm(&mut launcher, &origin).unwrap();
        let mut outcome = failure(1, true);
        let WorkbenchWorktreeOutcome::Available { status, .. } = &mut outcome else {
            unreachable!()
        };
        let WorktreeOperationStatus::Failed { reason: actual, .. } = status.as_mut() else {
            unreachable!()
        };
        *actual = reason;
        assert!(launcher.start_settled(&plan.operation_id, Ok(outcome)));
        assert!(!launcher.view().unwrap().can_resume_remove());
    }
}

#[test]
fn abandon_and_stale_discard_release_only_settled_retained_receipts() {
    let (mut launcher, origin, _) = fixture();
    let plan = confirm(&mut launcher, &origin).unwrap();
    let target = plan.removal.as_ref().unwrap().workspace_id.clone();
    let present = origin.cache.canonical_snapshot().clone();
    let mut absent = present.clone();
    absent
        .navigation
        .workspaces
        .retain(|workspace| workspace.workspace_id != target);
    // An accepted removal that has not settled is never discarded locally.
    assert!(!launcher.forget_stale_removal(&absent));
    assert!(launcher.start_settled(&plan.operation_id, Ok(failure(1, true))));
    assert!(launcher.retains_removal());
    assert!(!launcher.forget_stale_removal(&present));

    // Abandon needs its own drawn confirmation, and cancelling restores the receipt.
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    assert!(launcher.view().unwrap().can_abandon_remove());
    assert!(launcher.prepare_remove_abandon(token));
    assert_eq!(
        launcher.view().unwrap().stage(),
        WorktreeStage::AbandonRemove
    );
    let abandon = launcher.token().unwrap();
    assert!(!launcher.confirm_remove_abandon(token));
    assert!(!launcher.confirm_remove_abandon(abandon));
    assert!(launcher.confirm_remove_resume(abandon).is_none());
    assert!(launcher.cancel_remove_recovery());
    assert_eq!(launcher.view().unwrap().stage(), WorktreeStage::Result);
    let restored = launcher.token().unwrap();
    assert!(!launcher.prepare_remove_abandon(restored));
    launcher.draw(Some(restored));
    assert!(launcher.prepare_remove_abandon(restored));
    let abandon = launcher.token().unwrap();
    launcher.draw(Some(abandon));
    assert!(launcher.confirm_remove_abandon(abandon));
    assert!(launcher.operation().is_none());
    assert!(!launcher.visible());
    assert!(!launcher.reopen());

    // Nothing to abandon when Close already releases the receipt.
    let (mut launcher, origin, _) = fixture();
    let plan = confirm(&mut launcher, &origin).unwrap();
    assert!(launcher.start_settled(&plan.operation_id, Ok(failure(1, false))));
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    assert!(!launcher.view().unwrap().can_abandon_remove());
    assert!(!launcher.prepare_remove_abandon(token));

    // A settled receipt whose Workspace left the snapshot has no local recovery left.
    let (mut launcher, origin, _) = fixture();
    let plan = confirm(&mut launcher, &origin).unwrap();
    assert!(launcher.start_settled(&plan.operation_id, Ok(failure(1, true))));
    launcher.dismiss();
    assert!(launcher.forget_stale_removal(&absent));
    assert!(launcher.operation().is_none());
    assert!(!launcher.reopen());
}
