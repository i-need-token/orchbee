use super::*;
use orchbee_workbench_protocol::{
    WorkbenchWorktreeAction as Action, WorkbenchWorktreeOutcome as Outcome,
    WorkbenchWorktreeRequest, WorktreeIntent, WorktreeOperationFailure as Failure,
    WorktreeOperationPhase as Phase, WorktreeOperationStatus as Status,
    WorktreeWorkspaceDisposition,
};

fn apply(authority: &mut WorkbenchAuthority, action: Action) -> Outcome {
    let grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone())
            .with_terminal_lifecycle()
            .with_directory_association();
    authority
        .execute_worktree_request(
            Some(&grant),
            WorkbenchWorktreeRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id(50001),
                session_id: authority.session_id().clone(),
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
            let workspace = before.navigation.workspaces[0].workspace_id.clone();
            assert!(matches!(
                apply(
                    &mut authority,
                    Action::Start {
                        operation_id: request_id(50000),
                        project_id: None,
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
                authority
                    .snapshot()
                    .navigation
                    .workspace(&before.navigation.workspaces[0].workspace_id)
                    .is_none()
            );
        }
    }
}

#[test]
fn recovery_rebind_preserves_slots_and_cancels_only_the_owned_quiescent_close() {
    for count in [0, 2] {
        let (mut authority, grant) = workspace_fixture(count);
        let before = authority.snapshot().clone();
        let workspace = &before.navigation.workspaces[0];
        let context = workspace.terminal_context_id.clone();
        let members = authority
            .terminal_tree_membership(&context)
            .unwrap()
            .members()
            .to_vec();
        assert!(matches!(
            apply(
                &mut authority,
                Action::Start {
                    operation_id: request_id(60000),
                    project_id: None,
                    intent: WorktreeIntent::Remove {
                        workspace_id: workspace.workspace_id.clone(),
                        force: true,
                        trust_repository: false
                    }
                }
            ),
            Outcome::Available { .. }
        ));
        let mut plan = authority.claim_worktree_operation().unwrap();
        let close = workspace_close_command(&authority, 11000, 0, 0);
        let now = Instant::now();
        accept_workspace(&mut authority, &grant, close.clone(), now);
        assert!(authority.hold_worktree_close(&plan, &close.request_id));
        plan.progress.workspace = WorktreeWorkspaceDisposition::Closing {
            workspace_id: workspace.workspace_id.clone(),
            close_request_id: close.request_id.clone(),
        };
        for phase in [Phase::Closing, Phase::Removing] {
            plan.phase = phase;
            assert!(authority.publish_worktree_operation(
                &plan,
                Status::Pending {
                    phase,
                    progress: plan.progress.clone()
                }
            ));
        }
        if count > 0 {
            assert!(!authority.retain_worktree_workspace(&plan, &close.request_id));
            assert!(
                authority
                    .prepare_worktree_rebind(&plan, &close.request_id, &members[0], binding(900))
                    .is_err()
            );
        }
        for _ in 0..20 {
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
        }
        let mut stale = plan.clone();
        stale.attempt += 1;
        assert!(!authority.retain_worktree_workspace(&stale, &close.request_id));
        assert!(!authority.retain_worktree_workspace(&plan, &request_id(123456)));
        for (index, member) in members.iter().enumerate() {
            let fresh = binding(900 + index as u128);
            assert!(
                authority
                    .prepare_worktree_rebind(&stale, &close.request_id, member, fresh.clone())
                    .is_err()
            );
            assert!(
                authority
                    .prepare_worktree_rebind(&plan, &request_id(123456), member, fresh.clone())
                    .is_err()
            );
            assert!(
                authority
                    .prepare_worktree_rebind(
                        &plan,
                        &close.request_id,
                        member,
                        member.binding().clone()
                    )
                    .is_err()
            );
            let mut foreign = fresh.clone();
            foreign.worker_id =
                orchbee_workbench_protocol::WorkerId::parse(&format!("{:032x}", 321)).unwrap();
            assert!(
                authority
                    .prepare_worktree_rebind(&plan, &close.request_id, member, foreign)
                    .is_err()
            );
            if index > 0 {
                assert!(
                    authority
                        .prepare_worktree_rebind(&plan, &close.request_id, member, binding(900))
                        .is_err()
                );
            }
            // Dropping a fully prepared projection has no effect.
            drop(
                authority
                    .prepare_worktree_rebind(&plan, &close.request_id, member, fresh.clone())
                    .unwrap(),
            );
            assert_eq!(
                authority
                    .terminal_tree_membership(&context)
                    .unwrap()
                    .members()[index],
                *member
            );
            authority
                .prepare_worktree_rebind(&plan, &close.request_id, member, fresh.clone())
                .unwrap()
                .commit();
            assert!(
                authority
                    .prepare_worktree_rebind(
                        &plan,
                        &close.request_id,
                        member,
                        binding(950 + index as u128)
                    )
                    .is_err()
            );
            let current = authority.terminal_tree_membership(&context).unwrap();
            let replaced = &current.members()[index];
            assert_eq!(replaced.tab_id(), member.tab_id());
            assert_eq!(replaced.panel_id(), member.panel_id());
            assert_ne!(replaced.presentation_id(), member.presentation_id());
            assert_eq!(replaced.binding(), &fresh);
            assert!(authority.has_terminal_tab_close(member.binding()));
            assert!(!authority.has_terminal_tab_close(&fresh));
            assert!(authority.open_close_fences_context(&context));
        }
        assert_eq!(authority.snapshot().navigation, before.navigation);
        assert!(authority.retain_worktree_workspace(&plan, &close.request_id));
        assert!(!authority.retain_worktree_workspace(&plan, &close.request_id));
        assert!(!authority.open_close_fences_context(&context));
        assert!(
            !authority
                .terminal_tab_close_operations()
                .any(|id| id == &close.request_id)
        );
        assert!(matches!(
            authority.terminal_tab_close_status(&close.request_id),
            Some(WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason: WorkbenchTabClosePublicationReason::WorkspaceRetained,
                resumable: false
            })
        ));
        assert!(matches!(
            authority.prepare_terminal_tab_close_publication(&close.request_id, now),
            Err(WorkbenchTabClosePublicationError::NotReady)
        ));
        let resume = orchbee_workbench_protocol::WorkbenchWorkspaceCloseResume {
            protocol_version: ProtocolVersion,
            request_id: request_id(11002),
            session_id: authority.session_id().clone(),
            workspace_id: workspace.workspace_id.clone(),
            close_request_id: close.request_id.clone(),
            expected_attempt_generation: authority
                .terminal_tab_close_generation(&close.request_id)
                .unwrap(),
        };
        assert!(
            matches!(authority.begin_workspace_close_resume(Some(&grant), resume), WorkbenchWorkspaceCloseResumeBegin::Ready(result) if matches!(result.outcome, WorkbenchWorkspaceCloseResumeOutcome::Rejected { .. }))
        );
        // A fresh ordinary close must not join the cancelled request, even for empty scopes.
        let next = workspace_close_command(&authority, 11001, 0, 0);
        let accepted = accept_workspace(&mut authority, &grant, next.clone(), now);
        assert!(
            matches!(accepted.outcome, WorkbenchCommandOutcome::WorkspaceCloseAccepted { ref close_request_id, .. } if close_request_id == &next.request_id)
        );
    }
}
