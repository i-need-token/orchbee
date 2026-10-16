//! Conditional deletion must prove every captured binding before any effect.
use super::*;
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
fn pending_precommit_create_with_unchanged_revision_blocks_both_idle_actions() {
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
        assert_eq!(
            owner.snapshot(),
            &before,
            "pending reservation has no revision yet"
        );
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
        assert_eq!(owner.snapshot(), &before);
        assert_eq!(owner.outstanding_close_credits().ok(), Some(0));
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
