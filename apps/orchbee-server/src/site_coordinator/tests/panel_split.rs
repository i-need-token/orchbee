use super::tab_create::{
    configure_workspace, context_tab_command, dispatch_tab, immediate_completion,
    pending_first_open, tab_command,
};
use super::*;
use orchbee_capability_node_protocol::{
    LifecycleInventoryPage, LifecycleTargetObservation, LifecycleTerminalPhase,
    TerminalLifecycleAction, TerminalLifecycleOutcome, TerminalLifecycleResult,
};
use orchbee_client_server_protocol::OpenOutcome;
use orchbee_server::CloseStepObservation;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;
use orchbee_workbench_protocol::{
    PublicationReadIntent, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchLabel, WorkbenchPanelId, WorkbenchPanelSplitDirection,
    WorkbenchPublicationOutcome, WorkbenchRejectionReason, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
};
use std::time::Instant;

pub(super) fn split_command(
    coordinator: &mut SiteCoordinator,
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
        site_id: committed.site_id,
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
    coordinator: &mut SiteCoordinator,
    command: WorkbenchCommand,
    reference: u128,
) -> PendingTerminalPanelCreate {
    let PreparedTerminalPanelCreate::Dispatch(pending) = coordinator.prepare_terminal_panel_create(
        command,
        Some(id(reference)),
        Some(expiry()),
        Some(worker(1)),
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

pub(super) fn standalone_leaf(
    coordinator: &mut SiteCoordinator,
) -> (WorkbenchTerminalContextId, WorkbenchTabId, WorkbenchPanelId) {
    let committed = snapshot(coordinator);
    let context = committed.navigation.standalone_terminal_context_id;
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

// These fabricated replies prove Site refinement of the real Close finalizer, not Worker behavior.
pub(super) fn close_leaf(
    coordinator: &mut SiteCoordinator,
    context: WorkbenchTerminalContextId,
    tab: WorkbenchTabId,
    panel: WorkbenchPanelId,
    request: u128,
) -> (WorkbenchCommand, bool) {
    let committed = snapshot(coordinator);
    let close = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        site_id: committed.site_id,
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
    let changed = coordinator
        .publish_terminal_tab_close(&close.request_id, Instant::now())
        .unwrap();
    (close, changed)
}

fn open_probe(coordinator: &mut SiteCoordinator, request: u128) -> PreparedOpen {
    open_probe_with_worker(coordinator, request, Some(worker(1)))
}

fn open_probe_with_worker(
    coordinator: &mut SiteCoordinator,
    request: u128,
    placement: Option<WorkerPlacement>,
) -> PreparedOpen {
    coordinator.prepare_open(
        &orchbee_client_server_protocol::TerminalOpenRequest::new(id(request), geometry()),
        None,
        placement,
        None,
    )
}

fn standalone_leaf_panels(
    coordinator: &mut SiteCoordinator,
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
    let context = committed.navigation.standalone_terminal_context_id;
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
    coordinator: &mut SiteCoordinator,
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
        site_id: committed.site_id,
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

fn workbench_snapshot_unavailable(coordinator: &mut SiteCoordinator) -> bool {
    let response = coordinator
        .execute_workbench(
            WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 889)).unwrap(),
            })
            .try_into()
            .unwrap(),
            false,
        )
        .unwrap();
    matches!(
        response,
        WorkbenchResponse::Snapshot(result)
            if matches!(result.outcome, WorkbenchSnapshotOutcome::Unavailable { .. })
    )
}

fn publication_available(
    coordinator: &mut SiteCoordinator,
    request: u128,
    known_revision: WorkbenchRevision,
) -> bool {
    let response = coordinator
        .execute_workbench(
            WorkbenchRequest::PublicationRead(PublicationReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
                known_revision,
            })
            .try_into()
            .unwrap(),
            false,
        )
        .unwrap();
    matches!(
        response,
        WorkbenchResponse::Publication(result)
            if !matches!(result.outcome, WorkbenchPublicationOutcome::Unavailable { .. })
    )
}

#[test]
fn pending_split_completion_after_close_retarget_settles_and_resolves_bare_open() {
    let mut coordinator = site();
    let create_a = tab_command(&coordinator, 1400, 0, "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 1401);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1401),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1401)
            },
        }
    );
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
    let split_b = split_command(
        &mut coordinator,
        1402,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_b = dispatch_split(&mut coordinator, split_b.clone(), 1403);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1401),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1401)
            },
        }
    );

    // The real Close finalizer removes A and retargets the slot to still-pending B.
    let (_close, changed) = close_leaf(&mut coordinator, context, tab, panel_a, 1404);
    assert!(changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Creating(id(1403))
    );
    assert!(matches!(
        open_probe(&mut coordinator, 1405),
        PreparedOpen::Wait
    ));

    let completion_b = immediate_completion(pending_b.command());
    let completed = coordinator
        .complete_terminal_panel_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    assert!(completed.standalone_changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1403),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1403)
            },
        }
    );
    assert!(matches!(
        completed.result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
    assert!(matches!(
        open_probe(&mut coordinator, 1406),
        PreparedOpen::Ready(OpenOutcome::Opened {
            terminal_create_request_id
        }) if terminal_create_request_id == id(1403)
    ));
    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(split_b, None, None, None)),
        completed.result
    );
}

#[test]
fn split_completion_before_close_publication_leaves_the_settled_reference() {
    let mut coordinator = site();
    let create_a = tab_command(&coordinator, 1410, 0, "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 1411);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
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
    assert!(!completed.standalone_changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1411),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1411)
            },
        }
    );
    assert!(matches!(
        completed.result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));

    // The real Close finalizer still retargets to B through its retained Started outcome.
    let (_close, changed) = close_leaf(&mut coordinator, context, tab, panel_a, 1414);
    assert!(changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1413),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1413)
            },
        }
    );
    assert!(matches!(
        open_probe(&mut coordinator, 1415),
        PreparedOpen::Ready(OpenOutcome::Opened {
            terminal_create_request_id
        }) if terminal_create_request_id == id(1413)
    ));
    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(split_b, None, None, None)),
        completed.result
    );
}

#[test]
fn stale_split_completion_cannot_overwrite_the_newer_current_reference() {
    let mut coordinator = site();
    let create_a = tab_command(&coordinator, 1420, 0, "first");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 1421);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
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
    let (_close, changed) = close_leaf(&mut coordinator, context, tab, panel_a, 1426);
    assert!(changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Creating(id(1425))
    );

    let completion_b = immediate_completion(pending_b.command());
    let completed_b = coordinator
        .complete_terminal_panel_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    assert!(!completed_b.standalone_changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Creating(id(1425))
    );
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
    assert!(completed_c.standalone_changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1425),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1425)
            },
        }
    );
    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(split_b, None, None, None)),
        completed_b.result
    );
    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(split_c, None, None, None)),
        completed_c.result
    );
}

#[test]
fn workspace_split_completion_never_touches_the_standalone_slot() {
    let mut coordinator = site();
    let context = configure_workspace(&mut coordinator, "/tmp/workspace");
    let first_open = pending_first_open(&mut coordinator, 1430, 1431);
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Creating(ref reference) if *reference == id(1431)
    ));
    let revision = snapshot(&mut coordinator).revision.value();
    let create = context_tab_command(&coordinator, 1432, revision, context.clone(), "workspace");
    let pending_tab = dispatch_tab(&mut coordinator, create, 1433);
    let completion_tab = immediate_completion(pending_tab.command());
    assert!(
        !coordinator
            .complete_terminal_tab_create(pending_tab, completion_tab, Some(worker(1)))
            .unwrap()
            .standalone_changed
    );
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
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Creating(ref reference) if *reference == id(1431)
    ));

    let completion_split = immediate_completion(pending_split.command());
    let completed = coordinator
        .complete_terminal_panel_create(pending_split, completion_split, Some(worker(1)))
        .unwrap();
    assert!(!completed.standalone_changed);
    assert!(matches!(
        completed.result.outcome,
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Creating(ref reference) if *reference == id(1431)
    ));

    assert!(matches!(
        coordinator
            .complete(first_open, Completion::Unknown, Some(worker(1)))
            .unwrap(),
        ServerMessage::TerminalCreateResult(_)
    ));
    assert!(matches!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            ref reference,
            outcome: OpenOutcome::OutcomeUnknown {},
        } if *reference == id(1431)
    ));
    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_create(split, None, None, None)),
        completed.result
    );
}

#[test]
fn bare_open_replays_the_settled_reference_across_a_pending_and_settled_split_leaf() {
    let mut coordinator = site();
    let first_open = pending_first_open(&mut coordinator, 1500, 1501);
    let completion = immediate_completion(first_open.command());
    coordinator
        .complete(first_open, completion, Some(worker(1)))
        .unwrap();
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1501),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1501)
            },
        }
    );
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
    let split_cmd_pending_b = split_command(
        &mut coordinator,
        1502,
        context,
        tab,
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let pending_b = dispatch_split(&mut coordinator, split_cmd_pending_b, 1503);
    // Both leaves are committed members; B's terminal is still pending.
    let committed = snapshot(&mut coordinator);
    assert_eq!(committed.revision, WorkbenchRevision::new(2));
    let (_, _, committed_panels) = standalone_leaf_panels(&mut coordinator);
    assert_eq!(committed_panels.len(), 2);
    assert_eq!(committed_panels[0], panel_a);
    for request in [1504, 1505] {
        assert!(matches!(
            open_probe(&mut coordinator, request),
            PreparedOpen::Ready(OpenOutcome::Opened {
                terminal_create_request_id
            }) if terminal_create_request_id == id(1501)
        ));
    }
    // Replay leaves the slot, the revision, and availability untouched.
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1501),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1501)
            },
        }
    );
    assert_eq!(snapshot(&mut coordinator), committed);
    assert!(publication_available(
        &mut coordinator,
        1510,
        committed.revision
    ));

    let completion_b = immediate_completion(pending_b.command());
    let completed = coordinator
        .complete_terminal_panel_create(pending_b, completion_b, Some(worker(1)))
        .unwrap();
    assert!(!completed.standalone_changed);
    for request in [1506, 1507] {
        assert!(matches!(
            open_probe(&mut coordinator, request),
            PreparedOpen::Ready(OpenOutcome::Opened {
                terminal_create_request_id
            }) if terminal_create_request_id == id(1501)
        ));
    }
    let (_, _, panels) = standalone_leaf_panels(&mut coordinator);
    assert_eq!(panels, committed_panels);
}

#[test]
fn bare_open_replay_survives_a_legal_leaf_reorder_and_retargets_only_through_close() {
    let mut coordinator = site();
    let first_open = pending_first_open(&mut coordinator, 1520, 1521);
    let completion = immediate_completion(first_open.command());
    coordinator
        .complete(first_open, completion, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
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
    let (_, _, panels) = standalone_leaf_panels(&mut coordinator);
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
    let (_, _, reordered) = standalone_leaf_panels(&mut coordinator);
    assert_eq!(reordered, vec![panel_b.clone(), panel_a.clone()]);
    assert!(matches!(
        open_probe(&mut coordinator, 1525),
        PreparedOpen::Ready(OpenOutcome::Opened {
            terminal_create_request_id
        }) if terminal_create_request_id == id(1521)
    ));
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1521),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1521)
            },
        }
    );

    // Closing the reordered anchor retargets the slot to the surviving leaf.
    let (_close, changed) = close_leaf(
        &mut coordinator,
        context.clone(),
        tab.clone(),
        panel_a,
        1526,
    );
    assert!(changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1523),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1523)
            },
        }
    );
    assert!(matches!(
        open_probe(&mut coordinator, 1527),
        PreparedOpen::Ready(OpenOutcome::Opened {
            terminal_create_request_id
        }) if terminal_create_request_id == id(1523)
    ));

    // Closing the final leaf returns the slot to Empty; bare Open then starts fresh.
    let (_close, changed) = close_leaf(&mut coordinator, context, tab, panel_b.clone(), 1528);
    assert!(changed);
    assert_eq!(coordinator.standalone, StandaloneTerminal::Empty);
    assert!(matches!(
        open_probe(&mut coordinator, 1529),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy
        })
    ));
    let next_open = pending_first_open(&mut coordinator, 1530, 1531);
    let completion = immediate_completion(next_open.command());
    coordinator
        .complete(next_open, completion, Some(worker(1)))
        .unwrap();
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1531),
            outcome: OpenOutcome::Opened {
                terminal_create_request_id: id(1531)
            },
        }
    );
}

#[test]
fn bare_open_rejects_a_slot_reference_without_retained_authorization_on_a_multi_leaf_tree() {
    let mut coordinator = site();
    let first_open = pending_first_open(&mut coordinator, 1540, 1541);
    let completion = immediate_completion(first_open.command());
    coordinator
        .complete(first_open, completion, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
    let split_cmd_pending_b = split_command(
        &mut coordinator,
        1542,
        context,
        tab,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let _pending_b = dispatch_split(&mut coordinator, split_cmd_pending_b, 1543);
    assert!(!workbench_snapshot_unavailable(&mut coordinator));

    // A Settled reference that was never retained must not replay just because the tree is valid.
    coordinator.standalone = StandaloneTerminal::Settled {
        reference: id(9001),
        outcome: OpenOutcome::Opened {
            terminal_create_request_id: id(9001),
        },
    };
    assert!(matches!(
        open_probe(&mut coordinator, 1544),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy
        })
    ));
    assert!(workbench_snapshot_unavailable(&mut coordinator));
    assert!(matches!(
        open_probe(&mut coordinator, 1545),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy
        })
    ));
}

#[test]
fn bare_open_rejects_an_anchor_binding_absent_from_the_multi_leaf_tree() {
    let mut coordinator = site();
    let first_open = pending_first_open(&mut coordinator, 1550, 1551);
    let completion = immediate_completion(first_open.command());
    coordinator
        .complete(first_open, completion, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
    let split_cmd_pending_b = split_command(
        &mut coordinator,
        1552,
        context,
        tab,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let _pending_b = dispatch_split(&mut coordinator, split_cmd_pending_b, 1553);

    // Retain an authorized terminal create that never joins the standalone tree: the slot
    // reference resolves but its binding occurs zero times among the committed leaves.
    let _phantom = dispatch(coordinator.prepare(create(9002), Some(worker(1)), Some(expiry())));
    coordinator.standalone = StandaloneTerminal::Settled {
        reference: id(9002),
        outcome: OpenOutcome::Opened {
            terminal_create_request_id: id(9002),
        },
    };
    assert!(matches!(
        open_probe(&mut coordinator, 1554),
        PreparedOpen::Ready(OpenOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy
        })
    ));
    assert!(workbench_snapshot_unavailable(&mut coordinator));
}

#[test]
fn bare_open_reports_wrong_worker_generation_without_retiring_a_multi_leaf_tree() {
    let mut coordinator = site();
    let first_open = pending_first_open(&mut coordinator, 1560, 1561);
    let completion = immediate_completion(first_open.command());
    coordinator
        .complete(first_open, completion, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
    let split_cmd_pending_b = split_command(
        &mut coordinator,
        1562,
        context,
        tab,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let _pending_b = dispatch_split(&mut coordinator, split_cmd_pending_b, 1563);
    let committed = snapshot(&mut coordinator);

    // A stale Worker generation is a typed client result, not a composition divergence.
    for placement in [Some(worker(2)), None] {
        assert!(matches!(
            open_probe_with_worker(&mut coordinator, 1564, placement),
            PreparedOpen::Ready(OpenOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable
            })
        ));
    }
    assert!(!workbench_snapshot_unavailable(&mut coordinator));
    assert_eq!(snapshot(&mut coordinator), committed);
    assert!(matches!(
        open_probe(&mut coordinator, 1565),
        PreparedOpen::Ready(OpenOutcome::Opened {
            terminal_create_request_id
        }) if terminal_create_request_id == id(1561)
    ));
}

#[test]
fn bare_open_waits_on_a_creating_slot_over_a_multi_leaf_tree() {
    let mut coordinator = site();
    let first_open = pending_first_open(&mut coordinator, 1570, 1571);
    let completion = immediate_completion(first_open.command());
    coordinator
        .complete(first_open, completion, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
    let split_cmd_pending_b = split_command(
        &mut coordinator,
        1572,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Right,
    );
    let _pending_b = dispatch_split(&mut coordinator, split_cmd_pending_b, 1573);
    let split_cmd_pending_c = split_command(
        &mut coordinator,
        1574,
        context.clone(),
        tab.clone(),
        panel_a.clone(),
        WorkbenchPanelSplitDirection::Left,
    );
    let _pending_c = dispatch_split(&mut coordinator, split_cmd_pending_c, 1575);

    // Removing A leaves [C, B] and retargets the slot to still-pending C.
    let (_close, changed) = close_leaf(&mut coordinator, context, tab, panel_a, 1576);
    assert!(changed);
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Creating(id(1575))
    );
    let (_, _, panels) = standalone_leaf_panels(&mut coordinator);
    assert_eq!(panels.len(), 2);

    // Every waiter keeps waiting; none dispatches or retires the composition.
    for request in [1577, 1578] {
        assert!(matches!(
            open_probe(&mut coordinator, request),
            PreparedOpen::Wait
        ));
    }
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Creating(id(1575))
    );
    assert!(!workbench_snapshot_unavailable(&mut coordinator));
}

#[test]
fn bare_open_replays_a_non_opened_settled_result_over_a_multi_leaf_tree() {
    let mut coordinator = site();
    let first_open = pending_first_open(&mut coordinator, 1580, 1581);
    coordinator
        .complete(first_open, Completion::Unknown, Some(worker(1)))
        .unwrap();
    assert_eq!(
        coordinator.standalone,
        StandaloneTerminal::Settled {
            reference: id(1581),
            outcome: OpenOutcome::OutcomeUnknown {},
        }
    );
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
    let split_cmd_pending_b = split_command(
        &mut coordinator,
        1582,
        context,
        tab,
        panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let _pending_b = dispatch_split(&mut coordinator, split_cmd_pending_b, 1583);
    for request in [1584, 1585] {
        assert!(matches!(
            open_probe(&mut coordinator, request),
            PreparedOpen::Ready(OpenOutcome::OutcomeUnknown {})
        ));
    }
    assert!(!workbench_snapshot_unavailable(&mut coordinator));
}

#[test]
fn launch_selection_reaches_the_split_command_and_replay_identity() {
    let mut coordinator = site();
    let create_a = tab_command(&coordinator, 1600, 0, "anchor");
    let pending_a = dispatch_tab(&mut coordinator, create_a, 1601);
    let completion_a = immediate_completion(pending_a.command());
    coordinator
        .complete_terminal_tab_create(pending_a, completion_a, Some(worker(1)))
        .unwrap();
    let (context, tab, panel_a) = standalone_leaf(&mut coordinator);
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
    let replay =
        ready_split(coordinator.prepare_terminal_panel_create(split.clone(), None, None, None));
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
        ready_split(coordinator.prepare_terminal_panel_create(conflict, None, None, None)).outcome,
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
            ready_split(coordinator.prepare_terminal_panel_create(changed, None, None, None))
                .outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
    }
}
