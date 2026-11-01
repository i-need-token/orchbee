use super::*;
mod sidebar;
use crate::local_client::workbench::WorkbenchWorkspaceCloseAccepted;
use crate::localization::Locale;
use crate::tui_presentation::GeometrySettlement;
use crate::workbench_projection::{ClientContextLocation, project_native_context_tabs};
use crate::workbench_state::workspace_close::{
    WorkspaceCloseEvidence, WorkspaceCloseOperation, WorkspaceCloseResult, WorkspaceCloseTarget,
    WorkspaceCloseToken, retains_close_result,
};
use orchbee_workbench_protocol::{
    Project, WorkbenchAction, WorkbenchProjectId, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTabClosePhase, WorkbenchTabCloseStatus, WorkbenchWorkspaceCloseCounts,
    WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceGroupCloseResult, WorkbenchWorkspaceId,
    Workspace, WorkspaceGroupCloseAction, WorkspaceGroupCloseChild, WorkspaceGroupCloseMember,
    WorkspaceGroupCloseObservation, WorkspaceGroupCloseOutcome, WorkspaceGroupCloseStatus,
};

fn close_snapshot() -> Snapshot {
    let mut source = snapshot(1, 1, None);
    source.navigation.projects.push(Project {
        project_id: project_id(),
        label: "项目 Project".into(),
    });
    for index in 0..3 {
        source.navigation.workspaces.push(Workspace {
            automatic_label: false,
            workspace_id: workspace_id(index),
            project_id: (index != 2).then(project_id),
            terminal_context_id: context_id(372 + index),
            primary_directory_id: None,
            label: format!("Workspace {index}"),
        });
        source.terminal_contexts.push(TerminalContext {
            terminal_context_id: context_id(372 + index),
            tabs: Vec::new(),
            groups: Vec::new(),
        });
    }
    source.navigation.workspace_order = source
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    source
}
fn project_id() -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&id(370)).unwrap()
}
fn workspace_id(index: u128) -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&id(380 + index)).unwrap()
}
fn close_state(index: u128) -> WorkbenchClientState {
    let source = close_snapshot();
    let workspace = source.navigation.workspace(&workspace_id(index)).unwrap();
    let location = ClientContextLocation::Workspace {
        workspace_id: workspace.workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    };
    WorkbenchClientState::bootstrap(
        project_native_context_tabs(&source, &location).unwrap(),
        1,
        geometry(80),
    )
    .unwrap()
    .0
}
fn background_state(initial_workspace: bool) -> WorkbenchClientState {
    let mut source = close_snapshot();
    let location = if initial_workspace {
        {
            let fixture_context_id = source.navigation.workspaces[0].terminal_context_id.clone();
            ClientContextLocation::Workspace {
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                    fixture_context_id.as_str(),
                )
                .unwrap(),
                terminal_context_id: fixture_context_id,
            }
        }
    } else {
        source.terminal_contexts[1].tabs = std::mem::take(&mut source.terminal_contexts[0].tabs);
        let workspace = &source.navigation.workspaces[1];
        ClientContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        }
    };
    ready_with_cache(project_native_context_tabs(&source, &location).unwrap())
}
fn plan(
    state: &WorkbenchClientState,
    token: WorkspaceCloseToken,
) -> Option<WorkspaceCloseOperation> {
    state.plan_workspace_close(
        token,
        WorkbenchRequestId::parse(&id(390)).unwrap(),
        2,
        OperationDeadlines::from_sample(Instant::now()).unwrap(),
    )
}
fn start(state: &mut WorkbenchClientState, project: bool) -> WorkspaceCloseOperation {
    let token = if project {
        state.open_project_close_all(&project_id())
    } else {
        state.open_workspace_close()
    }
    .unwrap();
    state.acknowledge_workspace_close_draw(Some(token));
    let operation = plan(state, token).unwrap();
    assert!(state.begin_workspace_close(operation.clone()));
    operation
}
fn accepted(operation: &WorkspaceCloseOperation) -> WorkbenchWorkspaceCloseAccepted {
    let WorkspaceCloseTarget::Workspace {
        workspace_id,
        terminal_context_id,
    } = &operation.target
    else {
        panic!("Workspace");
    };
    WorkbenchWorkspaceCloseAccepted {
        close_request_id: operation.request_id.clone(),
        workspace_id: workspace_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        accepted_revision: operation.baseline.revision(),
    }
}

#[test]
fn workspace_close_refuses_a_blocked_first_draw_but_suspends_an_admitted_prompt() {
    let mut refused = close_state(0);
    refused.open_workspace_close().unwrap();
    assert_eq!(
        refused.settle_workspace_close_not_presented(),
        Some(GeometrySettlement::Refused)
    );
    assert!(refused.workspace_close_confirmation().is_none());
    assert_eq!(refused.notice, Some(ClientNotice::DialogRefused));

    let mut suspended = close_state(0);
    let token = suspended.open_workspace_close().unwrap();
    suspended.acknowledge_workspace_close_draw(Some(token));
    assert!(
        suspended
            .workspace_close_confirmation()
            .unwrap()
            .interactive()
    );
    assert_eq!(
        suspended.settle_workspace_close_not_presented(),
        Some(GeometrySettlement::Suspended)
    );
    let prompt = suspended.workspace_close_confirmation().unwrap();
    assert_eq!(prompt.token(), token);
    assert!(!prompt.interactive());
    assert!(plan(&suspended, token).is_none());
    suspended.acknowledge_workspace_close_draw(Some(token));
    assert!(
        suspended
            .workspace_close_confirmation()
            .unwrap()
            .interactive()
    );
    assert!(plan(&suspended, token).is_some());
}
fn observed(operation: &WorkspaceCloseOperation, closed: bool) -> WorkspaceCloseResult {
    let accepted = accepted(operation);
    WorkspaceCloseResult::WorkspaceObserved {
        outcome: Box::new(WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: accepted.close_request_id.clone(),
            workspace_id: accepted.workspace_id.clone(),
            terminal_context_id: accepted.terminal_context_id.clone(),
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
            status: if closed {
                WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(2),
                }
            } else {
                WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                }
            },
            counts: WorkbenchWorkspaceCloseCounts::default(),
            members: Vec::new(),
        }),
        accepted,
    }
}
fn group(operation: &WorkspaceCloseOperation, closed: bool) -> WorkspaceCloseResult {
    WorkspaceCloseResult::ProjectObserved(Box::new(WorkbenchWorkspaceGroupCloseResult {
        protocol_version: ProtocolVersion,
        request_id: operation.request_id.clone(),
        operation_id: operation.request_id.clone(),
        outcome: WorkspaceGroupCloseOutcome::Available {
            project_id: project_id(),
            attempt: 1,
            status: if closed {
                WorkspaceGroupCloseStatus::Closed {}
            } else {
                WorkspaceGroupCloseStatus::Pending {}
            },
            members: (0..2)
                .map(|index| WorkspaceGroupCloseMember {
                    workspace_id: workspace_id(index),
                    label: WorkbenchLabel::parse(&format!("Workspace {index}")).unwrap(),
                    close: (closed || index == 0).then(|| WorkspaceGroupCloseChild {
                        request_id: WorkbenchRequestId::parse(&id(400 + index)).unwrap(),
                        observation: closed.then(|| WorkspaceGroupCloseObservation {
                            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                            status: WorkbenchTabCloseStatus::Closed {
                                revision: WorkbenchRevision::new(2 + index as u64),
                            },
                        }),
                    }),
                })
                .collect(),
        },
    }))
}

#[test]
fn workspace_close_requires_exact_draw_and_only_targets_current_workspace_including_ungrouped() {
    for index in 0..3 {
        let mut state = close_state(index);
        let old = state.cache().clone();
        let token = state.open_workspace_close().unwrap();
        assert!(plan(&state, token).is_none());
        assert_eq!(
            state
                .workspace_close_confirmation()
                .unwrap()
                .project_member_count(),
            None
        );
        state.acknowledge_workspace_close_draw(Some(token));
        let operation = plan(&state, token).unwrap();
        assert_eq!(
            operation.workspace_command().unwrap().action,
            WorkbenchAction::WorkspaceClose {
                workspace_id: workspace_id(index),
            }
        );
        assert!(operation.project_request().is_none());
        state.cancel_workspace_close();
        assert!(plan(&state, token).is_none());
        assert_eq!(state.cache(), &old);
        assert!(state.workspace_close.pending.is_none());
        assert!(state.invariant_holds());
    }
}
#[test]
fn background_workspace_close_preserves_origin_attachment_on_cancel_and_publication() {
    for initial_workspace in [false, true] {
        for index in [1, 2] {
            let mut state = background_state(initial_workspace);
            let baseline = state.cache().clone();
            let desired = state.desired().unwrap().clone();
            let pane = state.pane_ref_for(&desired).unwrap();
            let binding = state.pane_record(&desired).unwrap().binding().clone();
            let selection = state.active().unwrap().clone();
            let token = state.open_workspace_close_at(&workspace_id(index)).unwrap();
            assert_eq!(state.cache(), &baseline);
            assert_eq!(
                state.workspace_close_confirmation().unwrap().label(),
                format!("Workspace {index}")
            );
            assert!(plan(&state, token).is_none());
            state.acknowledge_workspace_close_draw(Some(token));
            state.cancel_workspace_close();
            assert!(plan(&state, token).is_none());
            assert_eq!(state.desired(), Some(&desired));
            assert!(state.pane_attached(&pane));
            assert_eq!(state.cache(), &baseline);

            let next = state.open_workspace_close_at(&workspace_id(index)).unwrap();
            state.acknowledge_workspace_close_draw(Some(next));
            let operation = plan(&state, next).unwrap();
            assert_eq!(operation.baseline, baseline);
            assert_eq!(
                operation.workspace_command().unwrap().action,
                WorkbenchAction::WorkspaceClose {
                    workspace_id: workspace_id(index),
                }
            );
            assert!(state.begin_workspace_close(operation.clone()));
            assert!(state.workspace_close_blocks_structure());
            assert!(state.finish_workspace_close(&WorkspaceCloseEvidence {
                operation: operation.clone(),
                result: observed(&operation, true),
            }));

            let mut published = baseline.canonical_snapshot().clone();
            published.revision = WorkbenchRevision::new(2);
            published
                .navigation
                .workspaces
                .retain(|workspace| workspace.workspace_id != workspace_id(index));
            published
                .navigation
                .workspace_order
                .retain(|id| id != &workspace_id(index));
            published
                .terminal_contexts
                .retain(|context| context.terminal_context_id != context_id(372 + index));
            assert_eq!(
                finish_refresh(
                    &mut state,
                    operation.generation,
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: published.clone()
                    },
                    None,
                ),
                RefreshDecision::Continue,
                "background removal must not request origin cleanup or attachment"
            );
            assert_eq!(state.cache().canonical_snapshot(), &published);
            assert_eq!(state.cache().location(), baseline.location());
            assert_eq!(state.active(), Some(&selection));
            assert_eq!(state.desired(), Some(&desired));
            assert_eq!(state.pane_ref_for(&desired), Some(pane.clone()));
            assert_eq!(state.pane_record(&desired).unwrap().binding(), &binding);
            assert!(state.pane_attached(&pane));
            assert!(state.pane_projected(&pane));
            assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
            assert!(state.invariant_holds());
        }
    }
}

#[test]
fn addressed_workspace_close_rejects_missing_or_remapped_target_before_begin() {
    for removed in [false, true] {
        let mut state = background_state(false);
        let origin = state.cache().location().clone();
        let desired = state.desired().unwrap().clone();
        let pane = state.pane_ref_for(&desired).unwrap();
        assert!(state.open_workspace_close_at(&workspace_id(99)).is_none());
        assert!(state.workspace_close_confirmation().is_none());
        let token = state.open_workspace_close_at(&workspace_id(1)).unwrap();
        state.acknowledge_workspace_close_draw(Some(token));
        let operation = plan(&state, token).unwrap();
        let mut changed = state.cache().canonical_snapshot().clone();
        if removed {
            changed
                .navigation
                .workspaces
                .retain(|workspace| workspace.workspace_id != workspace_id(1));
            changed
                .navigation
                .workspace_order
                .retain(|id| id != &workspace_id(1));
            changed
                .terminal_contexts
                .retain(|context| context.terminal_context_id != context_id(373));
        } else {
            changed.navigation.workspaces[2].terminal_context_id = context_id(999);
            changed.terminal_contexts[2].terminal_context_id = context_id(999);
            changed
                .terminal_contexts
                .sort_by(|a, b| a.terminal_context_id.cmp(&b.terminal_context_id));
        }
        // Keep the revision unchanged to witness exact snapshot identity, not just revision.
        state.cache = state.cache.project_snapshot(&changed).unwrap();
        assert!(!state.begin_workspace_close(operation));
        assert!(state.settle_stale_workspace_close());
        assert!(state.workspace_close_confirmation().is_none());
        assert!(state.workspace_close.pending.is_none());
        assert!(state.pending_refresh_generation().is_none());
        assert_eq!(state.cache().location(), &origin);
        assert_eq!(state.desired(), Some(&desired));
        assert!(state.pane_attached(&pane));
        assert!(state.invariant_holds());
    }
}

#[test]
fn background_workspace_close_rejects_origin_or_stale_target_receipts_without_losing_evidence() {
    let mut state = background_state(false);
    let desired = state.desired().unwrap().clone();
    let pane = state.pane_ref_for(&desired).unwrap();
    let token = state.open_workspace_close_at(&workspace_id(1)).unwrap();
    state.acknowledge_workspace_close_draw(Some(token));
    let operation = plan(&state, token).unwrap();
    let mut wrong_scope = operation.clone();
    wrong_scope.target = WorkspaceCloseTarget::Workspace {
        workspace_id: workspace_id(1),
        terminal_context_id: operation.baseline.terminal_context_id().unwrap().clone(),
    };
    assert!(!wrong_scope.admits(&observed(&wrong_scope, true)));
    assert!(!state.begin_workspace_close(wrong_scope));
    assert!(state.begin_workspace_close(operation.clone()));
    let admitted = WorkspaceCloseEvidence {
        operation: operation.clone(),
        result: WorkspaceCloseResult::WorkspaceAccepted(accepted(&operation)),
    };
    assert!(state.retain_workspace_close_evidence(&admitted));
    for terminal_context_id in [
        operation.baseline.terminal_context_id().unwrap().clone(),
        context_id(999),
    ] {
        let mut result = observed(&operation, true);
        let WorkspaceCloseResult::WorkspaceObserved { accepted, outcome } = &mut result else {
            unreachable!()
        };
        accepted.terminal_context_id = terminal_context_id.clone();
        let WorkbenchWorkspaceCloseReadOutcome::Available {
            terminal_context_id: actual,
            ..
        } = outcome.as_mut()
        else {
            unreachable!()
        };
        *actual = terminal_context_id;
        assert!(!state.finish_workspace_close(&WorkspaceCloseEvidence {
            operation: operation.clone(),
            result
        }));
        let retained = state.workspace_close_status().unwrap();
        assert_eq!(retained.operation_id(), Some(&operation.request_id));
        assert_eq!(retained.workspace_id(), Some(&workspace_id(1)));
        assert_eq!(
            retained.status(),
            super::super::workspace_close::WorkspaceCloseStatus::Closing
        );
        assert_eq!(state.workspace_close.pending.as_ref(), Some(&operation));
    }
    assert_eq!(state.desired(), Some(&desired));
    assert!(state.pane_attached(&pane));
    assert!(state.invariant_holds());
}
#[test]
fn project_close_all_names_explicit_project_counts_only_members_and_preserves_focus() {
    let mut state = close_state(2);
    let before = state.cache().clone();
    let token = state.open_project_close_all(&project_id()).unwrap();
    let prompt = state.workspace_close_confirmation().unwrap();
    assert_eq!(prompt.label(), "项目 Project");
    assert_eq!(prompt.project_member_count(), Some(2));
    assert_eq!(state.cache(), &before);
    assert!(plan(&state, token).is_none());
    state.acknowledge_workspace_close_draw(Some(token));
    let operation = plan(&state, token).unwrap();
    assert_eq!(
        operation.project_request().unwrap().action,
        WorkspaceGroupCloseAction::Start {
            operation_id: operation.request_id.clone(),
            project_id: project_id(),
        }
    );
    assert!(operation.workspace_command().is_none());
}
#[test]
fn empty_project_close_all_is_valid_from_empty_workbench() {
    let mut source = close_snapshot();
    source.navigation.workspaces.clear();
    source.navigation.workspace_order.clear();
    source.terminal_contexts.clear();
    let mut state = WorkbenchClientState::bootstrap(
        project_preferred_workspace(&source).unwrap(),
        1,
        geometry(80),
    )
    .unwrap()
    .0;
    let before = state.desired().cloned();
    let token = state.open_project_close_all(&project_id()).unwrap();
    assert_eq!(
        state
            .workspace_close_confirmation()
            .unwrap()
            .project_member_count(),
        Some(0)
    );
    state.acknowledge_workspace_close_draw(Some(token));
    let operation = plan(&state, token).unwrap();
    assert!(state.begin_workspace_close(operation.clone()));
    let WorkspaceCloseResult::ProjectObserved(mut reply) = group(&operation, true) else {
        unreachable!()
    };
    let WorkspaceGroupCloseOutcome::Available { members, .. } = &mut reply.outcome else {
        unreachable!()
    };
    members.clear();
    let evidence = WorkspaceCloseEvidence {
        operation,
        result: WorkspaceCloseResult::ProjectObserved(reply),
    };
    assert!(state.finish_workspace_close(&evidence));
    assert_eq!(state.desired(), before.as_ref());
    assert_eq!(state.cache().projects(), source.navigation.projects);
    assert!(state.invariant_holds());
}
#[test]
fn close_refuses_stale_draw_and_project_membership_drift_without_submission() {
    let mut state = close_state(0);
    let token = state.open_workspace_close().unwrap();
    state.cancel_workspace_close();
    let next = state.open_project_close_all(&project_id()).unwrap();
    assert_ne!(next, token);
    state.acknowledge_workspace_close_draw(Some(token));
    assert!(plan(&state, next).is_none());
    state.acknowledge_workspace_close_draw(Some(next));
    let mut source = close_snapshot();
    source.revision = WorkbenchRevision::new(2);
    source.navigation.workspaces[2].project_id = None;
    state.cache = state.cache.project_snapshot(&source).unwrap();
    assert!(plan(&state, next).is_none());
    assert!(state.settle_stale_workspace_close());
    assert!(state.workspace_close_confirmation().is_none());
    assert!(state.invariant_holds());
}
#[test]
fn unresolved_close_retains_identity_blocks_structure_and_never_starts_a_second_operation() {
    for case in 0..4 {
        let mut state = close_state(0);
        assert!(state.close_scope_available());
        let operation = start(&mut state, case == 3);
        let result = match case {
            0 => WorkspaceCloseResult::Unknown,
            1 => WorkspaceCloseResult::WorkspaceAccepted(accepted(&operation)),
            2 => observed(&operation, false),
            _ => group(&operation, false),
        };
        let evidence = WorkspaceCloseEvidence {
            operation: operation.clone(),
            result,
        };
        assert!(state.finish_workspace_close(&evidence));
        state.pending_refresh = None;
        assert_eq!(
            state.workspace_close_status().unwrap().operation_id(),
            Some(&operation.request_id)
        );
        assert!(state.workspace_close_blocks_structure());
        assert!(!state.close_scope_available());
        assert!(!state.workspace_close_blocks_membership());
        assert!(state.open_workspace_close().is_none());
        assert!(state.open_project_close_all(&project_id()).is_none());
        assert!(state.workspace_close.pending.is_none());
        assert!(state.invariant_holds());
    }
}
#[test]
fn admission_survives_later_unknown_and_correlated_reads_never_regress() {
    let mut state = close_state(0);
    let operation = start(&mut state, false);
    let admitted = WorkspaceCloseResult::WorkspaceAccepted(accepted(&operation));
    let mut evidence = WorkspaceCloseEvidence {
        operation: operation.clone(),
        result: admitted.clone(),
    };
    assert!(state.retain_workspace_close_evidence(&evidence));
    evidence.result = WorkspaceCloseResult::Unknown;
    assert!(state.retain_workspace_close_evidence(&evidence));
    assert_eq!(
        state.workspace_close_status().unwrap().status(),
        super::super::workspace_close::WorkspaceCloseStatus::Closing
    );
    assert_eq!(
        state.workspace_close_status().unwrap().operation_id(),
        Some(&accepted(&operation).close_request_id)
    );
    let pending = observed(&operation, false);
    let closed = observed(&operation, true);
    assert!(retains_close_result(&admitted, &pending));
    assert!(retains_close_result(&pending, &closed));
    for after in [WorkspaceCloseResult::Unknown, admitted, pending] {
        assert!(!retains_close_result(&closed, &after));
    }
    assert!(retains_close_result(&closed, &closed));
    evidence.result = closed;
    assert!(state.finish_workspace_close(&evidence));
    assert!(!state.workspace_close_blocks_structure());
    assert!(state.invariant_holds());
}
#[test]
fn close_scope_guards_reject_cross_kind_operation_workspace_context_and_revision_mismatches() {
    let mut state = close_state(0);
    let operation = start(&mut state, false);
    assert!(!operation.admits(&group(&operation, false)));
    for case in 0..3 {
        let mut result = accepted(&operation);
        match case {
            0 => result.workspace_id = workspace_id(1),
            1 => result.terminal_context_id = context_id(999),
            _ => result.accepted_revision = WorkbenchRevision::new(2),
        }
        assert!(!operation.admits(&WorkspaceCloseResult::WorkspaceAccepted(result)));
    }
    for case in 0..5 {
        let mut result = observed(&operation, true);
        let WorkspaceCloseResult::WorkspaceObserved { outcome, .. } = &mut result else {
            unreachable!()
        };
        let WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id,
            workspace_id,
            terminal_context_id,
            status,
            ..
        } = outcome.as_mut()
        else {
            unreachable!()
        };
        match case {
            0 => *close_request_id = WorkbenchRequestId::parse(&id(999)).unwrap(),
            1 => *workspace_id = WorkbenchWorkspaceId::parse(&id(999)).unwrap(),
            2 => *terminal_context_id = context_id(999),
            3 => {
                *status = WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(1),
                }
            }
            _ => **outcome = WorkbenchWorkspaceCloseReadOutcome::NotClosing {},
        }
        assert!(!operation.admits(&result));
    }
    let mut state = close_state(2);
    let project = start(&mut state, true);
    assert!(!project.admits(&observed(&operation, false)));
    for case in 0..3 {
        let WorkspaceCloseResult::ProjectObserved(mut reply) = group(&project, false) else {
            unreachable!()
        };
        let WorkspaceGroupCloseOutcome::Available {
            project_id,
            attempt,
            ..
        } = &mut reply.outcome
        else {
            unreachable!()
        };
        match case {
            0 => reply.operation_id = WorkbenchRequestId::parse(&id(999)).unwrap(),
            1 => *project_id = WorkbenchProjectId::parse(&id(999)).unwrap(),
            _ => *attempt = 0,
        }
        assert!(!project.admits(&WorkspaceCloseResult::ProjectObserved(reply)));
    }
}
#[test]
fn project_observations_preserve_members_children_and_completed_results() {
    let mut state = close_state(0);
    let operation = start(&mut state, true);
    let before = group(&operation, false);
    let closed = group(&operation, true);
    assert!(operation.admits(&before));
    assert!(operation.admits(&closed));
    assert!(retains_close_result(&before, &closed));
    assert!(!retains_close_result(&closed, &before));
    for case in 0..4 {
        let WorkspaceCloseResult::ProjectObserved(mut reply) = before.clone() else {
            unreachable!()
        };
        let WorkspaceGroupCloseOutcome::Available { members, .. } = &mut reply.outcome else {
            unreachable!()
        };
        match case {
            0 => {
                members.pop();
            }
            1 => members[0].workspace_id = workspace_id(2),
            2 => members[0].close = None,
            _ => {
                members[0].close.as_mut().unwrap().request_id =
                    WorkbenchRequestId::parse(&id(999)).unwrap()
            }
        }
        assert!(!retains_close_result(
            &before,
            &WorkspaceCloseResult::ProjectObserved(reply)
        ));
    }
}

#[test]
fn accepted_existing_close_keeps_canonical_identity_and_fences_every_subsequent_observation() {
    let mut state = close_state(0);
    let operation = start(&mut state, false);
    let mut canonical = accepted(&operation);
    canonical.close_request_id = WorkbenchRequestId::parse(&id(999)).unwrap();
    assert!(
        state.retain_workspace_close_evidence(&WorkspaceCloseEvidence {
            operation: operation.clone(),
            result: WorkspaceCloseResult::WorkspaceAccepted(canonical.clone()),
        })
    );
    assert_eq!(
        state.workspace_close_status().unwrap().operation_id(),
        Some(&canonical.close_request_id)
    );
    assert!(
        !state.retain_workspace_close_evidence(&WorkspaceCloseEvidence {
            operation: operation.clone(),
            result: observed(&operation, true),
        })
    );
    let WorkspaceCloseResult::WorkspaceObserved { outcome, .. } = observed(&operation, true) else {
        unreachable!()
    };
    let mut outcome = *outcome;
    let WorkbenchWorkspaceCloseReadOutcome::Available {
        close_request_id, ..
    } = &mut outcome
    else {
        unreachable!()
    };
    *close_request_id = canonical.close_request_id.clone();
    assert!(state.finish_workspace_close(&WorkspaceCloseEvidence {
        operation,
        result: WorkspaceCloseResult::WorkspaceObserved {
            accepted: canonical.clone(),
            outcome: Box::new(outcome)
        },
    }));
    assert_eq!(
        state.workspace_close_status().unwrap().operation_id(),
        Some(&canonical.close_request_id)
    );
}

#[test]
fn workspace_observations_keep_exact_member_identity_attempt_and_irreversible_disposal() {
    use orchbee_workbench_protocol::{
        WorkbenchTabClosePublicationReason as Reason, WorkbenchWorkspaceCloseMember,
        WorkbenchWorkspaceCloseMemberStatus as Status,
    };
    let mut state = close_state(0);
    let operation = start(&mut state, false);
    let mut before = observed(&operation, false);
    let WorkspaceCloseResult::WorkspaceObserved { outcome, .. } = &mut before else {
        unreachable!()
    };
    let WorkbenchWorkspaceCloseReadOutcome::Available {
        members,
        counts,
        attempt_generation,
        ..
    } = outcome.as_mut()
    else {
        unreachable!()
    };
    *attempt_generation = WorkbenchTabCloseAttemptGeneration::new(2).unwrap();
    counts.disposed_awaiting_publication = 1;
    members.push(WorkbenchWorkspaceCloseMember {
        tab_id: WorkbenchTabId::parse(&id(800)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(801)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(802)).unwrap(),
        status: Status::DisposedAwaitingPublication {
            reason: Reason::Preparing,
            resumable: false,
        },
    });
    for case in 0..5 {
        let mut after = before.clone();
        let WorkspaceCloseResult::WorkspaceObserved { outcome, .. } = &mut after else {
            unreachable!()
        };
        let WorkbenchWorkspaceCloseReadOutcome::Available {
            members,
            attempt_generation,
            ..
        } = outcome.as_mut()
        else {
            unreachable!()
        };
        match case {
            0 => *attempt_generation = WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
            1 => members[0].presentation_id = TerminalPresentationId::parse(&id(999)).unwrap(),
            2 => {
                members[0].status = Status::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                }
            }
            3 => members.clear(),
            _ => *attempt_generation = WorkbenchTabCloseAttemptGeneration::new(3).unwrap(),
        }
        assert_eq!(retains_close_result(&before, &after), case == 4);
    }
}
#[test]
fn late_completion_preserves_later_focus_and_the_next_operation_identity() {
    let mut state = close_state(0);
    let first = start(&mut state, false);
    let evidence = WorkspaceCloseEvidence {
        operation: first.clone(),
        result: observed(&first, true),
    };
    assert!(state.finish_workspace_close(&evidence));
    state.pending_refresh = None;
    state.cache = close_state(2).cache;
    let later = state.cache.clone();
    let token = state.open_workspace_close().unwrap();
    state.acknowledge_workspace_close_draw(Some(token));
    let second = state
        .plan_workspace_close(
            token,
            WorkbenchRequestId::parse(&id(391)).unwrap(),
            3,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(state.begin_workspace_close(second.clone()));
    assert!(state.finish_workspace_close(&evidence));
    assert_eq!(state.cache, later);
    assert_eq!(state.workspace_close.pending.as_ref(), Some(&second));
    assert!(state.workspace_close_status().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn closed_receipt_survives_exact_automatic_canonical_landing_but_not_manual_navigation_or_return() {
    let mut source = close_snapshot();
    // Close the first displayed Workspace and land on the next, empty Workspace.
    // The initial nonempty Workspace remains a real, later navigation target.
    source.navigation.workspace_order.rotate_left(1);
    let initial_workspace = source.navigation.workspaces[0].workspace_id.clone();
    let location = ClientContextLocation::Workspace {
        workspace_id: workspace_id(0),
        terminal_context_id: context_id(372),
    };
    let mut state = WorkbenchClientState::bootstrap(
        project_native_context_tabs(&source, &location).unwrap(),
        1,
        geometry(80),
    )
    .unwrap()
    .0;
    let operation = start(&mut state, false);
    let evidence = WorkspaceCloseEvidence {
        operation: operation.clone(),
        result: observed(&operation, true),
    };
    assert!(state.finish_workspace_close(&evidence));
    let mut published = source;
    published.revision = WorkbenchRevision::new(2);
    published.navigation.workspaces.remove(1);
    published
        .navigation
        .workspace_order
        .retain(|workspace| workspace != &workspace_id(0));
    published.terminal_contexts.remove(1);
    assert_eq!(
        published.navigation.workspace_order,
        [workspace_id(1), workspace_id(2), initial_workspace]
    );
    let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
        operation.generation,
        WorkbenchSnapshotOutcome::Available {
            snapshot: published.clone(),
        },
    ) else {
        panic!("exact automatic removal candidate");
    };
    assert!(matches!(
        state
            .prepare_refresh_membership_install(
                operation.generation,
                prepared,
                None,
                Some(OperationDeadlines::from_sample(Instant::now()).unwrap())
            )
            .unwrap()
            .commit(),
        MembershipInstallDecision::Empty
    ));
    let landing = ClientContextLocation::Workspace {
        workspace_id: workspace_id(1),
        terminal_context_id: context_id(373),
    };
    assert_eq!(state.cache().location(), &landing);
    let view = state.workspace_close_status().unwrap();
    assert_eq!(
        view.status(),
        crate::workbench_client::WorkspaceCloseStatus::Closed
    );
    assert_eq!(view.operation_id(), Some(&operation.request_id));
    assert_eq!(view.workspace_id(), Some(&workspace_id(0)));
    assert!(state.desired().is_none());
    assert!(state.attached_key().is_none());
    // Visit the other empty Workspace and the nonempty initial Workspace, returning
    // to the automatic landing each time through the real selection owner.
    for (generation, index) in [(3, 1), (4, 0), (5, 2), (6, 0)] {
        assert!(state.start_context_selection_at(
            generation,
            index,
            OperationDeadlines::from_sample(Instant::now()).unwrap()
        ));
        super::context_selection::finish_selection(&mut state, published.clone(), generation);
        assert!(
            state.workspace_close_status().is_none(),
            "manual target index {index}"
        );
        assert!(state.invariant_holds());
    }
    assert_eq!(state.cache().location(), &landing);
    assert!(state.retain_workspace_close_evidence(&evidence));
    assert!(state.workspace_close_status().is_none());
}

#[test]
fn closed_current_workspace_reads_as_closing_until_its_viewer_cleanup_installs_the_landing() {
    // Live viewers on the closed current Workspace defer the validated removal behind
    // attachment cleanup. The status must not announce Closed while that Workspace is
    // still the installed, drawn current location.
    let mut state = background_state(false);
    let closed = state.cache().location().clone();
    let old = state.desired().unwrap().clone();
    let operation = start(&mut state, false);
    let WorkspaceCloseTarget::Workspace { workspace_id, .. } = &operation.target else {
        panic!("Workspace target");
    };
    assert!(state.finish_workspace_close(&WorkspaceCloseEvidence {
        operation: operation.clone(),
        result: observed(&operation, true),
    }));
    let mut published = operation.baseline.canonical_snapshot().clone();
    published.revision = WorkbenchRevision::new(2);
    let removed = published
        .navigation
        .workspace(workspace_id)
        .unwrap()
        .terminal_context_id
        .clone();
    published
        .navigation
        .workspaces
        .retain(|workspace| &workspace.workspace_id != workspace_id);
    published
        .navigation
        .workspace_order
        .retain(|workspace| workspace != workspace_id);
    published
        .terminal_contexts
        .retain(|context| context.terminal_context_id != removed);
    let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
        operation.generation,
        WorkbenchSnapshotOutcome::Available {
            snapshot: published,
        },
    ) else {
        panic!("exact removal candidate");
    };
    assert!(state.has_attachments() && !prepared.preserves_selection());
    let cleanup = state
        .stage_refresh_membership_cleanup(
            operation.generation,
            prepared,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert_eq!(
        state.cache().location(),
        &closed,
        "cleanup precedes landing"
    );
    assert_eq!(
        state.workspace_close_status().unwrap().status(),
        crate::workbench_client::WorkspaceCloseStatus::Closing
    );
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, old.anchor())
        .unwrap();
    let landing = prepared
        .selected_tab_id()
        .map(|tab| AttachmentKey::new(old.generation() + 1, tab.clone()));
    state
        .prepare_cleaned_membership_install(&cleanup, prepared, landing)
        .unwrap()
        .commit();
    assert_ne!(state.cache().location(), &closed);
    let view = state.workspace_close_status().unwrap();
    assert_eq!(
        view.status(),
        crate::workbench_client::WorkspaceCloseStatus::Closed
    );
    assert_eq!(view.operation_id(), Some(&operation.request_id));
    assert!(state.invariant_holds());
}
#[test]
fn automatic_nonempty_sibling_workspace_landing_uses_the_same_permit_without_new_ids_or_controller_authority()
 {
    let mut source = close_snapshot();
    source.navigation.workspaces.truncate(2);
    source.navigation.workspace_order.truncate(2);
    source.terminal_contexts.truncate(2);
    let workspace = &source.navigation.workspaces[1];
    let location = ClientContextLocation::Workspace {
        workspace_id: workspace.workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    };
    let mut state = WorkbenchClientState::bootstrap(
        project_native_context_tabs(&source, &location).unwrap(),
        1,
        geometry(80),
    )
    .unwrap()
    .0;
    let operation = start(&mut state, false);
    assert!(state.finish_workspace_close(&WorkspaceCloseEvidence {
        operation: operation.clone(),
        result: observed(&operation, true),
    }));
    let legacy = source.terminal_contexts[0].clone();
    source.navigation.workspaces.truncate(1);
    source.navigation.workspace_order.truncate(1);
    source.terminal_contexts.truncate(1);
    source.revision = WorkbenchRevision::new(2);
    let PreparedRefresh::Membership { prepared, .. } = state.prepare_refresh(
        operation.generation,
        WorkbenchSnapshotOutcome::Available {
            snapshot: source.clone(),
        },
    ) else {
        panic!("exact legacy removal candidate");
    };
    let desired = AttachmentKey::new(3, prepared.selected_tab_id().unwrap().clone());
    assert!(matches!(
        state
            .prepare_refresh_membership_install(
                operation.generation,
                prepared,
                Some(desired),
                Some(OperationDeadlines::from_sample(Instant::now()).unwrap())
            )
            .unwrap()
            .commit(),
        MembershipInstallDecision::Attach(_)
    ));
    assert_eq!(state.cache().location(), &{
        let fixture_context_id = source.navigation.workspaces[0].terminal_context_id.clone();
        ClientContextLocation::Workspace {
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap(),
            terminal_context_id: fixture_context_id,
        }
    });
    assert_eq!(state.cache().canonical_context(), Some(&legacy));
    let view = state.workspace_close_status().unwrap();
    assert_eq!(
        view.status(),
        crate::workbench_client::WorkspaceCloseStatus::Closed
    );
    assert_eq!(view.operation_id(), Some(&operation.request_id));
    assert_eq!(view.workspace_id(), Some(&workspace_id(0)));
    assert_eq!(state.controller_mode(), None);
    assert!(state.attached_key().is_none());
    assert!(state.invariant_holds());
}

#[test]
fn workspace_and_project_unknown_observations_cannot_be_promoted_or_restarted() {
    use orchbee_workbench_protocol::{
        WorkbenchTabCloseUnknownPhase, WorkbenchTabCloseUnknownReason, WorkspaceGroupCloseFailure,
    };
    let unknown = WorkbenchTabCloseStatus::OutcomeUnknown {
        phase: WorkbenchTabCloseUnknownPhase::Dispose,
        reason: WorkbenchTabCloseUnknownReason::TransportLost,
    };
    let mut state = close_state(0);
    let operation = start(&mut state, false);
    let mut before = observed(&operation, false);
    let WorkspaceCloseResult::WorkspaceObserved { outcome, .. } = &mut before else {
        unreachable!()
    };
    let WorkbenchWorkspaceCloseReadOutcome::Available { status, .. } = outcome.as_mut() else {
        unreachable!()
    };
    *status = unknown.clone();
    assert!(retains_close_result(&before, &before));
    assert!(!retains_close_result(&before, &observed(&operation, false)));
    assert!(!retains_close_result(&before, &observed(&operation, true)));
    let mut later_attempt = before.clone();
    let WorkspaceCloseResult::WorkspaceObserved { outcome, .. } = &mut later_attempt else {
        unreachable!()
    };
    let WorkbenchWorkspaceCloseReadOutcome::Available {
        attempt_generation, ..
    } = outcome.as_mut()
    else {
        unreachable!()
    };
    *attempt_generation = WorkbenchTabCloseAttemptGeneration::new(2).unwrap();
    assert!(!retains_close_result(&before, &later_attempt));

    let mut state = close_state(0);
    let operation = start(&mut state, true);
    let mut before = group(&operation, false);
    let WorkspaceCloseResult::ProjectObserved(reply) = &mut before else {
        unreachable!()
    };
    let WorkspaceGroupCloseOutcome::Available { members, .. } = &mut reply.outcome else {
        unreachable!()
    };
    members[0].close.as_mut().unwrap().observation = Some(WorkspaceGroupCloseObservation {
        attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
        status: unknown,
    });
    assert!(retains_close_result(&before, &before));
    assert!(!retains_close_result(&before, &group(&operation, true)));
    assert!(!retains_close_result(&before, &group(&operation, false)));
    let mut failed = before.clone();
    let WorkspaceCloseResult::ProjectObserved(reply) = &mut failed else {
        unreachable!()
    };
    let WorkspaceGroupCloseOutcome::Available { status, .. } = &mut reply.outcome else {
        unreachable!()
    };
    *status = WorkspaceGroupCloseStatus::Failed {
        reason: WorkspaceGroupCloseFailure::OutcomeUnknown,
        resumable: false,
    };
    assert!(retains_close_result(&before, &failed));
    let mut restarted = failed.clone();
    let WorkspaceCloseResult::ProjectObserved(reply) = &mut restarted else {
        unreachable!()
    };
    let WorkspaceGroupCloseOutcome::Available { attempt, .. } = &mut reply.outcome else {
        unreachable!()
    };
    *attempt = 2;
    assert!(!retains_close_result(&failed, &restarted));
}
#[test]
fn close_pending_indicator_uses_injected_time_and_localized_recovery_matches_typed_scope() {
    for project in [false, true] {
        let mut state = close_state(0);
        let operation = start(&mut state, project);
        let start = operation.deadlines.total() - OPERATION_TOTAL;
        assert!(!state.tick_workspace_close_pending(start + Duration::from_millis(199)));
        assert!(state.workspace_close_status().is_none());
        assert!(state.tick_workspace_close_pending(start + Duration::from_millis(200)));
        assert!(!state.tick_workspace_close_pending(start + Duration::from_millis(201)));
        let view = state.workspace_close_status().unwrap();
        for locale in [Locale::EnUs, Locale::ZhCn] {
            // A pending close names no status command; only an unknown outcome needs one.
            let text = locale.workspace_close_status(view);
            assert!(!text.contains("close-status"), "{text}");
            assert_eq!(text, locale.workspace_close_status_shorter(view)[0]);
        }
    }
}
