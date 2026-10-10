use super::{authority, command, create, identity, project_id, submit};
use crate::{WorkbenchAuthority, WorkbenchGrant};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchDirectoryPath, WorkbenchProjectId, WorkbenchRequestId,
    WorkbenchTerminalGeometry, WorkbenchWorktreeAction as Action,
    WorkbenchWorktreeOutcome as Outcome, WorkbenchWorktreeRequest as Request, WorkerId,
    WorktreeAdmissionFailure, WorktreeIntent, WorktreeOperationFailure as Failure,
    WorktreeOperationPhase as Phase, WorktreeOperationStatus as Status, WorktreeReference,
    WorktreeSource,
};
use std::num::NonZeroU64;
fn setup() -> (WorkbenchAuthority, WorkbenchGrant, WorkbenchProjectId) {
    let (mut authority, navigation) = authority();
    let project = project_id(&submit(
        &mut authority,
        &navigation,
        command(10, 0, create("repo")),
    ));
    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone())
        .with_directory_association()
        .with_terminal_lifecycle();
    (authority, grant, project)
}
fn intent() -> WorktreeIntent {
    WorktreeIntent::Create {
        source: WorktreeSource::Directory {
            path: WorkbenchDirectoryPath::parse("C:/repo/subdir").unwrap(),
        },
        branch: None,
        base: None,
        path: None,
        label: None,
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        trust_repository: false,
    }
}
fn start(project: &WorkbenchProjectId, id: u128) -> Action {
    Action::Start {
        operation_id: WorkbenchRequestId::parse(&identity(id)).unwrap(),
        project_id: Some(project.clone()),
        intent: intent(),
    }
}
fn apply(
    authority: &mut WorkbenchAuthority,
    grant: Option<&WorkbenchGrant>,
    action: Action,
    available: bool,
) -> Outcome {
    let request = Request {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(10000)).unwrap(),
        site_id: authority.site_id().clone(),
        action,
    };
    authority
        .execute_worktree_request(
            grant,
            request,
            Some((WorkerId::parse(&identity(20)).unwrap(), NonZeroU64::MIN)),
            available,
        )
        .outcome
}
#[test]
fn read_is_pure_replay_is_retained_and_changed_intent_cannot_mutate() {
    let (mut authority, grant, project) = setup();
    let action = start(&project, 40);
    assert_eq!(
        apply(&mut authority, None, action.clone(), true),
        Outcome::Denied {}
    );
    assert!(authority.claim_worktree_operation().is_none());
    let accepted = apply(&mut authority, Some(&grant), action.clone(), true);
    let Action::Start { operation_id, .. } = &action else {
        unreachable!()
    };
    assert_eq!(
        apply(
            &mut authority,
            Some(&grant),
            Action::Read {
                operation_id: operation_id.clone()
            },
            false
        ),
        accepted
    );
    assert_eq!(
        apply(&mut authority, Some(&grant), action.clone(), false),
        accepted
    );
    let mut changed = action.clone();
    let Action::Start {
        intent: WorktreeIntent::Create { branch, .. },
        ..
    } = &mut changed
    else {
        unreachable!()
    };
    *branch = Some(WorktreeReference::parse("different").unwrap());
    assert_eq!(
        apply(&mut authority, Some(&grant), changed, true),
        Outcome::Conflict {}
    );
    assert_eq!(
        apply(&mut authority, Some(&grant), start(&project, 41), true),
        Outcome::Unavailable {
            reason: WorktreeAdmissionFailure::Busy
        }
    );
    let plan = authority.claim_worktree_operation().unwrap();
    assert!(plan.source.directory_id.is_none());
    assert_eq!(plan.source.path.as_str(), "C:/repo/subdir");
    assert!(authority.claim_worktree_operation().is_none());
    let failed = Status::Failed {
        reason: Failure::WorkerBusy {},
        progress: plan.progress.clone(),
        resumable: true,
    };
    assert!(authority.publish_worktree_operation(&plan, failed.clone()));
    assert!(!authority.publish_worktree_operation(&plan, failed));
    assert!(
        matches!(apply(&mut authority, Some(&grant), action, false), Outcome::Available { status, .. } if matches!(*status, Status::Failed { .. }))
    );
}
#[test]
fn resume_is_generation_fenced_and_old_owner_cannot_publish_into_new_attempt() {
    let (mut authority, grant, project) = setup();
    apply(&mut authority, Some(&grant), start(&project, 40), true);
    let old = authority.claim_worktree_operation().unwrap();
    assert!(authority.publish_worktree_operation(
        &old,
        Status::Failed {
            reason: Failure::WorkerBusy {},
            progress: old.progress.clone(),
            resumable: true
        }
    ));
    let resume = Action::Resume {
        operation_id: old.operation_id.clone(),
        expected_attempt: 1,
    };
    assert!(matches!(
        apply(&mut authority, Some(&grant), resume.clone(), true),
        Outcome::Available { attempt: 2, .. }
    ));
    assert_eq!(
        apply(&mut authority, Some(&grant), resume, true),
        Outcome::Conflict {}
    );
    let current = authority.claim_worktree_operation().unwrap();
    assert_eq!(current.attempt, 2);
    assert!(!authority.publish_worktree_operation(
        &old,
        Status::Failed {
            reason: Failure::WorkerBusy {},
            progress: old.progress.clone(),
            resumable: true
        }
    ));
    let mut forged = current.clone();
    forged.project_id = Some(WorkbenchProjectId::parse(&identity(88)).unwrap());
    assert!(!authority.publish_worktree_operation(
        &forged,
        Status::Pending {
            phase: Phase::Creating,
            progress: forged.progress.clone()
        }
    ));
    assert!(!authority.publish_worktree_operation(
        &current,
        Status::Pending {
            phase: Phase::Removing,
            progress: current.progress.clone()
        }
    ));
    assert!(authority.publish_worktree_operation(
        &current,
        Status::Pending {
            phase: Phase::Creating,
            progress: current.progress.clone()
        }
    ));
}
#[test]
fn capacity_never_expires_an_old_operation_and_unknown_failure_cannot_resume() {
    let (mut authority, grant, project) = setup();
    for id in 100..164 {
        assert!(matches!(
            apply(&mut authority, Some(&grant), start(&project, id), true),
            Outcome::Available { attempt: 1, .. }
        ));
        let plan = authority.claim_worktree_operation().unwrap();
        assert!(authority.publish_worktree_operation(
            &plan,
            Status::Failed {
                reason: Failure::PhysicalUnknown {},
                progress: plan.progress.clone(),
                resumable: false
            }
        ));
    }
    assert_eq!(
        apply(&mut authority, Some(&grant), start(&project, 164), true),
        Outcome::Unavailable {
            reason: WorktreeAdmissionFailure::Capacity
        }
    );
    assert!(matches!(
        apply(&mut authority, Some(&grant), start(&project, 100), false),
        Outcome::Available { attempt: 1, .. }
    ));
    let resume = Action::Resume {
        operation_id: WorkbenchRequestId::parse(&identity(100)).unwrap(),
        expected_attempt: 1,
    };
    assert!(
        matches!(apply(&mut authority, Some(&grant), resume, true), Outcome::Available { attempt: 1, status, .. } if matches!(*status, Status::Failed { resumable: false, .. }))
    );
    assert!(authority.claim_worktree_operation().is_none());
}
#[test]
fn wrong_site_denies_even_existing_receipts_and_missing_source_never_admits() {
    let (mut authority, grant, project) = setup();
    apply(&mut authority, Some(&grant), start(&project, 40), true);
    let wrong = Request {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(1000)).unwrap(),
        site_id: orchbee_workbench_protocol::SiteId::parse(&identity(200)).unwrap(),
        action: start(&project, 40),
    };
    assert_eq!(
        authority
            .execute_worktree_request(Some(&grant), wrong, None, false)
            .outcome,
        Outcome::Denied {}
    );
    let (mut authority, grant, _) = setup();
    let absent = WorkbenchProjectId::parse(&identity(400)).unwrap();
    assert_eq!(
        apply(&mut authority, Some(&grant), start(&absent, 40), true),
        Outcome::Unavailable {
            reason: WorktreeAdmissionFailure::SourceUnavailable
        }
    );
    assert!(authority.claim_worktree_operation().is_none());
}

#[test]
fn observed_source_subdirectory_keeps_checkout_membership_until_directory_changes() {
    use orchbee_workbench_protocol::{
        WorkbenchAction, WorkbenchCommandOutcome, WorkbenchLabel, WorktreeCheckoutFacts,
    };
    let (mut authority, grant, project) = setup();
    let created = submit(
        &mut authority,
        &grant,
        command(
            11,
            1,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project.clone()),
                label: WorkbenchLabel::parse("main").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = created.outcome else {
        panic!("workspace");
    };
    let worker = WorkerId::parse(&identity(20)).unwrap();
    let result = authority.accept_workspace_directory(
        Some(&grant),
        command(
            12,
            2,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                workspace_id: workspace_id.clone(),
                path: WorkbenchDirectoryPath::parse("C:/repo/subdir").unwrap(),
            },
        ),
        worker.clone(),
    );
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    let mut intent = intent();
    let WorktreeIntent::Create { source, .. } = &mut intent else {
        unreachable!()
    };
    *source = WorktreeSource::Workspace {
        workspace_id: workspace_id.clone(),
    };
    apply(
        &mut authority,
        Some(&grant),
        Action::Start {
            operation_id: WorkbenchRequestId::parse(&identity(40)).unwrap(),
            project_id: None,
            intent,
        },
        true,
    );
    let plan = authority.claim_worktree_operation().unwrap();
    let checkout = WorktreeCheckoutFacts {
        repository_root: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        common_directory: WorkbenchDirectoryPath::parse("C:/repo/.git").unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        branch: None,
        dirty: false,
    };
    assert!(authority.associate_worktree_source(&plan, checkout.clone()));
    assert_eq!(authority.worktree_checkout(&workspace_id), Some(checkout));
    assert!(authority.publish_worktree_operation(
        &plan,
        Status::Failed {
            reason: Failure::SourceChanged {},
            progress: plan.progress.clone(),
            resumable: false
        }
    ));
    let mut changed = authority.snapshot().clone();
    changed.navigation.directories[0].path = WorkbenchDirectoryPath::parse("C:/elsewhere").unwrap();
    authority.projection = crate::workbench_projection::WorkbenchProjectionStore::load(
        &orchbee_workbench_protocol::serialize_document(
            &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(changed),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(authority.worktree_checkout(&workspace_id).is_none());
    assert!(!authority.associate_worktree_source(
        &plan,
        WorktreeCheckoutFacts {
            repository_root: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
            common_directory: WorkbenchDirectoryPath::parse("C:/repo/.git").unwrap(),
            path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
            branch: None,
            dirty: false,
        }
    ));
}
