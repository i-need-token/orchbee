use super::*;
use orchbee_capability_node_protocol::{
    LifecycleStepOrdinal, LifecycleUnknownReason, TerminalLifecycleAction,
    TerminalLifecycleAuthorization,
};
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, PanelSlot, Presentation, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchPanelSplitDirection, WorkbenchSplitId,
};

use crate::workbench_authority::{
    WorkbenchPanelMoveBegin, WorkbenchPanelMovePreparation, WorkbenchPanelSplitBegin,
    WorkbenchPanelSplitPreparation,
};

fn split_authority() -> (WorkbenchAuthority, WorkbenchGrant) {
    split_fixture(1)
}

fn split_fixture(count: u128) -> (WorkbenchAuthority, WorkbenchGrant) {
    let (mut authority, grant) = fixture(count);
    let mut snapshot = authority.projection.snapshot().clone();
    let tab = &mut snapshot.terminal_contexts[0].tabs[0];
    let left = tab.root.clone();
    tab.root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: left,
                slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&identity(102)).unwrap(),
                    primary_presentation: Presentation {
                        binding: Some(binding(700 + count)),
                        label: "right".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(103)).unwrap(),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&identity(104)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&identity(100)).unwrap(),
    };
    authority.projection = WorkbenchProjectionStore::load(
        &orchbee_workbench_protocol::serialize_document(
            &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot),
        )
        .unwrap(),
    )
    .unwrap();
    (authority, grant)
}

fn accept_close_members(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    close: WorkbenchCommand,
    now: Instant,
) -> WorkbenchCommandResult {
    let primary = primary(authority, grant, close);
    let runtimes = primary
        .members()
        .iter()
        .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(runtimes, now)
    else {
        panic!("aggregate Close preparation")
    };
    prepared.commit()
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
                let runtime = step.target().unwrap().runtime();
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

fn step_request(
    close_id: &WorkbenchRequestId,
    target: &WorkbenchTabCloseTarget,
    action: TerminalLifecycleAction,
    step_ordinal: LifecycleStepOrdinal,
    generation: WorkbenchTabCloseAttemptGeneration,
) -> TerminalLifecycleRequest {
    let runtime = target.runtime();
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
    let result = TerminalLifecycleResult::new(request, TerminalLifecycleOutcome::Disposed).unwrap();
    authority
        .settle_terminal_tab_close_step(close_id, token, now, CloseStepObservation::Result(&result))
        .unwrap();
}

fn settle_unknown(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    step: WorkbenchTabCloseStepTarget,
    now: Instant,
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
    authority
        .settle_terminal_tab_close_step(
            close_id,
            token,
            now,
            CloseStepObservation::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
        )
        .unwrap();
}

#[test]
fn split_tab_close_routes_each_frozen_leaf_then_publishes_one_tab_removal() {
    let (mut authority, grant) = split_authority();
    let context_id = authority
        .projection
        .snapshot()
        .navigation
        .standalone_terminal_context_id
        .clone();
    let tab_id = authority.projection.snapshot().terminal_contexts[0].tabs[0]
        .tab_id
        .clone();
    let close = command(
        &authority,
        10000,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id.clone(),
            tab_id: tab_id.clone(),
        },
    );
    let now = Instant::now();
    let primary = primary(&mut authority, &grant, close.clone());
    assert_eq!(primary.members().len(), 2);
    let runtimes = primary
        .members()
        .iter()
        .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(runtimes, now)
    else {
        panic!("split Close preparation");
    };
    let _accepted = prepared.commit();

    let first = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    let first_binding = first.target().unwrap().member().binding().clone();
    settle_disposed(&mut authority, &close.request_id, first, now);
    let second = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    assert_ne!(second.target().unwrap().member().binding(), &first_binding);
    settle_disposed(&mut authority, &close.request_id, second, now);

    for _ in 0..2 {
        let step = authority
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap();
        let runtime = step.target().unwrap().runtime();
        assert_eq!(step.next(), CloseContinuationNext::TargetCleanup);
        authority
            .complete_terminal_tab_close_target_cleanup(&close.request_id, runtime)
            .unwrap();
    }
    assert_eq!(
        authority
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap()
            .next(),
        CloseContinuationNext::Publication
    );
    let closed = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap()
        .commit();
    assert!(matches!(closed, WorkbenchTabCloseStatus::Closed { .. }));
    assert!(
        authority.projection.snapshot().terminal_contexts[0]
            .tabs
            .is_empty()
    );
    assert_eq!(
        authority
            .terminal_tab_close_target_for_binding(&close.request_id, &first_binding)
            .unwrap()
            .member()
            .tab_id(),
        &tab_id
    );
}

#[test]
fn split_tab_close_quarantines_siblings_after_unknown_but_keeps_known_cleanup() {
    let (mut authority, grant) = split_authority();
    let context_id = authority
        .projection
        .snapshot()
        .navigation
        .standalone_terminal_context_id
        .clone();
    let tab_id = authority.projection.snapshot().terminal_contexts[0].tabs[0]
        .tab_id
        .clone();
    let close = command(
        &authority,
        10001,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id,
            tab_id,
        },
    );
    let now = Instant::now();
    let primary = primary(&mut authority, &grant, close.clone());
    let runtimes = primary
        .members()
        .iter()
        .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(runtimes, now)
    else {
        panic!("split Close preparation");
    };
    let _ = prepared.commit();

    let first = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    let first_runtime = first.target().unwrap().runtime();
    let first_binding = first.target().unwrap().member().binding().clone();
    settle_unknown(&mut authority, &close.request_id, first, now);
    assert!(matches!(
        authority.terminal_tab_close_status(&close.request_id),
        Some(WorkbenchTabCloseStatus::OutcomeUnknown { .. })
    ));
    assert_eq!(
        authority
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap()
            .next(),
        CloseContinuationNext::Parked
    );

    let second_binding = authority
        .terminal_tree_membership(
            &authority
                .terminal_tab_close_target(&close.request_id)
                .unwrap()
                .terminal_context_id()
                .clone(),
        )
        .unwrap()
        .members()
        .iter()
        .find(|member| member.binding() != &first_binding)
        .unwrap()
        .binding()
        .clone();
    let second = authority
        .terminal_tab_close_target_for_binding(&close.request_id, &second_binding)
        .unwrap();
    assert_ne!(second.runtime(), first_runtime);
    assert!(matches!(
        authority
            .terminal_tab_close_target_status(&close.request_id, second.runtime())
            .unwrap(),
        WorkbenchTabCloseStatus::Closing { .. }
    ));
}

#[test]
fn aggregate_close_fences_same_tab_split_and_move_until_publication() {
    let (mut authority, grant) = split_fixture(2);
    let context_id = authority
        .projection
        .snapshot()
        .navigation
        .standalone_terminal_context_id
        .clone();
    let closing_tab_id = authority.projection.snapshot().terminal_contexts[0].tabs[0]
        .tab_id
        .clone();
    let other_tab_id = authority.projection.snapshot().terminal_contexts[0].tabs[1]
        .tab_id
        .clone();
    let membership = authority.terminal_tree_membership(&context_id).unwrap();
    let closing_members: Vec<_> = membership
        .members_for_tab(&closing_tab_id)
        .cloned()
        .collect();
    let other_members: Vec<_> = membership.members_for_tab(&other_tab_id).cloned().collect();
    assert_eq!(closing_members.len(), 2);
    assert_eq!(other_members.len(), 1);
    let now = Instant::now();
    let close = command(
        &authority,
        10000,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id.clone(),
            tab_id: closing_tab_id.clone(),
        },
    );
    let accepted = accept_close_members(&mut authority, &grant, close.clone(), now);
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    let retained = authority.ledger.len();

    // The pending aggregate Close fences same-Tab structural admission; the refusal is
    // transient — no ledger entry, no admission, no terminal launch.
    let split = command(
        &authority,
        10001,
        WorkbenchAction::PanelSplit {
            terminal_context_id: context_id.clone(),
            tab_id: closing_tab_id.clone(),
            panel_id: closing_members[0].panel_id().clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("fenced split").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
        },
    );
    let WorkbenchPanelSplitBegin::Ready(fenced) =
        authority.begin_terminal_panel_split(Some(&grant), split.clone())
    else {
        panic!("pending aggregate Close must fence the split before admission")
    };
    assert!(matches!(
        fenced.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(authority.ledger.len(), retained);
    let WorkbenchPanelSplitBegin::Ready(retried) =
        authority.begin_terminal_panel_split(Some(&grant), split.clone())
    else {
        panic!("unretained refusal must re-evaluate")
    };
    assert_eq!(retried, fenced);
    assert_eq!(authority.ledger.len(), retained);

    let movement = command(
        &authority,
        10002,
        WorkbenchAction::PanelMove {
            terminal_context_id: context_id.clone(),
            tab_id: closing_tab_id.clone(),
            panel_id: closing_members[0].panel_id().clone(),
            target_panel_id: closing_members[1].panel_id().clone(),
            direction: WorkbenchPanelSplitDirection::Right,
        },
    );
    let WorkbenchPanelMoveBegin::Admitted(moving) =
        authority.begin_terminal_panel_move(Some(&grant), movement)
    else {
        panic!("move admission")
    };
    let WorkbenchPanelMovePreparation::Ready(fenced_move) = moving.finish() else {
        panic!("pending aggregate Close must fence the move before commit")
    };
    assert!(matches!(
        fenced_move.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(authority.ledger.len(), retained);

    // Ledger replay precedes the fence.
    let WorkbenchTabCloseBegin::Ready(replayed) =
        authority.begin_terminal_tab_close(Some(&grant), close.clone())
    else {
        panic!("Close replay")
    };
    assert_eq!(replayed, accepted);

    // An unrelated Tab still admits structural work.
    let other_split = command(
        &authority,
        10003,
        WorkbenchAction::PanelSplit {
            terminal_context_id: context_id.clone(),
            tab_id: other_tab_id.clone(),
            panel_id: other_members[0].panel_id().clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("other split").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
        },
    );
    let WorkbenchPanelSplitBegin::Admitted(other) =
        authority.begin_terminal_panel_split(Some(&grant), other_split.clone())
    else {
        panic!("unrelated Tab split must remain admissible")
    };
    let WorkbenchPanelSplitPreparation::Prepared(other_prepared) = other.finish(binding(1703))
    else {
        panic!("unrelated Tab split preparation")
    };
    let _pending = other_prepared.commit_into_pending();
    let WorkbenchPanelSplitBegin::Ready(replayed_split) =
        authority.begin_terminal_panel_split(Some(&grant), other_split)
    else {
        panic!("pending split replay")
    };
    assert!(matches!(
        replayed_split.outcome,
        WorkbenchCommandOutcome::PanelSplitPending { .. }
    ));

    // The fenced Close still publishes exactly and the authority remains usable.
    let closed = drive_close_to_publication(&mut authority, &close.request_id, now);
    assert!(matches!(closed, WorkbenchTabCloseStatus::Closed { .. }));
    assert!(!authority.retired);
    let tabs = &authority.projection.snapshot().terminal_contexts[0].tabs;
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].tab_id, other_tab_id);
    let WorkbenchPanelSplitBegin::Ready(after) =
        authority.begin_terminal_panel_split(Some(&grant), split)
    else {
        panic!("post-publication re-evaluation")
    };
    assert!(matches!(
        after.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
}

#[test]
fn leaf_close_leaves_same_tab_split_and_move_admissible() {
    let (mut authority, grant) = split_authority();
    let context_id = authority
        .projection
        .snapshot()
        .navigation
        .standalone_terminal_context_id
        .clone();
    let tab_id = authority.projection.snapshot().terminal_contexts[0].tabs[0]
        .tab_id
        .clone();
    let membership = authority.terminal_tree_membership(&context_id).unwrap();
    let members: Vec<_> = membership.members_for_tab(&tab_id).cloned().collect();
    assert_eq!(members.len(), 2);
    let now = Instant::now();
    let close = command(
        &authority,
        10010,
        WorkbenchAction::PanelClose {
            terminal_context_id: context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: members[0].panel_id().clone(),
        },
    );
    let accepted = accept(&mut authority, &grant, close.clone(), now);
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::PanelCloseAccepted { .. }
    ));

    // A leaf-scoped Close re-proves only its captured leaf tuple: the same-Tab move is
    // still admitted and commits.
    let movement = command(
        &authority,
        10011,
        WorkbenchAction::PanelMove {
            terminal_context_id: context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: members[1].panel_id().clone(),
            target_panel_id: members[0].panel_id().clone(),
            direction: WorkbenchPanelSplitDirection::Left,
        },
    );
    let WorkbenchPanelMoveBegin::Admitted(moving) =
        authority.begin_terminal_panel_move(Some(&grant), movement)
    else {
        panic!("leaf Close must not fence the same-Tab move")
    };
    let WorkbenchPanelMovePreparation::Prepared(prepared_move) = moving.finish() else {
        panic!("leaf Close must not fence the same-Tab move")
    };
    let moved = prepared_move.commit();
    assert!(matches!(
        moved.outcome,
        WorkbenchCommandOutcome::PanelMoved { .. }
    ));

    // A same-Tab split on the surviving leaf is still admitted.
    let split = command(
        &authority,
        10012,
        WorkbenchAction::PanelSplit {
            terminal_context_id: context_id,
            tab_id: tab_id.clone(),
            panel_id: members[1].panel_id().clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("leaf split").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
        },
    );
    let WorkbenchPanelSplitBegin::Admitted(splitting) =
        authority.begin_terminal_panel_split(Some(&grant), split)
    else {
        panic!("leaf Close must not fence the same-Tab split")
    };
    let WorkbenchPanelSplitPreparation::Prepared(prepared_split) = splitting.finish(binding(1702))
    else {
        panic!("same-Tab split preparation")
    };
    let _pending = prepared_split.commit_into_pending();

    // The leaf Close still publishes its contraction over the changed membership.
    let closed = drive_close_to_publication(&mut authority, &close.request_id, now);
    assert!(matches!(closed, WorkbenchTabCloseStatus::Closed { .. }));
    assert!(!authority.retired);
    assert_eq!(
        authority.projection.snapshot().terminal_contexts[0].tabs[0].tab_id,
        tab_id
    );
}
