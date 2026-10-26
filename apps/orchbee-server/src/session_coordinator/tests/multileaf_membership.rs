//! Multi-leaf membership witnesses for the accepted tree-view contract: a Tab
//! legally owns an ordered leaf sequence, so `tab.create`/`tab.move` admission
//! and initial_workspace Create completion consume `tree_*` membership while keeping
//! the reference-derived unique-binding composition unchanged.

use super::panel_split::{close_leaf, dispatch_split, split_command, workspace_leaf};
use super::tab_create::{
    configure_workspace, context_tab_command, dispatch_tab, immediate_completion, ready,
};
use super::*;
use orchbee_capability_node_protocol::{
    LifecycleInventoryPage, LifecycleTargetObservation, LifecycleTerminalPhase,
    TerminalLifecycleAction, TerminalLifecycleOutcome, TerminalLifecycleResult,
};
use orchbee_server::{CloseContinuationNext, CloseStepObservation};
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchPanelSplitDirection, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTabId, WorkbenchTabMoveDirection,
    WorkbenchTerminalContextId, WorkbenchTerminalOutcome,
};
use std::num::NonZeroU64;
use std::time::Instant;

fn move_command(
    coordinator: &mut SessionCoordinator,
    request: u128,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    direction: WorkbenchTabMoveDirection,
) -> WorkbenchCommand {
    let committed = snapshot(coordinator);
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        session_id: committed.session_id,
        expected_revision: committed.revision,
        action: WorkbenchAction::TabMove {
            terminal_context_id,
            tab_id,
            direction,
        },
    }
}

fn snapshot_outcome(coordinator: &mut SessionCoordinator) -> WorkbenchSnapshotOutcome {
    coordinator
        .execute_workbench(
            WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 888)).unwrap(),
            })
            .try_into()
            .unwrap(),
            false,
        )
        .map(|response| match response {
            WorkbenchResponse::Snapshot(result) => result.outcome,
            _ => panic!("snapshot response expected"),
        })
        .unwrap()
}

/// Settles a initial_workspace first tab, then commits a real split so the context
/// holds one Tab with two bound leaf members ([A, S]).
fn settle_first_then_split(coordinator: &mut SessionCoordinator, base: u128) {
    let initial = snapshot(coordinator);
    let create_a = context_tab_command(
        coordinator,
        base,
        initial.revision.value(),
        initial.navigation.workspaces[0].terminal_context_id.clone(),
        "first",
    );
    let pending_a = dispatch_tab(coordinator, create_a, base + 1);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = workspace_leaf(coordinator);
    let split = split_command(
        coordinator,
        base + 2,
        context,
        tab,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_s = dispatch_split(coordinator, split, base + 3);
    let completion_s = immediate_completion(pending_s.command());
    coordinator
        .complete_terminal_panel_create(pending_s, completion_s, Some(worker(1)))
        .unwrap();
}

/// Drives the real aggregate Close finalizer through publication: lifecycle
/// steps use fabricated terminal replies, which is the labeled synthetic seam;
/// admission, per-target cleanup ordering, publication and retargeting are the
/// production path.
pub(super) fn drive_close_to_publication(
    coordinator: &mut SessionCoordinator,
    close: &WorkbenchCommand,
) {
    let mut now = Instant::now();
    for _step in 0..64 {
        let step = coordinator
            .close_authority()
            .unwrap()
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap();
        match step.next() {
            CloseContinuationNext::WaitUntil(until) => now = until,
            CloseContinuationNext::Lifecycle {
                action,
                step_ordinal,
            } => {
                let target = step.target().expect("lifecycle step target").clone();
                let Some(token) = coordinator
                    .reserve_close_lifecycle_step_for_target(
                        &close.request_id,
                        target,
                        action,
                        step_ordinal,
                        Instant::now(),
                        expiry(),
                    )
                    .unwrap()
                else {
                    panic!("announced lifecycle step must reserve")
                };
                let now = Instant::now();
                coordinator
                    .close_authority()
                    .unwrap()
                    .confirm_terminal_tab_close_step_admitted(&close.request_id, &token, now)
                    .unwrap();
                let outcome = match token.request().action {
                    TerminalLifecycleAction::InventoryPage { .. } => {
                        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
                            inventory_generation: NonZeroU64::MIN,
                            next_cursor: None,
                            returned_count: 1,
                            target: Some(LifecycleTargetObservation {
                                close_activity: orchbee_capability_node_protocol::LifecycleCloseActivity::Unknown,
                                revision: 3,
                                phase: LifecycleTerminalPhase::Exited {
                                    terminate_requested: false,
                                },
                            }),
                        })
                    }
                    TerminalLifecycleAction::Terminate => {
                        TerminalLifecycleOutcome::TerminateAccepted
                    }
                    TerminalLifecycleAction::Force => TerminalLifecycleOutcome::ForceAccepted,
                    TerminalLifecycleAction::Dispose => TerminalLifecycleOutcome::Disposed,
                    TerminalLifecycleAction::TerminateIfIdle
                    | TerminalLifecycleAction::InspectInventoryPage { .. } => {
                        panic!("guarded Close requires the dedicated idle-evidence fixture")
                    }
                };
                let result = TerminalLifecycleResult::new(token.request(), outcome).unwrap();
                coordinator
                    .close_authority()
                    .unwrap()
                    .settle_terminal_tab_close_step(
                        &close.request_id,
                        token,
                        now,
                        CloseStepObservation::Result(&result),
                    )
                    .unwrap();
            }
            CloseContinuationNext::TargetCleanup => {
                let runtime = step
                    .target()
                    .expect("cleanup step target")
                    .runtime()
                    .expect("created Host runtime");
                coordinator
                    .close_authority()
                    .unwrap()
                    .complete_terminal_tab_close_target_cleanup(&close.request_id, runtime)
                    .unwrap();
            }
            CloseContinuationNext::Publication => {
                return coordinator
                    .publish_terminal_tab_close(&close.request_id, Instant::now())
                    .unwrap();
            }
            next => panic!("unexpected close step {next:?}"),
        }
    }
    panic!("close publication never became ready")
}

fn tab_close_command(
    coordinator: &mut SessionCoordinator,
    request: u128,
    context: WorkbenchTerminalContextId,
    tab: WorkbenchTabId,
) -> WorkbenchCommand {
    let committed = snapshot(coordinator);
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        session_id: committed.session_id,
        expected_revision: committed.revision,
        action: WorkbenchAction::TabClose {
            terminal_context_id: context,
            tab_id: tab,
        },
    }
}

fn terminal_started(result: &WorkbenchCommandResult) {
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
}

#[test]
fn multileaf_initial_workspace_tab_create_admits_and_settles() {
    let mut coordinator = session();
    settle_first_then_split(&mut coordinator, 9000);
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    assert_eq!(
        committed
            .terminal_contexts
            .iter()
            .find(|item| item.terminal_context_id == context)
            .unwrap()
            .tabs
            .len(),
        1
    );
    let create_c = context_tab_command(
        &coordinator,
        9010,
        committed.revision.value(),
        context.clone(),
        "third",
    );
    let pending_c = dispatch_tab(&mut coordinator, create_c.clone(), 9011);

    let completion_c = immediate_completion(pending_c.command());
    let completed = coordinator
        .complete_terminal_tab_create(pending_c, completion_c, Some(worker(1)))
        .unwrap();

    terminal_started(&completed.result);
    let committed = snapshot(&mut coordinator);
    let context = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap();
    assert_eq!(context.tabs.len(), 2);
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(create_c, None, None, None)),
        completed.result
    );
}

#[test]
fn multileaf_initial_workspace_tab_move_reorders_without_retiring() {
    let mut coordinator = session();
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create_a = context_tab_command(&coordinator, 9100, 0, context.clone(), "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 9101);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let create_b = context_tab_command(&coordinator, 9102, 1, context.clone(), "second");
    let pending_b = dispatch_tab(&mut coordinator, create_b, 9103);
    let completion_b = immediate_completion(pending_b.command());
    coordinator
        .complete_terminal_tab_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    let (ctx, tab_a, panel_a) = workspace_leaf(&mut coordinator);
    let split = split_command(
        &mut coordinator,
        9104,
        ctx,
        tab_a,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_s = dispatch_split(&mut coordinator, split, 9105);
    let completion_s = immediate_completion(pending_s.command());
    coordinator
        .complete_terminal_panel_create(pending_s, completion_s, Some(worker(1)))
        .unwrap();
    let committed = snapshot(&mut coordinator);
    let tab_b = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap()
        .tabs[1]
        .tab_id
        .clone();
    let request = move_command(
        &mut coordinator,
        9110,
        context.clone(),
        tab_b.clone(),
        WorkbenchTabMoveDirection::First,
    );
    let result = coordinator.execute_terminal_tab_move(request).unwrap();
    assert_eq!(
        result.outcome,
        WorkbenchCommandOutcome::TabMoved {
            revision: WorkbenchRevision::new(4),
            terminal_context_id: context.clone(),
            tab_id: tab_b.clone(),
        }
    );
    assert!(matches!(
        snapshot_outcome(&mut coordinator),
        WorkbenchSnapshotOutcome::Available { .. }
    ));
    assert_eq!(
        snapshot(&mut coordinator)
            .terminal_contexts
            .iter()
            .find(|item| item.terminal_context_id == context)
            .unwrap()
            .tabs[0]
            .tab_id,
        tab_b
    );
}

#[test]
fn multileaf_workspace_tab_move_reorders_without_retiring() {
    let mut coordinator = session();
    let context = configure_workspace(&mut coordinator, "/tmp/workspace");
    let create_a = context_tab_command(&coordinator, 9200, 3, context.clone(), "workspace A");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 9201);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let create_b = context_tab_command(&coordinator, 9202, 4, context.clone(), "workspace B");
    let pending_b = dispatch_tab(&mut coordinator, create_b, 9203);
    let completion_b = immediate_completion(pending_b.command());
    coordinator
        .complete_terminal_tab_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    let committed = snapshot(&mut coordinator);
    let workspace_tab = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap()
        .tabs[0]
        .clone();
    let PanelNode::Panel { panel_id, .. } = &workspace_tab.root else {
        panic!("simple panel expected")
    };
    let split = split_command(
        &mut coordinator,
        9204,
        context.clone(),
        workspace_tab.tab_id.clone(),
        panel_id.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_s = dispatch_split(&mut coordinator, split, 9205);
    let completion_s = immediate_completion(pending_s.command());
    coordinator
        .complete_terminal_panel_create(pending_s, completion_s, Some(worker(1)))
        .unwrap();
    let committed = snapshot(&mut coordinator);
    let tab_b = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap()
        .tabs[1]
        .tab_id
        .clone();
    let request = move_command(
        &mut coordinator,
        9210,
        context.clone(),
        tab_b.clone(),
        WorkbenchTabMoveDirection::First,
    );
    let result = coordinator.execute_terminal_tab_move(request).unwrap();
    assert_eq!(
        result.outcome,
        WorkbenchCommandOutcome::TabMoved {
            revision: WorkbenchRevision::new(committed.revision.value() + 1),
            terminal_context_id: context.clone(),
            tab_id: tab_b.clone(),
        }
    );

    assert_eq!(
        snapshot(&mut coordinator)
            .terminal_contexts
            .iter()
            .find(|item| item.terminal_context_id == context)
            .unwrap()
            .tabs[0]
            .tab_id,
        tab_b
    );
}

#[test]
fn create_dispatched_then_split_committed_then_completion_settles() {
    let mut coordinator = session();
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create_a = context_tab_command(&coordinator, 9400, 0, context.clone(), "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 9401);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    // Dispatch a second Create while the context is still single-leaf, then let a
    // real split commit before the Create completes: the context becomes
    // multi-leaf while the Create's own member is already projected.
    let create_c = context_tab_command(&coordinator, 9402, 1, context.clone(), "second");
    let pending_c = dispatch_tab(&mut coordinator, create_c, 9403);
    let (ctx, tab, panel_a) = workspace_leaf(&mut coordinator);
    let split = split_command(
        &mut coordinator,
        9404,
        ctx,
        tab,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_b = dispatch_split(&mut coordinator, split, 9405);
    let completion_b = immediate_completion(pending_b.command());
    coordinator
        .complete_terminal_panel_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    let completion_c = immediate_completion(pending_c.command());
    let completed = coordinator
        .complete_terminal_tab_create(pending_c, completion_c, Some(worker(1)))
        .unwrap();

    terminal_started(&completed.result);
    let committed = snapshot(&mut coordinator);
    let context = committed
        .terminal_contexts
        .iter()
        .find(|item| {
            item.terminal_context_id == committed.navigation.workspaces[0].terminal_context_id
        })
        .unwrap();
    assert_eq!(context.tabs.len(), 2);
}

#[test]
fn pending_create_completion_survives_another_tab_close() {
    // Closing another settled Tab must not invalidate this pending Create.
    let mut coordinator = session();
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create_a = context_tab_command(&coordinator, 9500, 0, context.clone(), "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 9501);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let create_c = context_tab_command(&coordinator, 9502, 1, context.clone(), "second");
    let pending_c = dispatch_tab(&mut coordinator, create_c, 9503);
    let (ctx, tab_a, panel_a) = workspace_leaf(&mut coordinator);
    let _close = close_leaf(&mut coordinator, ctx, tab_a, panel_a, 9504);

    let completion_c = immediate_completion(pending_c.command());
    let completed = coordinator
        .complete_terminal_tab_create(pending_c, completion_c, Some(worker(1)))
        .unwrap();
    terminal_started(&completed.result);
}

#[test]
fn pending_create_and_split_complete_independently() {
    // A split can commit while its Tab creation is pending; both completions
    // must retain their own successful results.
    let mut coordinator = session();
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create_a = context_tab_command(&coordinator, 9600, 0, context.clone(), "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 9601);

    let (ctx, tab_a, panel_a) = workspace_leaf(&mut coordinator);
    let split = split_command(
        &mut coordinator,
        9602,
        ctx,
        tab_a,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_s = dispatch_split(&mut coordinator, split, 9603);

    let completion_a = immediate_completion(pending_a.command());
    let completed = coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();

    terminal_started(&completed.result);
    // The pending split still completes independently afterwards.
    let completion_s = immediate_completion(pending_s.command());
    let completed_s = coordinator
        .complete_terminal_panel_create(pending_s, completion_s, Some(worker(1)))
        .unwrap();
    assert!(matches!(
        completed_s.result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
}

#[test]
fn pending_creates_settle_out_of_order_after_another_tab_closes() {
    // Closing a settled Tab preserves two other pending Creates. They may
    // complete out of order, and the retained result must still replay.
    let mut coordinator = session();
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create_a = context_tab_command(&coordinator, 9700, 0, context.clone(), "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 9701);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let create_b = context_tab_command(&coordinator, 9702, 1, context.clone(), "second");
    let pending_b = dispatch_tab(&mut coordinator, create_b, 9703);
    let create_c = context_tab_command(&coordinator, 9704, 2, context.clone(), "third");
    let pending_c = dispatch_tab(&mut coordinator, create_c.clone(), 9705);
    let (_ctx, tab_a, _panel_a) = workspace_leaf(&mut coordinator);
    let close = tab_close_command(&mut coordinator, 9706, context.clone(), tab_a);
    let accepted = coordinator
        .execute_terminal_tab_close(close.clone(), Some(worker(1)), true, Instant::now())
        .unwrap();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    drive_close_to_publication(&mut coordinator, &close);

    // Complete the later Create first.
    let completion_c = immediate_completion(pending_c.command());
    let completed_c = coordinator
        .complete_terminal_tab_create(pending_c, completion_c, Some(worker(1)))
        .unwrap();

    terminal_started(&completed_c.result);
    // The earlier Create still completes independently.
    let completion_b = immediate_completion(pending_b.command());
    let completed_b = coordinator
        .complete_terminal_tab_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    terminal_started(&completed_b.result);

    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(create_c, None, None, None)),
        completed_c.result
    );
}

#[test]
fn a_pending_member_cannot_be_removed_by_close() {
    // Close admission resolves a runtime for every frozen member; a pending
    // Create sits in TerminalPhase::Creating and resolves Busy, so the close is
    // refused before publication. Therefore no completion can ever arrive for
    // a member that a close already removed: the retarget-away-of-completing-
    // member and close-to-Empty-with-pending-completion traces are impossible
    // by construction, and this refusal is their boundary witness.
    let mut coordinator = session();
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create_a = context_tab_command(&coordinator, 9750, 0, context.clone(), "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 9751);
    let (ctx, tab_a, panel_a) = workspace_leaf(&mut coordinator);
    let committed = snapshot(&mut coordinator);
    for action in [
        WorkbenchAction::PanelClose {
            terminal_context_id: ctx.clone(),
            tab_id: tab_a.clone(),
            panel_id: panel_a.clone(),
        },
        WorkbenchAction::TabClose {
            terminal_context_id: ctx.clone(),
            tab_id: tab_a.clone(),
        },
    ] {
        let close = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 9752)).unwrap(),
            session_id: committed.session_id.clone(),
            expected_revision: committed.revision,
            action,
        };
        let result = coordinator
            .execute_terminal_tab_close(close, Some(worker(1)), true, Instant::now())
            .unwrap();
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            }
        ));
    }
    // The member and pending operation remain intact, so completion succeeds.
    let (still_ctx, still_tab, _) = workspace_leaf(&mut coordinator);
    assert_eq!(still_ctx, ctx);
    assert_eq!(still_tab, tab_a);
    let completion_a = immediate_completion(pending_a.command());
    let completed_a = coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    terminal_started(&completed_a.result);
}

#[test]
fn last_close_to_empty_keeps_retained_replay_and_fresh_create_path() {
    // Closing the last Tab leaves its Workspace empty. The retained result
    // still replays without resurrecting that Tab, and a fresh Create succeeds.
    let mut coordinator = session();
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create_a = context_tab_command(&coordinator, 9800, 0, context.clone(), "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a.clone(), 9801);
    let completion_a = immediate_completion(pending_a.command());
    let completed_a = coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let (ctx, tab_a, panel_a) = workspace_leaf(&mut coordinator);
    let _close = close_leaf(&mut coordinator, ctx, tab_a, panel_a, 9802);

    let committed = snapshot(&mut coordinator);
    let context_id = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context_id)
        .unwrap();
    assert!(context.tabs.is_empty());
    // A's settled result replays exactly even though its member is gone.
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(create_a, None, None, None)),
        completed_a.result
    );
    // A fresh Create in the empty Workspace commits and settles normally.
    let create_b = context_tab_command(
        &coordinator,
        9803,
        committed.revision.value(),
        context_id,
        "second",
    );
    let pending_b = dispatch_tab(&mut coordinator, create_b, 9804);
    let completion_b = immediate_completion(pending_b.command());
    let completed_b = coordinator
        .complete_terminal_tab_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    terminal_started(&completed_b.result);
}

#[test]
fn an_open_aggregate_close_fences_the_same_tab_split_but_not_a_new_create() {
    let mut coordinator = session();
    settle_first_then_split(&mut coordinator, 9980);
    let committed = snapshot(&mut coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let tab = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap()
        .tabs[0]
        .clone();
    let close = tab_close_command(&mut coordinator, 9984, context.clone(), tab.tab_id.clone());
    let accepted = coordinator
        .execute_terminal_tab_close(close.clone(), Some(worker(1)), true, Instant::now())
        .unwrap();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    // While the aggregate Close is pending, a same-Tab panel.split is refused
    // with the transient busy result...
    let PanelNode::Split { children, .. } = &tab.root else {
        panic!("split root expected")
    };
    let PanelNode::Panel { panel_id, .. } = &children[0].node else {
        panic!("panel leaf expected")
    };
    let split = split_command(
        &mut coordinator,
        9985,
        context.clone(),
        tab.tab_id.clone(),
        panel_id.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let PreparedTerminalPanelCreate::Ready(refused) = coordinator.prepare_terminal_panel_create(
        split,
        Some(id(9986)),
        Some(expiry()),
        Some(worker(1)),
    ) else {
        panic!("fenced split must not dispatch")
    };
    assert!(matches!(
        refused.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    // ...while an unrelated whole-Tab create still commits on the fresh
    // revision: the fence is exact, not a broad serialization.
    let create = context_tab_command(
        &coordinator,
        9987,
        committed.revision.value(),
        context.clone(),
        "unfenced",
    );
    let pending = dispatch_tab(&mut coordinator, create, 9988);
    let completion = immediate_completion(pending.command());
    let completed = coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();

    terminal_started(&completed.result);
    // Publishing the Close removes the split Tab and retains the newly created Tab.
    drive_close_to_publication(&mut coordinator, &close);

    let committed = snapshot(&mut coordinator);
    let context = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap();
    assert_eq!(context.tabs.len(), 1);
    assert_eq!(context.tabs[0].label, "unfenced");
}
