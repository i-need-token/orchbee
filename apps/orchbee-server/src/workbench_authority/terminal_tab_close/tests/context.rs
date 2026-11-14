mod idle;
mod worktree_hold;
use super::workspace::workspace_fixture;
use super::*;
use crate::workbench_authority::{
    WorkbenchTabMoveBegin, WorkbenchWorkspaceCloseBegin, WorkbenchWorkspaceCloseResumeBegin,
    WorkbenchWorkspaceCloseResumePreparation,
};
use orchbee_capability_node_protocol::{
    LifecycleStepOrdinal, TerminalLifecycleAction, TerminalLifecycleAuthorization,
};
use orchbee_workbench_protocol::{
    WorkbenchPanelSplitDirection, WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseResume, WorkbenchWorkspaceCloseResumeOutcome,
};

fn workspace_close_command(
    authority: &WorkbenchAuthority,
    id: u128,
    project_index: usize,
    workspace_index: usize,
) -> WorkbenchCommand {
    let project = &authority.projection.snapshot().navigation.projects[project_index];
    command(
        authority,
        id,
        WorkbenchAction::WorkspaceClose {
            workspace_id: authority
                .snapshot()
                .navigation
                .workspaces_in_project(Some(&project.project_id))
                .nth(workspace_index)
                .unwrap()
                .workspace_id
                .clone(),
        },
    )
}

fn primary_workspace<'a>(
    authority: &'a mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchTabClosePrimaryAdmission<'a> {
    let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
        authority.begin_workspace_close(Some(grant), command)
    else {
        panic!("Workspace Close admission")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("primary Workspace Close")
    };
    *primary
}

fn accept_workspace(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
    now: Instant,
) -> WorkbenchCommandResult {
    let primary = primary_workspace(authority, grant, command);
    let runtimes = primary
        .members()
        .iter()
        .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(runtimes, now)
    else {
        panic!("Workspace Close preparation")
    };
    prepared.commit()
}

fn workspace_status(
    authority: &WorkbenchAuthority,
    grant: &WorkbenchGrant,
    workspace_id: &WorkbenchWorkspaceId,
) -> WorkbenchWorkspaceCloseReadOutcome {
    authority
        .read_workspace_close(
            Some(grant),
            WorkbenchWorkspaceCloseRead {
                protocol_version: ProtocolVersion,
                request_id: request_id(99000),
                session_id: authority.session_id().clone(),
                workspace_id: workspace_id.clone(),
            },
        )
        .outcome
}

fn step_request(
    close_id: &WorkbenchRequestId,
    target: &WorkbenchTabCloseTarget,
    action: TerminalLifecycleAction,
    step_ordinal: LifecycleStepOrdinal,
    generation: WorkbenchTabCloseAttemptGeneration,
) -> TerminalLifecycleRequest {
    let runtime = target.runtime().expect("created Host runtime");
    TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: runtime.terminal_id(),
            worker_id: runtime.worker_id(),
            worker_generation: runtime.worker_generation(),
            binding_fence: runtime.fence(),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes(8000_u128.to_be_bytes()).unwrap(),
                LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
            ),
        },
        close_operation_id: OperationId::from_bytes(
            u128::from_str_radix(close_id.as_str(), 16)
                .unwrap()
                .to_be_bytes(),
        )
        .unwrap(),
        request_id: OperationId::from_bytes(
            (8100 + u128::from(runtime.terminal_id().as_bytes()[15])).to_be_bytes(),
        )
        .unwrap(),
        attempt_generation: NonZeroU64::new(generation.value()).unwrap(),
        step_ordinal,
        target: runtime.host_identity(),
        action,
    }
}

fn settle_disposed(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    step: WorkbenchTabCloseStepTarget,
    now: Instant,
) {
    settle_step(
        authority,
        close_id,
        step,
        now,
        Some(TerminalLifecycleOutcome::Disposed),
    );
}

/// Settles the scheduled lifecycle step with `outcome`, or as not delivered when it is `None`.
fn settle_step(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    step: WorkbenchTabCloseStepTarget,
    now: Instant,
    outcome: Option<TerminalLifecycleOutcome>,
) {
    let target = step.target().expect("targeted lifecycle step");
    let CloseContinuationNext::Lifecycle {
        action,
        step_ordinal,
    } = step.next()
    else {
        panic!("expected lifecycle step");
    };
    let generation = authority.terminal_tab_close_generation(close_id).unwrap();
    let request = step_request(close_id, target, action, step_ordinal, generation);
    let token = authority
        .reserve_terminal_tab_close_step(close_id, now, request)
        .unwrap();
    authority
        .confirm_terminal_tab_close_step_admitted(close_id, &token, now)
        .unwrap();
    let result = outcome.map(|outcome| TerminalLifecycleResult::new(request, outcome).unwrap());
    let observation = result.as_ref().map_or(
        CloseStepObservation::NotDelivered,
        CloseStepObservation::Result,
    );
    authority
        .settle_terminal_tab_close_step(close_id, token, now, observation)
        .unwrap();
}

fn drive_close_to_publication(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    now: Instant,
) -> WorkbenchTabCloseStatus {
    loop {
        let step = authority
            .next_terminal_tab_close_step_target(close_id, now)
            .unwrap();
        match step.next() {
            CloseContinuationNext::Lifecycle { .. } => {
                settle_disposed(authority, close_id, step, now)
            }
            CloseContinuationNext::TargetCleanup => {
                let runtime = step
                    .target()
                    .unwrap()
                    .runtime()
                    .expect("created Host runtime");
                authority
                    .complete_terminal_tab_close_target_cleanup(close_id, runtime)
                    .unwrap();
            }
            CloseContinuationNext::Publication => {
                return authority
                    .prepare_terminal_tab_close_publication(close_id, now)
                    .unwrap()
                    .commit();
            }
            next => panic!("unexpected Close step {next:?}"),
        }
    }
}

fn workspace_resume_request(
    close: &WorkbenchCommand,
    id: u128,
    generation: u64,
) -> WorkbenchWorkspaceCloseResume {
    let WorkbenchAction::WorkspaceClose { workspace_id } = &close.action else {
        panic!()
    };
    WorkbenchWorkspaceCloseResume {
        protocol_version: ProtocolVersion,
        request_id: request_id(id),
        session_id: close.session_id.clone(),
        workspace_id: workspace_id.clone(),
        close_request_id: close.request_id.clone(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(generation).unwrap(),
    }
}

fn workspace_resume(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    request: WorkbenchWorkspaceCloseResume,
    now: Instant,
) -> orchbee_workbench_protocol::WorkbenchWorkspaceCloseResumeResult {
    match authority.begin_workspace_close_resume(Some(grant), request) {
        WorkbenchWorkspaceCloseResumeBegin::Ready(result) => result,
        WorkbenchWorkspaceCloseResumeBegin::Admitted(admission) => match admission.finish(now) {
            WorkbenchWorkspaceCloseResumePreparation::Ready(result) => result,
            WorkbenchWorkspaceCloseResumePreparation::Prepared(prepared) => prepared.commit(),
        },
    }
}

#[test]
fn empty_workspace_close_publishes_without_lifecycle_and_preserves_project_directories() {
    let (mut authority, grant) = workspace_fixture(0);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let WorkbenchAction::WorkspaceClose { workspace_id } = &close.action else {
        panic!()
    };
    let workspace_id = workspace_id.clone();
    let before = authority.projection.snapshot().clone();
    let context_id = before.navigation.workspaces[2].terminal_context_id.clone();

    let primary = primary_workspace(&mut authority, &grant, close.clone());
    assert!(primary.members().is_empty());
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(Vec::new(), now)
    else {
        panic!("empty Workspace Close preparation")
    };
    let accepted = prepared.commit();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            ref terminal_context_id,
            accepted_revision,
            ..
        } if *terminal_context_id == context_id
            && accepted_revision == close.expected_revision
    ));
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
    assert_eq!(authority.projection.snapshot(), &before);
    assert!(matches!(
        workspace_status(&authority, &grant, &workspace_id),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued
            },
            ref members,
            ref counts,
            ..
        } if members.is_empty()
            && counts.closing == 0
            && counts.disposed_awaiting_publication == 0
    ));

    // No Worker lifecycle steps exist: the scheduled step is the publication itself.
    let step = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    assert_eq!(step.next(), CloseContinuationNext::Publication);
    assert!(step.target().is_none());
    assert!(matches!(
        workspace_status(&authority, &grant, &workspace_id),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason: WorkbenchTabClosePublicationReason::Preparing,
                resumable: false,
            },
            ..
        }
    ));

    let prepared = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap();
    assert!(prepared.exact_removed_members().is_empty());

    assert!(prepared.before_membership().is_empty());
    assert!(prepared.after_membership().is_empty());
    let closed = prepared.commit();
    let WorkbenchTabCloseStatus::Closed { revision } = closed else {
        panic!("Workspace Close must close")
    };

    let mut expected = before.clone();
    expected.revision = revision;
    expected
        .terminal_contexts
        .retain(|context| context.terminal_context_id != context_id);
    expected
        .navigation
        .workspaces
        .retain(|workspace| workspace.workspace_id != workspace_id);
    expected
        .navigation
        .workspace_order
        .retain(|id| *id != workspace_id);
    assert_eq!(authority.projection.snapshot(), &expected);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(0));

    // The tombstone keeps answering on the removed scope; the command replays.
    assert!(matches!(
        workspace_status(&authority, &grant, &workspace_id),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));
    let WorkbenchWorkspaceCloseBegin::Ready(replayed) =
        authority.begin_workspace_close(Some(&grant), close.clone())
    else {
        panic!("Workspace Close replay")
    };
    assert_eq!(replayed, accepted);
    // A fresh scope request against the removed Workspace rejects atomically.
    let stale = command(
        &authority,
        10001,
        WorkbenchAction::WorkspaceClose {
            workspace_id: workspace_id.clone(),
        },
    );
    assert!(matches!(
        authority.begin_workspace_close(Some(&grant), stale),
        WorkbenchWorkspaceCloseBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::WorkspaceUnavailable,
                ..
            },
            ..
        })
    ));
    assert!(!authority.retired);
}

#[test]
fn populated_workspace_close_lifecycles_every_member_then_removes_scope() {
    let (mut authority, grant) = workspace_fixture(2);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let WorkbenchAction::WorkspaceClose { workspace_id } = &close.action else {
        panic!()
    };
    let workspace_id = workspace_id.clone();
    let context_id = authority.projection.snapshot().navigation.workspaces[2]
        .terminal_context_id
        .clone();
    let accepted = accept_workspace(&mut authority, &grant, close.clone(), now);
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
    ));

    let WorkbenchWorkspaceCloseReadOutcome::Available {
        members,
        counts,
        terminal_context_id,
        status,
        ..
    } = workspace_status(&authority, &grant, &workspace_id)
    else {
        panic!("read must be available")
    };
    assert_eq!(terminal_context_id, context_id);
    assert_eq!(members.len(), 2);
    assert_eq!(counts.closing, 2);
    assert!(matches!(status, WorkbenchTabCloseStatus::Closing { .. }));

    let closed = drive_close_to_publication(&mut authority, &close.request_id, now);
    assert!(matches!(closed, WorkbenchTabCloseStatus::Closed { .. }));
    let snapshot = authority.projection.snapshot();
    assert!(
        !snapshot
            .terminal_contexts
            .iter()
            .any(|context| context.terminal_context_id == context_id)
    );
    assert!(
        !snapshot
            .navigation
            .workspaces
            .iter()
            .any(|workspace| workspace.workspace_id == workspace_id)
    );
    // Projects, Directories, the sibling Workspace, and the fixture Context are untouched.
    assert_eq!(snapshot.navigation.projects.len(), 2);
    assert_eq!(snapshot.navigation.directories.len(), 3);
    assert_eq!(snapshot.terminal_contexts.len(), 2);
    assert!(matches!(
        workspace_status(&authority, &grant, &workspace_id),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));
    assert!(!authority.retired);
}

#[test]
fn scope_join_replays_acceptance_member_conflict_is_busy_and_disjoint_scope_proceeds() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let accepted = accept_workspace(&mut authority, &grant, close.clone(), now);
    let context_id = authority.projection.snapshot().navigation.workspaces[2]
        .terminal_context_id
        .clone();

    // Identical scope joins the retained operation without a new close credit.
    let alias = workspace_close_command(&authority, 10001, 1, 0);
    let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
        authority.begin_workspace_close(Some(&grant), alias.clone())
    else {
        panic!("alias admission")
    };
    let WorkbenchTabCloseResolution::Ready(joined) = admission.resolve_after_composition() else {
        panic!("same scope must join, not conflict")
    };
    assert_eq!(joined.outcome, accepted.outcome);
    assert_eq!(joined.request_id, alias.request_id);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));

    // A member-level close inside the open Context scope is busy, not a join.
    let member = authority
        .terminal_tree_membership(&context_id)
        .unwrap()
        .members()[0]
        .clone();
    let leaf = command(
        &authority,
        10002,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id.clone(),
            tab_id: member.tab_id().clone(),
        },
    );
    let WorkbenchTabCloseBegin::Admitted(leaf_admission) =
        authority.begin_terminal_tab_close(Some(&grant), leaf.clone())
    else {
        panic!("leaf admission")
    };
    assert!(matches!(
        leaf_admission.resolve_after_composition(),
        WorkbenchTabCloseResolution::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            },
            ..
        })
    ));

    // The disjoint sibling Workspace scope admits independently.
    let other = workspace_close_command(&authority, 10003, 0, 0);
    let accepted_other = accept_workspace(&mut authority, &grant, other.clone(), now);
    assert!(matches!(
        accepted_other.outcome,
        WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
    ));
    assert_eq!(authority.outstanding_close_credits().ok(), Some(2));
}

#[test]
fn member_close_first_then_workspace_close_is_busy() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let context_id = authority.projection.snapshot().navigation.workspaces[2]
        .terminal_context_id
        .clone();
    let member = authority
        .terminal_tree_membership(&context_id)
        .unwrap()
        .members()[0]
        .clone();
    let leaf = command(
        &authority,
        10000,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id.clone(),
            tab_id: member.tab_id().clone(),
        },
    );
    accept(&mut authority, &grant, leaf, now);
    let close = workspace_close_command(&authority, 10001, 1, 0);
    let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
        authority.begin_workspace_close(Some(&grant), close)
    else {
        panic!("workspace admission")
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
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
}

#[test]
fn open_context_close_fences_create_move_split_and_member_close() {
    let (mut authority, grant) = workspace_fixture(2);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let context_id = authority.projection.snapshot().navigation.workspaces[2]
        .terminal_context_id
        .clone();
    accept_workspace(&mut authority, &grant, close.clone(), now);
    let retained = authority.ledger.len();
    let member = authority
        .terminal_tree_membership(&context_id)
        .unwrap()
        .members()[0]
        .clone();

    let create = command(
        &authority,
        10001,
        WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: context_id.clone(),
            label: WorkbenchLabel::parse("fenced create").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    assert!(matches!(
        authority.begin_terminal_tab_create(Some(&grant), create),
        WorkbenchTabCreateBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            },
            ..
        })
    ));
    let movement = command(
        &authority,
        10002,
        WorkbenchAction::TabMove {
            terminal_context_id: context_id.clone(),
            tab_id: member.tab_id().clone(),
            direction: WorkbenchTabMoveDirection::Last,
        },
    );
    assert!(matches!(
        authority.begin_terminal_tab_move(Some(&grant), movement),
        WorkbenchTabMoveBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            },
            ..
        })
    ));
    let indexed = command(
        &authority,
        10006,
        WorkbenchAction::TabMoveIndex {
            terminal_context_id: context_id.clone(),
            tab_id: member.tab_id().clone(),
            insert_index: orchbee_workbench_protocol::WorkbenchTabInsertIndex::new(256).unwrap(),
        },
    );
    assert!(matches!(
        authority.begin_terminal_tab_move(Some(&grant), indexed),
        WorkbenchTabMoveBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            },
            ..
        })
    ));
    let split = command(
        &authority,
        10003,
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: context_id.clone(),
            tab_id: member.tab_id().clone(),
            panel_id: member.panel_id().clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("fenced split").unwrap(),
            custom_label: None,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    assert!(matches!(
        authority.begin_terminal_panel_create(Some(&grant), split),
        crate::workbench_authority::WorkbenchPanelCreateBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            },
            ..
        })
    ));
    let leaf = command(
        &authority,
        10004,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id.clone(),
            tab_id: member.tab_id().clone(),
        },
    );
    let WorkbenchTabCloseBegin::Admitted(leaf_admission) =
        authority.begin_terminal_tab_close(Some(&grant), leaf)
    else {
        panic!("leaf admission")
    };
    assert!(matches!(
        leaf_admission.resolve_after_composition(),
        WorkbenchTabCloseResolution::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            },
            ..
        })
    ));
    assert_eq!(authority.ledger.len(), retained);

    // The sibling context is untouched by the fence.
    let other_context = authority.projection.snapshot().navigation.workspaces[1]
        .terminal_context_id
        .clone();
    let other_create = command(
        &authority,
        10005,
        WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: other_context,
            label: WorkbenchLabel::parse("other create").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    assert!(matches!(
        authority.begin_terminal_tab_create(Some(&grant), other_create),
        WorkbenchTabCreateBegin::Admitted(_)
    ));
}

#[test]
fn pending_create_covers_context_and_blocks_workspace_close_admission() {
    let (mut authority, grant) = workspace_fixture(1);
    let context_id = authority.projection.snapshot().navigation.workspaces[2]
        .terminal_context_id
        .clone();
    let create = command(
        &authority,
        10000,
        WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: context_id.clone(),
            label: WorkbenchLabel::parse("pending sibling").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&grant), create.clone())
    else {
        panic!("Create admission")
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(3200), None)
    else {
        panic!("Create preparation")
    };
    let _pending = prepared.commit().into_pending();

    let close = workspace_close_command(&authority, 10001, 1, 0);
    let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
        authority.begin_workspace_close(Some(&grant), close)
    else {
        panic!("workspace admission")
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
}

#[test]
fn resumable_populated_workspace_close_opens_a_new_generation_and_completes() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let WorkbenchAction::WorkspaceClose { workspace_id } = &close.action else {
        panic!()
    };
    let workspace_id = workspace_id.clone();
    accept_workspace(&mut authority, &grant, close.clone(), now);
    spend_automatic_renewals(&mut authority, &close.request_id);
    let parked = now + Duration::from_secs(30);
    assert_eq!(
        authority
            .next_terminal_tab_close_step_target(&close.request_id, parked)
            .unwrap()
            .next(),
        CloseContinuationNext::Parked
    );

    let accepted = workspace_resume(
        &mut authority,
        &grant,
        workspace_resume_request(&close, 11000, 1),
        parked,
    );
    assert!(matches!(
        accepted.outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Resumed {
            attempt_generation,
        } if attempt_generation == WorkbenchTabCloseAttemptGeneration::new(2).unwrap()
    ));
    // The same receipt replays; a stale generation is fenced.
    let replayed = workspace_resume(
        &mut authority,
        &grant,
        workspace_resume_request(&close, 11000, 1),
        parked,
    );
    assert_eq!(replayed, accepted);
    let stale = workspace_resume(
        &mut authority,
        &grant,
        workspace_resume_request(&close, 11001, 1),
        parked,
    );
    assert!(matches!(
        stale.outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Stale {
            attempt_generation,
        } if attempt_generation == WorkbenchTabCloseAttemptGeneration::new(2).unwrap()
    ));

    let closed = drive_close_to_publication(&mut authority, &close.request_id, parked);
    assert!(matches!(closed, WorkbenchTabCloseStatus::Closed { .. }));
    assert!(matches!(
        workspace_status(&authority, &grant, &workspace_id),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { .. },
            ..
        }
    ));
    assert!(!authority.retired);
}

#[test]
fn empty_workspace_parked_publication_resumes_then_publishes() {
    let (mut authority, grant) = workspace_fixture(0);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let WorkbenchAction::WorkspaceClose { workspace_id } = &close.action else {
        panic!()
    };
    let workspace_id = workspace_id.clone();
    accept_workspace(&mut authority, &grant, close.clone(), now);
    spend_automatic_renewals(&mut authority, &close.request_id);
    // A failed publication parks the empty scope resumably.
    authority
        .close_record_mut(&close.request_id)
        .unwrap()
        .publication_failed(WorkbenchTabClosePublicationReason::PreparationFailed);
    assert!(matches!(
        workspace_status(&authority, &grant, &workspace_id),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason: WorkbenchTabClosePublicationReason::PreparationFailed,
                resumable: true,
            },
            ..
        }
    ));
    assert_eq!(
        authority
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap()
            .next(),
        CloseContinuationNext::Parked
    );
    let accepted = workspace_resume(
        &mut authority,
        &grant,
        workspace_resume_request(&close, 11000, 1),
        now,
    );
    assert!(matches!(
        accepted.outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
    ));
    let closed = drive_close_to_publication(&mut authority, &close.request_id, now);
    assert!(matches!(closed, WorkbenchTabCloseStatus::Closed { .. }));
    assert!(!authority.retired);
}

/// A retryable step failure keeps the close `closing`: the Server renews two expired attempts,
/// and the third park is the first parked status, carrying the last cause.
#[test]
fn retryable_failure_stays_closing_through_two_renewals_then_parks_with_its_cause() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let WorkbenchAction::WorkspaceClose { workspace_id } = &close.action else {
        panic!()
    };
    let workspace_id = workspace_id.clone();
    accept_workspace(&mut authority, &grant, close.clone(), now);
    let spacing = Duration::from_millis(250);
    for attempt in 0..3_u32 {
        let started = now + Duration::from_secs(30) * attempt;
        let step = authority
            .next_terminal_tab_close_step_target(&close.request_id, started)
            .unwrap();
        assert!(matches!(
            step.next(),
            CloseContinuationNext::Lifecycle {
                action: TerminalLifecycleAction::Dispose,
                ..
            }
        ));
        assert_eq!(
            authority
                .terminal_tab_close_generation(&close.request_id)
                .unwrap()
                .value(),
            u64::from(attempt) + 1
        );
        settle_step(&mut authority, &close.request_id, step, started, None);
        assert!(matches!(
            workspace_status(&authority, &grant, &workspace_id),
            WorkbenchWorkspaceCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closing { .. },
                ..
            }
        ));
        assert_eq!(
            authority
                .next_terminal_tab_close_step(&close.request_id, started)
                .unwrap(),
            CloseContinuationNext::WaitUntil(started + spacing)
        );
        assert!(matches!(
            authority
                .next_terminal_tab_close_step(&close.request_id, started + spacing)
                .unwrap(),
            CloseContinuationNext::Lifecycle {
                action: TerminalLifecycleAction::Dispose,
                ..
            }
        ));
    }
    assert_eq!(
        authority
            .next_terminal_tab_close_step(&close.request_id, now + Duration::from_secs(90))
            .unwrap(),
        CloseContinuationNext::Parked
    );
    assert_eq!(
        authority
            .terminal_tab_close_generation(&close.request_id)
            .unwrap()
            .value(),
        3
    );
    assert!(matches!(
        workspace_status(&authority, &grant, &workspace_id),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::CleanupBlocked {
                reason:
                    orchbee_workbench_protocol::WorkbenchTabCloseBlockedReason::WorkerUnavailable,
                resumable: true,
            },
            ..
        }
    ));
}

/// A record with an unknown member is never renewed, even when another member parks resumably.
#[test]
fn a_close_with_an_unknown_member_is_never_renewed() {
    let (mut authority, grant) = workspace_fixture(2);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let WorkbenchAction::WorkspaceClose { workspace_id } = &close.action else {
        panic!()
    };
    let workspace_id = workspace_id.clone();
    accept_workspace(&mut authority, &grant, close.clone(), now);
    let retried = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    settle_step(&mut authority, &close.request_id, retried, now, None);
    let lost = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    settle_step(
        &mut authority,
        &close.request_id,
        lost,
        now,
        Some(TerminalLifecycleOutcome::OutcomeUnknown(
            orchbee_capability_node_protocol::LifecycleUnknownReason::TransportLost,
        )),
    );
    for expired in [30, 60] {
        assert_eq!(
            authority
                .next_terminal_tab_close_step(&close.request_id, now + Duration::from_secs(expired))
                .unwrap(),
            CloseContinuationNext::Parked
        );
    }
    assert_eq!(
        authority
            .terminal_tab_close_generation(&close.request_id)
            .unwrap()
            .value(),
        1
    );
    assert_eq!(
        authority
            .close_record(&close.request_id)
            .unwrap()
            .automatic_renewals,
        super::super::AUTOMATIC_RENEWALS
    );
    assert!(matches!(
        workspace_status(&authority, &grant, &workspace_id),
        WorkbenchWorkspaceCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::OutcomeUnknown { .. },
            ..
        }
    ));
}

#[test]
fn workspace_close_rejects_unknown_scope_and_denies_without_lifecycle_grant() {
    let (mut authority, grant) = workspace_fixture(1);
    let before = authority.projection.snapshot().clone();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    let reader =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone());
    assert!(matches!(
        authority.begin_workspace_close(Some(&reader), close.clone()),
        WorkbenchWorkspaceCloseBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Denied {},
            ..
        })
    ));
    let WorkbenchAction::WorkspaceClose { .. } = &close.action else {
        panic!()
    };
    let wrong_workspace = command(
        &authority,
        10001,
        WorkbenchAction::WorkspaceClose {
            workspace_id: WorkbenchWorkspaceId::parse(&identity(99999)).unwrap(),
        },
    );
    assert!(matches!(
        authority.begin_workspace_close(Some(&grant), wrong_workspace),
        WorkbenchWorkspaceCloseBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::WorkspaceUnavailable,
                ..
            },
            ..
        })
    ));
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(0));
    assert_eq!(authority.ledger.len(), 1); // one retained rejection
}

#[test]
fn workspace_resume_judgment_order_denies_mismatches_and_receipt_conflicts() {
    let (mut authority, grant) = workspace_fixture(0);
    let now = Instant::now();
    let close = workspace_close_command(&authority, 10000, 1, 0);
    accept_workspace(&mut authority, &grant, close.clone(), now);
    authority
        .close_record_mut(&close.request_id)
        .unwrap()
        .publication_failed(WorkbenchTabClosePublicationReason::PreparationFailed);

    let reader = WorkbenchGrant::read_only(authority.session_id().clone());
    let valid = workspace_resume_request(&close, 11000, 1);
    assert!(matches!(
        authority.begin_workspace_close_resume(Some(&reader), valid.clone()),
        WorkbenchWorkspaceCloseResumeBegin::Ready(
            orchbee_workbench_protocol::WorkbenchWorkspaceCloseResumeResult {
                outcome: WorkbenchWorkspaceCloseResumeOutcome::Denied {},
                ..
            }
        )
    ));
    let mut unknown = valid.clone();
    unknown.close_request_id = request_id(99999);
    assert!(matches!(
        workspace_resume(&mut authority, &grant, unknown, now).outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::OperationUnavailable
        }
    ));
    let mut mismatched = valid.clone();
    mismatched.workspace_id = WorkbenchWorkspaceId::parse(&identity(99996)).unwrap();
    assert!(matches!(
        workspace_resume(&mut authority, &grant, mismatched, now).outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::OperationMismatch
        }
    ));
    let mut ahead = valid.clone();
    ahead.expected_attempt_generation = WorkbenchTabCloseAttemptGeneration::new(2).unwrap();
    assert!(matches!(
        workspace_resume(&mut authority, &grant, ahead, now).outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::GenerationMismatch
        }
    ));
    let accepted = workspace_resume(&mut authority, &grant, valid.clone(), now);
    assert!(matches!(
        accepted.outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
    ));
    let mut conflict = valid.clone();
    conflict.expected_attempt_generation = WorkbenchTabCloseAttemptGeneration::new(2).unwrap();
    assert!(matches!(
        workspace_resume(&mut authority, &grant, conflict, now).outcome,
        WorkbenchWorkspaceCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::RequestIdentityConflict
        }
    ));
}
