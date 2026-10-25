//! Conditional deletion must prove every captured binding before any effect.
use super::*;
use crate::test_support::WorkbenchFixture;
use crate::workbench_authority::WorkbenchProjectDissolveBegin;
use orchbee_workbench_protocol::{PanelNode, Presentation, TerminalPresentationId};

fn conditional(owner: &WorkbenchAuthority, id: u128) -> WorkbenchCommand {
    let mut close = workspace_close_command(owner, id, 1, 0);
    let WorkbenchAction::WorkspaceClose { workspace_id } = close.action else {
        unreachable!()
    };
    close.action = WorkbenchAction::WorkspaceCloseIfIdle { workspace_id };
    close
}

fn dissolve(owner: &WorkbenchAuthority, id: u128) -> WorkbenchCommand {
    command(
        owner,
        id,
        WorkbenchAction::ProjectDissolveIfIdle {
            project_id: owner.snapshot().navigation.projects[1].project_id.clone(),
        },
    )
}

fn never_created(
    members: &[crate::workbench_authority::WorkbenchTerminalTab],
) -> Vec<WorkbenchTabCloseMemberRuntime> {
    members
        .iter()
        .map(|member| WorkbenchTabCloseMemberRuntime::Uncreated {
            binding: member.binding().clone(),
        })
        .collect()
}

fn finish(
    primary: WorkbenchTabClosePrimaryAdmission<'_>,
    runtimes: Vec<WorkbenchTabCloseMemberRuntime>,
) -> WorkbenchCommandResult {
    match primary.finish_resolved_members(runtimes, Instant::now()) {
        WorkbenchTabClosePreparation::Ready(result) => result,
        WorkbenchTabClosePreparation::Prepared(prepared) => prepared.commit(),
    }
}

#[test]
fn only_complete_idle_runtime_proofs_admit_unconfirmed_close_for_every_member() {
    for unknown in [None, Some(0), Some(1)] {
        let (mut owner, grant) = workspace_fixture(2);
        let close = conditional(&owner, 11800);
        let before = owner.snapshot().clone();
        let primary = primary_workspace(&mut owner, &grant, close);
        let proofs = primary
            .members()
            .iter()
            .enumerate()
            .map(|(index, member)| {
                let target = runtime(member);
                if unknown == Some(index) {
                    WorkbenchTabCloseMemberRuntime::Runtime {
                        target,
                        observed_phase: None,
                    }
                } else {
                    WorkbenchTabCloseMemberRuntime::IdleRuntime {
                        target,
                        observed_phase: LifecycleTerminalPhase::Running {
                            terminate_requested: false,
                        },
                    }
                }
            })
            .collect();
        let result = finish(primary, proofs);
        assert_eq!(owner.snapshot(), &before);
        if unknown.is_none() {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
            ));
            assert_eq!(owner.outstanding_close_credits().ok(), Some(1));
        } else {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::ConfirmationRequired,
                    ..
                }
            ));
            assert_eq!(owner.outstanding_close_credits().ok(), Some(0));
        }
    }
}

#[test]
fn confirmed_close_upgrades_a_late_idle_refusal_but_never_an_unknown_signal() {
    for unknown in [false, true] {
        let (mut owner, grant) = workspace_fixture(1);
        let close = conditional(&owner, 11820);
        let primary = primary_workspace(&mut owner, &grant, close.clone());
        let proofs = primary
            .members()
            .iter()
            .map(|member| WorkbenchTabCloseMemberRuntime::IdleRuntime {
                target: runtime(member),
                observed_phase: LifecycleTerminalPhase::Running {
                    terminate_requested: false,
                },
            })
            .collect();
        finish(primary, proofs);
        let now = Instant::now();
        let step = owner
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap();
        let CloseContinuationNext::Lifecycle {
            action,
            step_ordinal,
        } = step.next()
        else {
            panic!("guarded step");
        };
        assert_eq!(action, TerminalLifecycleAction::TerminateIfIdle);
        let request = step_request(
            &close.request_id,
            step.target().unwrap(),
            action,
            step_ordinal,
            WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
        );
        let token = owner
            .reserve_terminal_tab_close_step(&close.request_id, now, request)
            .unwrap();
        owner
            .confirm_terminal_tab_close_step_admitted(&close.request_id, &token, now)
            .unwrap();
        let outcome = if unknown {
            orchbee_capability_node_protocol::TerminalLifecycleOutcome::OutcomeUnknown(
                orchbee_capability_node_protocol::LifecycleUnknownReason::TransportLost,
            )
        } else {
            orchbee_capability_node_protocol::TerminalLifecycleOutcome::HostFailure(
                orchbee_capability_node_protocol::LifecycleHostFailure::ConfirmationRequired,
            )
        };
        let result =
            orchbee_capability_node_protocol::TerminalLifecycleResult::new(request, outcome)
                .unwrap();
        owner
            .settle_terminal_tab_close_step(
                &close.request_id,
                token,
                now,
                CloseStepObservation::Result(&result),
            )
            .unwrap();
        let confirmed = workspace_close_command(&owner, 11821, 1, 0);
        let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
            owner.begin_workspace_close(Some(&grant), confirmed.clone())
        else {
            panic!("confirmed admission");
        };
        let WorkbenchTabCloseResolution::Ready(result) = admission.resolve_after_confirmation(now)
        else {
            panic!("retained operation");
        };
        if unknown {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::TerminalBusy
                }
            ));
            assert_eq!(
                owner
                    .terminal_tab_close_generation(&close.request_id)
                    .unwrap()
                    .value(),
                1
            );
        } else {
            assert!(
                matches!(result.outcome, WorkbenchCommandOutcome::WorkspaceCloseAccepted { ref close_request_id, .. } if close_request_id == &close.request_id)
            );
            assert_eq!(
                owner
                    .terminal_tab_close_generation(&close.request_id)
                    .unwrap()
                    .value(),
                2
            );
            assert!(matches!(
                owner
                    .next_terminal_tab_close_step_target(&close.request_id, now)
                    .unwrap()
                    .next(),
                CloseContinuationNext::Lifecycle {
                    action: TerminalLifecycleAction::Terminate,
                    ..
                }
            ));
            assert!(
                matches!(owner.begin_workspace_close(Some(&grant), confirmed), WorkbenchWorkspaceCloseBegin::Ready(replay) if replay == result)
            );
        }
        assert_eq!(owner.outstanding_close_credits().ok(), Some(1));
    }
}

fn tab_create(owner: &WorkbenchAuthority, id: u128) -> WorkbenchCommand {
    command(
        owner,
        id,
        WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: owner.snapshot().navigation.workspaces[1]
                .terminal_context_id
                .clone(),
            label: WorkbenchLabel::parse("peer create").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    )
}

#[test]
fn idle_close_accepts_only_complete_never_created_scope_and_installs_existing_fence() {
    for count in [0, 2] {
        let (mut owner, grant) = workspace_fixture(count);
        let close = conditional(&owner, 11000);
        let before = owner.snapshot().clone();
        let primary = primary_workspace(&mut owner, &grant, close.clone());
        assert_eq!(primary.members().len(), usize::try_from(count).unwrap());
        let proofs = never_created(primary.members());
        let accepted = finish(primary, proofs);
        assert!(
            matches!(accepted.outcome, WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            ref close_request_id, accepted_revision, ..
        } if close_request_id == &close.request_id && accepted_revision == before.revision)
        );
        assert_eq!(owner.snapshot(), &before);
        assert_eq!(owner.outstanding_close_credits().ok(), Some(1));
        let create = tab_create(&owner, 11001);
        assert!(matches!(
            owner.begin_terminal_tab_create(Some(&grant), create),
            WorkbenchTabCreateBegin::Ready(WorkbenchCommandResult {
                outcome: WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::TerminalBusy
                },
                ..
            })
        ));
        assert!(
            matches!(owner.begin_workspace_close(Some(&grant), close.clone()),
            WorkbenchWorkspaceCloseBegin::Ready(result) if result == accepted)
        );
        let mut switched = close;
        let WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } = switched.action else {
            unreachable!()
        };
        switched.action = WorkbenchAction::WorkspaceClose { workspace_id };
        assert!(matches!(
            owner.begin_workspace_close(Some(&grant), switched),
            WorkbenchWorkspaceCloseBegin::Ready(WorkbenchCommandResult {
                outcome: WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::RequestIdentityConflict,
                    ..
                },
                ..
            })
        ));
    }
}

#[test]
fn runtime_close_refusal_is_retained_without_effect_and_never_becomes_confirmation() {
    let (mut owner, grant) = workspace_fixture(2);
    let close = conditional(&owner, 11100);
    let before = owner.snapshot().clone();
    let primary = primary_workspace(&mut owner, &grant, close.clone());
    let mut proofs = never_created(primary.members());
    proofs[1] = WorkbenchTabCloseMemberRuntime::Runtime {
        target: runtime(&primary.members()[1]),
        // Even an Exited observation is not the never-created admission proof.
        observed_phase: Some(LifecycleTerminalPhase::Exited {
            terminate_requested: false,
        }),
    };
    let refused = finish(primary, proofs);
    assert!(matches!(
        refused.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::ConfirmationRequired,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.outstanding_close_credits().ok(), Some(0));
    assert!(
        matches!(owner.begin_workspace_close(Some(&grant), close.clone()),
        WorkbenchWorkspaceCloseBegin::Ready(result) if result == refused)
    );
    let mut confirmed = close;
    let WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } = confirmed.action else {
        unreachable!()
    };
    confirmed.action = WorkbenchAction::WorkspaceClose { workspace_id };
    assert!(matches!(
        owner.begin_workspace_close(Some(&grant), confirmed.clone()),
        WorkbenchWorkspaceCloseBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            },
            ..
        })
    ));
    confirmed.request_id = request_id(11101);
    // A genuinely new, explicitly confirmed command keeps the old lifecycle.
    assert!(matches!(
        accept_workspace(&mut owner, &grant, confirmed, Instant::now()).outcome,
        WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
    ));
}

#[test]
fn committed_but_unsettled_create_blocks_both_idle_actions_at_the_current_revision() {
    for project in [false, true] {
        let (mut owner, grant) = workspace_fixture(0);
        let before = owner.snapshot().clone();
        let create = tab_create(&owner, 11200);
        let WorkbenchTabCreateBegin::Admitted(admission) =
            owner.begin_terminal_tab_create(Some(&grant), create)
        else {
            panic!("pending Create admission")
        };
        let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(3200))
        else {
            panic!("pending Create preparation")
        };
        let _pending = prepared.commit().into_pending();
        let committed = owner.snapshot().clone();
        assert_eq!(
            committed.revision.value(),
            before.revision.value() + 1,
            "Create commits canonical membership before installing its unsettled entry"
        );
        let context = &committed.navigation.workspaces[1]
            .terminal_context_id
            .clone();
        assert_eq!(
            committed
                .terminal_contexts
                .iter()
                .find(|entry| &entry.terminal_context_id == context)
                .unwrap()
                .tabs
                .len(),
            1
        );
        assert!(owner.pending_terminal_covers_context(context));
        let result = if project {
            let request = dissolve(&owner, 11201);
            let WorkbenchProjectDissolveBegin::Ready(result) =
                owner.begin_project_dissolve_if_idle(Some(&grant), request)
            else {
                panic!("pending scope must not reach runtime proof")
            };
            result
        } else {
            let request = conditional(&owner, 11201);
            let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
                owner.begin_workspace_close(Some(&grant), request)
            else {
                panic!("conditional admission")
            };
            let WorkbenchTabCloseResolution::Ready(result) = admission.resolve_after_composition()
            else {
                panic!("pending scope must not reach runtime proof")
            };
            result
        };
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            }
        ));
        assert_eq!(owner.snapshot(), &committed);
        assert_eq!(owner.outstanding_close_credits().ok(), Some(0));
    }
}

#[test]
fn dropping_uncommitted_create_preserves_empty_scope_without_a_pending_reservation() {
    for prepared in [false, true] {
        let (mut owner, grant) = workspace_fixture(0);
        let before = owner.snapshot().clone();
        let request = tab_create(&owner, 11210);
        let WorkbenchTabCreateBegin::Admitted(admission) =
            owner.begin_terminal_tab_create(Some(&grant), request)
        else {
            panic!("Create admission")
        };
        // Admission and preparation retain an exclusive Authority borrow. There is no
        // production seam for another command until this value commits or is dropped.
        if prepared {
            let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(3200))
            else {
                panic!("Create preparation")
            };
            drop(prepared);
        } else {
            drop(admission);
        }
        assert_eq!(owner.snapshot(), &before);
        assert!(!owner.pending_terminal_covers_context(
            &before.navigation.workspaces[1].terminal_context_id.clone()
        ));
        let request = conditional(&owner, 11211);
        let primary = primary_workspace(&mut owner, &grant, request);
        assert!(primary.members().is_empty());
        assert!(matches!(
            finish(primary, Vec::new()).outcome,
            WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
        ));
    }
}

#[test]
fn idle_dissolve_preserves_all_resources_and_checks_complete_project_not_first_workspace() {
    for live_last in [false, true] {
        let (mut owner, grant) = workspace_fixture(1);
        let mut snapshot = owner.snapshot().clone();
        let project = snapshot.navigation.projects[1].project_id.clone();
        snapshot.navigation.workspaces[0].project_id = Some(project.clone());
        owner.projection = WorkbenchProjectionStore::load(
            &orchbee_workbench_protocol::serialize_document(
                &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot),
            )
            .unwrap(),
        )
        .unwrap();
        let before = owner.snapshot().clone();
        let request = dissolve(&owner, 11300);
        let WorkbenchProjectDissolveBegin::Admitted(admission) =
            owner.begin_project_dissolve_if_idle(Some(&grant), request.clone())
        else {
            panic!("Project conditional admission")
        };
        assert_eq!(admission.members().len(), 2, "entire Project scope");
        let mut proofs = never_created(admission.members());
        if live_last {
            proofs[1] = WorkbenchTabCloseMemberRuntime::Runtime {
                target: runtime(&admission.members()[1]),
                observed_phase: None,
            };
        }
        let result = admission.finish_resolved_members(proofs);
        if live_last {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::ConfirmationRequired,
                    ..
                }
            ));
            assert_eq!(owner.snapshot(), &before);
        } else {
            let mut expected = before.clone();
            expected.revision = WorkbenchRevision::new(before.revision.value() + 1);
            expected
                .navigation
                .projects
                .retain(|entry| entry.project_id != project);
            expected
                .navigation
                .project_order
                .retain(|entry| entry != &project);
            for workspace in &mut expected.navigation.workspaces {
                workspace.project_id = None;
            }
            assert!(
                matches!(result.outcome, WorkbenchCommandOutcome::ProjectDissolved { ref project_id, .. } if project_id == &project)
            );
            assert_eq!(owner.snapshot(), &expected, "only organization changes");
        }
        assert_eq!(
            owner.outstanding_close_credits().ok(),
            Some(0),
            "dissolve never composes closes"
        );
        assert!(
            matches!(owner.begin_project_dissolve_if_idle(Some(&grant), request.clone()),
            WorkbenchProjectDissolveBegin::Ready(replay) if replay == result)
        );
        let mut switched = request;
        switched.action = WorkbenchAction::ProjectDissolve {
            project_id: project,
        };
        assert!(matches!(
            submit(&mut owner, &grant, switched).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
    }
}

#[test]
fn idle_actions_deny_grants_before_runtime_proof_or_ledger_reservation() {
    let (mut owner, _) = workspace_fixture(1);
    let read_only = WorkbenchGrant::read_only(owner.site_id().clone());
    let before = owner.snapshot().clone();
    for grant in [None, Some(&read_only)] {
        let close = conditional(&owner, 11400);
        assert!(matches!(
            owner.begin_workspace_close(grant, close),
            WorkbenchWorkspaceCloseBegin::Ready(WorkbenchCommandResult {
                outcome: WorkbenchCommandOutcome::Denied {},
                ..
            })
        ));
        let project = dissolve(&owner, 11401);
        assert!(matches!(
            owner.begin_project_dissolve_if_idle(grant, project),
            WorkbenchProjectDissolveBegin::Ready(WorkbenchCommandResult {
                outcome: WorkbenchCommandOutcome::Denied {},
                ..
            })
        ));
    }
    assert_eq!(owner.snapshot(), &before);
    assert!(owner.ledger.is_empty());
}

#[test]
fn child_only_bindings_cannot_be_omitted_from_idle_proof() {
    for project in [false, true] {
        let (mut owner, grant) = workspace_fixture(1);
        let mut snapshot = owner.snapshot().clone();
        let PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } = &mut snapshot.terminal_contexts[2].tabs[0].root
        else {
            unreachable!()
        };
        child_presentations.push(Presentation {
            custom_label: None,
            presentation_id: TerminalPresentationId::parse(&identity(8800)).unwrap(),
            binding: primary_presentation.binding.take(),
            label: "child only".into(),
            launch_failure: None,
        });
        owner.projection = WorkbenchProjectionStore::load(
            &orchbee_workbench_protocol::serialize_document(
                &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot),
            )
            .unwrap(),
        )
        .unwrap();
        let before = owner.snapshot().clone();
        let result = if project {
            let request = dissolve(&owner, 11500);
            let WorkbenchProjectDissolveBegin::Admitted(admission) =
                owner.begin_project_dissolve_if_idle(Some(&grant), request)
            else {
                panic!("Project conditional admission")
            };
            assert_eq!(admission.members().len(), 1);
            assert_eq!(admission.members()[0].binding(), &binding(3100));
            admission.finish_resolved_members(Vec::new())
        } else {
            let request = conditional(&owner, 11500);
            let primary = primary_workspace(&mut owner, &grant, request);
            assert_eq!(primary.members().len(), 1);
            assert_eq!(primary.members()[0].binding(), &binding(3100));
            finish(primary, Vec::new())
        };
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
            }
        ));
        assert_eq!(owner.snapshot(), &before);
        assert_eq!(owner.outstanding_close_credits().ok(), Some(0));
    }
}

#[test]
fn a_confirmed_fingerprint_cannot_be_reused_as_a_conditional_action() {
    for project in [false, true] {
        let (mut owner, grant) = workspace_fixture(1);
        let conditional = if project {
            dissolve(&owner, 11600)
        } else {
            conditional(&owner, 11600)
        };
        let mut confirmed = conditional.clone();
        confirmed.action = match &conditional.action {
            WorkbenchAction::ProjectDissolveIfIdle { project_id } => {
                WorkbenchAction::ProjectDissolve {
                    project_id: project_id.clone(),
                }
            }
            WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } => {
                WorkbenchAction::WorkspaceClose {
                    workspace_id: workspace_id.clone(),
                }
            }
            _ => unreachable!(),
        };
        let refusal = if project {
            assert!(matches!(
                submit(&mut owner, &grant, confirmed).outcome,
                WorkbenchCommandOutcome::ProjectDissolved { .. }
            ));
            let WorkbenchProjectDissolveBegin::Ready(result) =
                owner.begin_project_dissolve_if_idle(Some(&grant), conditional)
            else {
                panic!("fingerprint conflict precedes source resolution")
            };
            result
        } else {
            assert!(matches!(
                accept_workspace(&mut owner, &grant, confirmed, Instant::now()).outcome,
                WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
            ));
            let WorkbenchWorkspaceCloseBegin::Ready(result) =
                owner.begin_workspace_close(Some(&grant), conditional)
            else {
                panic!("fingerprint conflict precedes scope join")
            };
            result
        };
        assert!(matches!(
            refusal.outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
    }
}

#[test]
fn a_busy_project_plan_is_retained_before_any_idle_runtime_proof_is_requested() {
    let (mut owner, grant) = workspace_fixture(1);
    let close = workspace_close_command(&owner, 11700, 1, 0);
    accept_workspace(&mut owner, &grant, close, Instant::now());
    let before = owner.snapshot().clone();
    let request = dissolve(&owner, 11701);
    let WorkbenchProjectDissolveBegin::Ready(result) =
        owner.begin_project_dissolve_if_idle(Some(&grant), request.clone())
    else {
        panic!("settled busy plan must never yield runtime-proof admission")
    };
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    assert!(
        matches!(owner.begin_project_dissolve_if_idle(Some(&grant), request),
        WorkbenchProjectDissolveBegin::Ready(replay) if replay == result)
    );
    assert!(!owner.retired);
}
