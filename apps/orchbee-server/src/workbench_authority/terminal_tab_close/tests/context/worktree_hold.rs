use super::*;
use orchbee_workbench_protocol::{
    WorkbenchWorktreeAction as Action, WorkbenchWorktreeOutcome as Outcome,
    WorkbenchWorktreeRequest, WorktreeIntent, WorktreeOperationFailure as Failure,
    WorktreeOperationPhase as Phase, WorktreeOperationStatus as Status,
    WorktreeWorkspaceDisposition,
};

fn apply(authority: &mut WorkbenchAuthority, action: Action) -> Outcome {
    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone())
        .with_terminal_lifecycle()
        .with_directory_association();
    authority
        .execute_worktree_request(
            Some(&grant),
            WorkbenchWorktreeRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id(50001),
                site_id: authority.site_id().clone(),
                action,
            },
            Some((binding(700).worker_id, NonZeroU64::MIN)),
            true,
        )
        .outcome
}

#[test]
fn held_publication_preserves_zero_and_multi_terminal_layout_and_fences_stale_attempts() {
    for count in [0, 2] {
        for explicit_close in [false, true] {
            let (mut authority, grant) = workspace_fixture(count);
            let before = authority.snapshot().clone();
            let project = &before.navigation.projects[0];
            let workspace = project.workspaces[0].workspace_id.clone();
            assert!(matches!(
                apply(
                    &mut authority,
                    Action::Start {
                        operation_id: request_id(50000),
                        project_id: project.project_id.clone(),
                        intent: WorktreeIntent::Remove {
                            workspace_id: workspace.clone(),
                            force: true,
                            trust_repository: false
                        }
                    }
                ),
                Outcome::Available { .. }
            ));
            let mut plan = authority.claim_worktree_operation().unwrap();
            let close = workspace_close_command(&authority, 10000, 0, 0);
            let now = Instant::now();
            accept_workspace(&mut authority, &grant, close.clone(), now);
            assert!(authority.hold_worktree_close(&plan, &close.request_id));
            assert!(!authority.hold_worktree_close(&plan, &request_id(99999)));
            let mut stale = plan.clone();
            stale.attempt += 1;
            assert!(!authority.hold_worktree_close(&stale, &close.request_id));
            plan.progress.workspace = WorktreeWorkspaceDisposition::Closing {
                workspace_id: workspace,
                close_request_id: close.request_id.clone(),
            };
            plan.phase = Phase::Closing;
            assert!(authority.publish_worktree_operation(
                &plan,
                Status::Pending {
                    phase: plan.phase,
                    progress: plan.progress.clone()
                }
            ));
            if count != 0 {
                assert!(!authority.worktree_close_quiesced(&plan, &close.request_id));
            }
            let competing = workspace_close_command(&authority, 10001, 0, 0);
            let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
                authority.begin_workspace_close(Some(&grant), competing)
            else {
                panic!("admission");
            };
            assert!(matches!(
                admission.resolve_after_composition(),
                WorkbenchTabCloseResolution::Ready(WorkbenchCommandResult {
                    outcome: WorkbenchCommandOutcome::Unavailable {
                        reason: WorkbenchCommandUnavailableReason::TerminalBusy
                    },
                    ..
                })
            ));
            for step_index in 0..20 {
                let step = authority
                    .next_terminal_tab_close_step_target(&close.request_id, now)
                    .unwrap();
                match step.next() {
                    CloseContinuationNext::Lifecycle { .. } => {
                        settle_disposed(&mut authority, &close.request_id, step, now)
                    }
                    CloseContinuationNext::TargetCleanup => authority
                        .complete_terminal_tab_close_target_cleanup(
                            &close.request_id,
                            step.target().unwrap().runtime().unwrap(),
                        )
                        .unwrap(),
                    CloseContinuationNext::Parked => break,
                    other => panic!("unexpected {other:?}"),
                }
                assert!(step_index < 19, "bounded terminal cleanup");
            }
            assert!(authority.worktree_close_quiesced(&plan, &close.request_id));
            assert!(!authority.worktree_close_quiesced(&stale, &close.request_id));
            assert!(matches!(
                authority.prepare_terminal_tab_close_publication(&close.request_id, now),
                Err(WorkbenchTabClosePublicationError::NotReady)
            ));
            assert_eq!(authority.snapshot(), &before);
            plan.phase = Phase::Removing;
            assert!(authority.publish_worktree_operation(
                &plan,
                Status::Pending {
                    phase: plan.phase,
                    progress: plan.progress.clone()
                }
            ));
            assert!(!authority.complete_worktree_removal(&mut stale));
            if !explicit_close {
                assert!(authority.complete_worktree_removal(&mut plan));
            }
            assert!(authority.publish_worktree_operation(
                &plan,
                Status::Failed {
                    reason: Failure::CloseIncomplete {},
                    progress: plan.progress.clone(),
                    resumable: true
                }
            ));
            if explicit_close {
                assert!(authority.worktree_holds_close(&close.request_id));
                let mut rejected = workspace_close_command(&authority, 10002, 0, 0);
                rejected.expected_revision = WorkbenchRevision::new(999);
                assert!(matches!(
                    authority.begin_workspace_close(Some(&grant), rejected),
                    WorkbenchWorkspaceCloseBegin::Ready(_)
                ));
                assert!(authority.worktree_holds_close(&close.request_id));
                let explicit = workspace_close_command(&authority, 10003, 0, 0);
                let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
                    authority.begin_workspace_close(Some(&grant), explicit)
                else {
                    panic!("manual admission");
                };
                assert!(matches!(
                    admission.resolve_after_composition(),
                    WorkbenchTabCloseResolution::Ready(WorkbenchCommandResult {
                        outcome: WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. },
                        ..
                    })
                ));
                assert!(!authority.worktree_holds_close(&close.request_id));
            } else {
                assert!(matches!(
                    apply(
                        &mut authority,
                        Action::Resume {
                            operation_id: plan.operation_id.clone(),
                            expected_attempt: 1
                        }
                    ),
                    Outcome::Available { attempt: 2, .. }
                ));
                let resumed = authority.claim_worktree_operation().unwrap();
                assert!(resumed.removal_complete);
                assert_eq!(resumed.phase, Phase::Removing);
                assert!(!authority.hold_worktree_close(&resumed, &close.request_id));
                assert!(!authority.complete_worktree_removal(&mut plan));
            }
            let status = authority
                .prepare_terminal_tab_close_publication(&close.request_id, now)
                .unwrap()
                .commit();
            assert!(matches!(status, WorkbenchTabCloseStatus::Closed { .. }));
            assert!(
                authority.snapshot().navigation.projects[0]
                    .workspaces
                    .is_empty()
            );
        }
    }
}
