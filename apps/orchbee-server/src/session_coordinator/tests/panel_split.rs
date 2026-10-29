use super::tab_create::{
    configure_workspace, context_tab_command, dispatch_tab, immediate_completion,
    pending_first_tab, tab_command,
};
use super::*;
use orchbee_capability_node_protocol::{
    LifecycleInventoryPage, LifecycleTargetObservation, LifecycleTerminalPhase,
    TerminalLifecycleAction, TerminalLifecycleOutcome, TerminalLifecycleResult,
};
use orchbee_server::CloseStepObservation;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchLabel, WorkbenchPanelId, WorkbenchPanelSplitDirection, WorkbenchRejectionReason,
    WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome,
};
use std::time::Instant;

pub(super) fn split_command(
    coordinator: &mut SessionCoordinator,
    request: u128,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
) -> WorkbenchCommand {
    let committed = snapshot(coordinator);
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        session_id: committed.session_id,
        expected_revision: committed.revision,
        action: WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id,
            tab_id,
            panel_id,
            direction,
            label: WorkbenchLabel::parse("split").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    }
}

pub(super) fn dispatch_split(
    coordinator: &mut SessionCoordinator,
    command: WorkbenchCommand,
    reference: u128,
) -> PendingTerminalPanelCreate {
    let PreparedTerminalPanelCreate::Dispatch(pending) = coordinator.prepare_terminal_panel_create(
        command,
        Some(id(reference)),
        Some(expiry()),
        Some(worker(1)),
        &LaunchCapture::absent(),
    ) else {
        panic!("panel.split dispatch expected")
    };
    pending
}

fn ready_split(prepared: PreparedTerminalPanelCreate) -> WorkbenchCommandResult {
    let PreparedTerminalPanelCreate::Ready(result) = prepared else {
        panic!("ready panel.split result expected")
    };
    result
}

pub(super) fn workspace_leaf(
    coordinator: &mut SessionCoordinator,
) -> (WorkbenchTerminalContextId, WorkbenchTabId, WorkbenchPanelId) {
    let committed = snapshot(coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let tab = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap()
        .tabs
        .first()
        .unwrap();
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!("simple panel expected")
    };
    (context, tab.tab_id.clone(), panel_id.clone())
}

// These fabricated replies prove Session refinement of the real Close finalizer, not Worker behavior.
pub(super) fn close_leaf(
    coordinator: &mut SessionCoordinator,
    context: WorkbenchTerminalContextId,
    tab: WorkbenchTabId,
    panel: WorkbenchPanelId,
    request: u128,
) -> WorkbenchCommand {
    let committed = snapshot(coordinator);
    let close = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        session_id: committed.session_id,
        expected_revision: committed.revision,
        action: WorkbenchAction::PanelClose {
            terminal_context_id: context,
            tab_id: tab,
            panel_id: panel,
        },
    };
    let accepted = coordinator
        .execute_terminal_tab_close(close.clone(), Some(worker(1)), true, Instant::now())
        .unwrap();
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::PanelCloseAccepted { .. }
    ));
    for _step in 0..8 {
        let Some(token) = coordinator
            .reserve_close_lifecycle_step(&close.request_id, Instant::now(), expiry())
            .unwrap()
        else {
            break;
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
                        close_activity:
                            orchbee_capability_node_protocol::LifecycleCloseActivity::Unknown,
                        revision: 3,
                        phase: LifecycleTerminalPhase::Exited {
                            terminate_requested: false,
                        },
                    }),
                })
            }
            _ => TerminalLifecycleOutcome::Disposed,
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
    let runtime = coordinator
        .close_authority()
        .unwrap()
        .terminal_tab_close_target(&close.request_id)
        .unwrap()
        .runtime()
        .expect("created Host runtime");
    coordinator
        .close_authority()
        .unwrap()
        .complete_terminal_tab_close_target_cleanup(&close.request_id, runtime)
        .unwrap();
    coordinator
        .publish_terminal_tab_close(&close.request_id, Instant::now())
        .unwrap();
    close
}

fn workspace_leaf_panels(
    coordinator: &mut SessionCoordinator,
) -> (
    WorkbenchTerminalContextId,
    WorkbenchTabId,
    Vec<WorkbenchPanelId>,
) {
    fn collect(node: &PanelNode, panels: &mut Vec<WorkbenchPanelId>) {
        match node {
            PanelNode::Panel { panel_id, .. } => panels.push(panel_id.clone()),
            PanelNode::Split { children, .. } => {
                for child in children.iter() {
                    collect(&child.node, panels);
                }
            }
        }
    }
    let committed = snapshot(coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let tab = committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap()
        .tabs
        .first()
        .unwrap();
    let mut panels = Vec::new();
    collect(&tab.root, &mut panels);
    (context, tab.tab_id.clone(), panels)
}

fn move_command(
    coordinator: &mut SessionCoordinator,
    request: u128,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    target_panel_id: WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
) -> WorkbenchCommand {
    let committed = snapshot(coordinator);
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        session_id: committed.session_id,
        expected_revision: committed.revision,
        action: WorkbenchAction::PanelMove {
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
            direction,
        },
    }
}

#[test]
fn pending_split_completion_after_close_retarget_settles_and_resolves_bare_open() {
    let mut coordinator = session();
    let create_a = tab_command(&coordinator, 1400, 0, "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 1401);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();

    let (context, tab, panel_a) = workspace_leaf(&mut coordinator);
    let split_b = split_command(
        &mut coordinator,
        1402,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_b = dispatch_split(&mut coordinator, split_b.clone(), 1403);

    // The real Close finalizer removes A and retargets the slot to still-pending B.
    let _close = close_leaf(&mut coordinator, context, tab, panel_a, 1404);

    let completion_b = immediate_completion(pending_b.command());
    let completed = coordinator
        .complete_terminal_panel_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();

    assert!(matches!(
        completed.result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));

    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(
            split_b,
            None,
            None,
            None,
            &LaunchCapture::absent()
        )),
        completed.result
    );
}

#[test]
fn split_completion_before_close_publication_leaves_the_settled_reference() {
    let mut coordinator = session();
    let create_a = tab_command(&coordinator, 1410, 0, "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 1411);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = workspace_leaf(&mut coordinator);
    let split_b = split_command(
        &mut coordinator,
        1412,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_b = dispatch_split(&mut coordinator, split_b.clone(), 1413);

    let completion_b = immediate_completion(pending_b.command());
    let completed = coordinator
        .complete_terminal_panel_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();

    assert!(matches!(
        completed.result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));

    // The real Close finalizer still retargets to B through its retained Started outcome.
    let _close = close_leaf(&mut coordinator, context, tab, panel_a, 1414);

    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(
            split_b,
            None,
            None,
            None,
            &LaunchCapture::absent()
        )),
        completed.result
    );
}

#[test]
fn stale_split_completion_cannot_overwrite_the_newer_current_reference() {
    let mut coordinator = session();
    let create_a = tab_command(&coordinator, 1420, 0, "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 1421);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = workspace_leaf(&mut coordinator);
    let split_b = split_command(
        &mut coordinator,
        1422,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_b = dispatch_split(&mut coordinator, split_b.clone(), 1423);
    let split_c = split_command(
        &mut coordinator,
        1424,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Left,
    );
    let pending_c = dispatch_split(&mut coordinator, split_c.clone(), 1425);

    // Removing A leaves [C, B]; the real retarget selects C, the first post-removal member.
    let _close = close_leaf(&mut coordinator, context, tab, panel_a, 1426);

    let completion_b = immediate_completion(pending_b.command());
    let completed_b = coordinator
        .complete_terminal_panel_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();

    assert!(matches!(
        completed_b.result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));

    let completion_c = immediate_completion(pending_c.command());
    let completed_c = coordinator
        .complete_terminal_panel_create(pending_c, completion_c, Some(worker(1)))
        .unwrap();

    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(
            split_b,
            None,
            None,
            None,
            &LaunchCapture::absent()
        )),
        completed_b.result
    );
    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(
            split_c,
            None,
            None,
            None,
            &LaunchCapture::absent()
        )),
        completed_c.result
    );
}

#[test]
fn workspace_split_completion_preserves_sibling_tab_settlement() {
    let mut coordinator = session();
    let context = configure_workspace(&mut coordinator, "/tmp/workspace");
    let first_open = pending_first_tab(&mut coordinator, 1430, 1431);

    let revision = snapshot(&mut coordinator).revision.value();
    let create = context_tab_command(&coordinator, 1432, revision, context.clone(), "workspace");
    let pending_tab = dispatch_tab(&mut coordinator, create, 1433);
    let completion_tab = immediate_completion(pending_tab.command());
    coordinator
        .complete_terminal_tab_create(pending_tab, completion_tab, Some(worker(1)))
        .unwrap();
    let committed = snapshot(&mut coordinator);
    let tab = &committed
        .terminal_contexts
        .iter()
        .find(|item| item.terminal_context_id == context)
        .unwrap()
        .tabs[0];
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!("simple panel expected")
    };
    let split = split_command(
        &mut coordinator,
        1434,
        context,
        tab.tab_id.clone(),
        panel_id.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_split = dispatch_split(&mut coordinator, split.clone(), 1435);

    let completion_split = immediate_completion(pending_split.command());
    let completed = coordinator
        .complete_terminal_panel_create(pending_split, completion_split, Some(worker(1)))
        .unwrap();

    assert!(matches!(
        completed.result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));

    assert!(matches!(
        coordinator
            .complete_terminal_tab_create(first_open, Completion::Unknown, Some(worker(1)))
            .unwrap()
            .result
            .outcome,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
            ..
        }
    ));

    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(
            split,
            None,
            None,
            None,
            &LaunchCapture::absent()
        )),
        completed.result
    );
}

#[test]
fn workspace_leaf_reorder_and_close_preserve_remaining_members() {
    let mut coordinator = session();
    let first_open = pending_first_tab(&mut coordinator, 1520, 1521);
    let completion = immediate_completion(first_open.command());
    coordinator
        .complete_terminal_tab_create(first_open, completion, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = workspace_leaf(&mut coordinator);
    let split_cmd_pending_b = split_command(
        &mut coordinator,
        1522,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_b = dispatch_split(&mut coordinator, split_cmd_pending_b, 1523);
    let completion_b = immediate_completion(pending_b.command());
    coordinator
        .complete_terminal_panel_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    let (_, _, panels) = workspace_leaf_panels(&mut coordinator);
    let [panel_a_committed, panel_b] = panels.as_slice() else {
        panic!("two committed leaves expected")
    };
    assert_eq!(*panel_a_committed, panel_a);

    // A legal reorder keeps the anchor; the Settled slot reference is not retargeted.
    let movement = move_command(
        &mut coordinator,
        1524,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        panel_b.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let moved = coordinator
        .execute_terminal_panel_placement(movement)
        .unwrap();
    assert!(matches!(
        moved.outcome,
        WorkbenchCommandOutcome::PanelMoved { .. }
    ));
    let (_, _, reordered) = workspace_leaf_panels(&mut coordinator);
    assert_eq!(reordered, vec![panel_b.clone(), panel_a.clone()]);

    // Closing the reordered anchor retargets the slot to the surviving leaf.
    let _close = close_leaf(
        &mut coordinator,
        context.clone(),
        tab.clone(),
        panel_a,
        1526,
    );

    // Closing the final leaf returns the slot to Empty; bare Open then starts fresh.
    let _close = close_leaf(&mut coordinator, context, tab, panel_b.clone(), 1528);

    let next_open = pending_first_tab(&mut coordinator, 1530, 1531);
    let completion = immediate_completion(next_open.command());
    coordinator
        .complete_terminal_tab_create(next_open, completion, Some(worker(1)))
        .unwrap();
}

#[test]
fn launch_selection_reaches_the_split_command_and_replay_identity() {
    let mut coordinator = session();
    let create_a = tab_command(&coordinator, 1600, 0, "anchor");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 1601);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = workspace_leaf(&mut coordinator);
    let mut split = split_command(
        &mut coordinator,
        1602,
        context,
        tab,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let WorkbenchAction::PanelSplit {
        launch, options, ..
    } = &mut split.action
    else {
        panic!("panel.split action expected")
    };
    options.cwd = Some(WorkbenchDirectoryPath::parse("C:\\launch space").unwrap());
    options.environment = orchbee_workbench_protocol::LaunchEnvironment::new(vec![
        ("KEY".into(), "literal=值".into()),
        ("EMPTY".into(), String::new()),
    ])
    .unwrap();
    *launch = Some(
        orchbee_workbench_protocol::LaunchSelection::new(
            "/bin/sh".to_owned(),
            vec!["-l".to_owned(), String::new(), "--flag".to_owned()],
        )
        .unwrap(),
    );

    // The pending OBCN command carries the selection verbatim.
    let pending = dispatch_split(&mut coordinator, split.clone(), 1603);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(authorized) = pending.command() else {
        panic!("terminal Launch expected")
    };
    assert_eq!(authorized.cwd().unwrap().as_str(), "C:\\launch space");
    assert_eq!(
        authorized.environment().entries(),
        &[
            ("KEY".into(), "literal=值".into()),
            ("EMPTY".into(), String::new())
        ]
    );
    let selected = authorized.launch().expect("explicit launch selection");
    assert_eq!(selected.executable(), "/bin/sh");
    assert_eq!(
        selected.arguments(),
        ["-l".to_owned(), String::new(), "--flag".to_owned()].as_slice()
    );

    // An identical replay returns the pending outcome without dispatching a
    // second launch.
    let replay = ready_split(coordinator.prepare_terminal_panel_create(
        split.clone(),
        None,
        None,
        None,
        &LaunchCapture::absent(),
    ));
    assert!(matches!(
        replay.outcome,
        WorkbenchCommandOutcome::PanelSplitPending { .. }
    ));

    // The same request id with a different selection conflicts before effect.
    let mut conflict = split.clone();
    let WorkbenchAction::PanelSplit { launch, .. } = &mut conflict.action else {
        panic!("panel.split action expected")
    };
    *launch = Some(
        orchbee_workbench_protocol::LaunchSelection::new("/bin/zsh".to_owned(), Vec::new())
            .unwrap(),
    );
    assert!(matches!(
        ready_split(coordinator.prepare_terminal_panel_create(
            conflict,
            None,
            None,
            None,
            &LaunchCapture::absent()
        ))
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));

    for change_cwd in [false, true] {
        let mut changed = split.clone();
        let WorkbenchAction::PanelSplit { options, .. } = &mut changed.action else {
            panic!("launch action");
        };
        if change_cwd {
            options.cwd = Some(WorkbenchDirectoryPath::parse("C:\\other").unwrap());
        } else {
            options.environment = orchbee_workbench_protocol::LaunchEnvironment::empty();
        }
        assert!(matches!(
            ready_split(coordinator.prepare_terminal_panel_create(
                changed,
                None,
                None,
                None,
                &LaunchCapture::absent()
            ))
            .outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
    }
}
