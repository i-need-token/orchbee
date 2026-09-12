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
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchLabel, WorkbenchPanelId, WorkbenchPanelSplitDirection, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
};
use std::time::Instant;

fn split_command(
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
            terminal_context_id,
            tab_id,
            panel_id,
            direction,
            label: WorkbenchLabel::parse("split").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        },
    }
}

fn dispatch_split(
    coordinator: &mut SiteCoordinator,
    command: WorkbenchCommand,
    reference: u128,
) -> PendingTerminalPanelSplit {
    let PreparedTerminalPanelSplit::Dispatch(pending) = coordinator.prepare_terminal_panel_split(
        command,
        Some(id(reference)),
        Some(expiry()),
        Some(worker(1)),
    ) else {
        panic!("panel.split dispatch expected")
    };
    pending
}

fn ready_split(prepared: PreparedTerminalPanelSplit) -> WorkbenchCommandResult {
    let PreparedTerminalPanelSplit::Ready(result) = prepared else {
        panic!("ready panel.split result expected")
    };
    result
}

fn standalone_leaf(
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
fn close_leaf(
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
        .runtime();
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
    coordinator.prepare_open(
        &orchbee_client_server_protocol::TerminalOpenRequest::new(id(request), geometry()),
        None,
        Some(worker(1)),
        None,
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
        .complete_terminal_panel_split(pending_b, completion_b, Some(worker(1)))
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
        ready_split(coordinator.prepare_terminal_panel_split(split_b, None, None, None)),
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
        .complete_terminal_panel_split(pending_b, completion_b, Some(worker(1)))
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
        ready_split(coordinator.prepare_terminal_panel_split(split_b, None, None, None)),
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
        .complete_terminal_panel_split(pending_b, completion_b, Some(worker(1)))
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
        .complete_terminal_panel_split(pending_c, completion_c, Some(worker(1)))
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
        ready_split(coordinator.prepare_terminal_panel_split(split_b, None, None, None)),
        completed_b.result
    );
    assert_eq!(
        ready_split(coordinator.prepare_terminal_panel_split(split_c, None, None, None)),
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
        .complete_terminal_panel_split(pending_split, completion_split, Some(worker(1)))
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
        ready_split(coordinator.prepare_terminal_panel_split(split, None, None, None)),
        completed.result
    );
}
