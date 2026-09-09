use super::*;
use orchbee_workbench_protocol::{
    WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalContextId,
};

fn move_command(
    coordinator: &SiteCoordinator,
    request: u128,
    revision: u64,
    tab_id: WorkbenchTabId,
    direction: WorkbenchTabMoveDirection,
) -> WorkbenchCommand {
    let workbench = coordinator.workbench.as_ref().unwrap();
    let terminal_context_id = workbench
        .standalone_membership()
        .unwrap()
        .terminal_context_id()
        .clone();
    move_command_for_context(
        coordinator,
        request,
        revision,
        terminal_context_id,
        tab_id,
        direction,
    )
}

fn move_command_for_context(
    coordinator: &SiteCoordinator,
    request: u128,
    revision: u64,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    direction: WorkbenchTabMoveDirection,
) -> WorkbenchCommand {
    let workbench = coordinator.workbench.as_ref().unwrap();
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        site_id: workbench.site_id().clone(),
        expected_revision: WorkbenchRevision::new(revision),
        action: WorkbenchAction::TabMove {
            terminal_context_id,
            tab_id,
            direction,
        },
    }
}

#[test]
fn pending_bootstrap_moves_through_every_position_then_both_completion_orders_survive_retirement() {
    for first_completes_first in [false, true] {
        let mut coordinator = site();
        let first_command = tab_command(&coordinator, 1100, 0, "first creation label");
        let first = dispatch_tab(&mut coordinator, first_command.clone(), 1101);
        let second_command = tab_command(&coordinator, 1102, 1, "second creation label");
        let second = dispatch_tab(&mut coordinator, second_command.clone(), 1103);
        let third_command = tab_command(&coordinator, 1104, 2, "third creation label");
        let third = dispatch_tab(&mut coordinator, third_command, 1105);
        coordinator
            .complete_terminal_tab_create(third, Completion::Busy, Some(worker(1)))
            .unwrap();
        let baseline = snapshot(&mut coordinator);
        let first_tab = &baseline.terminal_contexts[0].tabs[0];
        let pending_replay =
            ready(coordinator.prepare_terminal_tab_create(first_command.clone(), None, None, None));
        let mut historical_move = None;
        for (index, (direction, order)) in [
            (WorkbenchTabMoveDirection::Last, [1, 2, 0]),
            (WorkbenchTabMoveDirection::First, [0, 1, 2]),
            (WorkbenchTabMoveDirection::Next, [1, 0, 2]),
        ]
        .into_iter()
        .enumerate()
        {
            let request = move_command(
                &coordinator,
                1120 + index as u128,
                3 + index as u64,
                first_tab.tab_id.clone(),
                direction,
            );
            let result = coordinator
                .execute_terminal_tab_move(request.clone())
                .unwrap();
            assert_eq!(
                result.outcome,
                WorkbenchCommandOutcome::TabMoved {
                    revision: WorkbenchRevision::new(4 + index as u64),
                    terminal_context_id: baseline.navigation.standalone_terminal_context_id.clone(),
                    tab_id: first_tab.tab_id.clone(),
                }
            );
            let mut expected = baseline.clone();
            expected.revision = WorkbenchRevision::new(4 + index as u64);
            expected.terminal_contexts[0].tabs = order
                .map(|index| baseline.terminal_contexts[0].tabs[index].clone())
                .to_vec();
            assert_eq!(snapshot(&mut coordinator), expected);
            assert_eq!(
                ready(coordinator.prepare_terminal_tab_create(
                    first_command.clone(),
                    None,
                    None,
                    None
                )),
                pending_replay
            );
            assert!(matches!(
                coordinator.prepare_open(
                    &orchbee_client_server_protocol::TerminalOpenRequest::new(id(1150), geometry()),
                    None,
                    Some(worker(1)),
                    None,
                ),
                PreparedOpen::Wait
            ));
            if historical_move.is_none() {
                historical_move = Some((request, result));
            }
        }
        let (historical_request, historical_result) = historical_move.unwrap();
        assert_eq!(
            coordinator
                .execute_terminal_tab_move(historical_request.clone())
                .unwrap(),
            historical_result
        );
        let membership = coordinator
            .workbench
            .as_ref()
            .unwrap()
            .standalone_membership()
            .unwrap();
        coordinator
            .workbench
            .as_mut()
            .unwrap()
            .retire_standalone_composition();
        let first_completion = immediate_tab_completion(&first);
        let (first_result, second_result) = if first_completes_first {
            (
                coordinator
                    .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
                    .unwrap()
                    .result,
                coordinator
                    .complete_terminal_tab_create(second, Completion::Unknown, Some(worker(1)))
                    .unwrap()
                    .result,
            )
        } else {
            let second_result = coordinator
                .complete_terminal_tab_create(second, Completion::Unknown, Some(worker(1)))
                .unwrap()
                .result;
            assert!(
                matches!(&coordinator.standalone, StandaloneTerminal::Creating(reference) if reference == &id(1101))
            );
            (
                coordinator
                    .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
                    .unwrap()
                    .result,
                second_result,
            )
        };
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = &first_tab.root
        else {
            unreachable!()
        };
        assert_eq!(
            first_result.outcome,
            WorkbenchCommandOutcome::TabCreated {
                revision: WorkbenchRevision::new(1),
                terminal_context_id: baseline.navigation.standalone_terminal_context_id,
                tab_id: first_tab.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap()
                },
            }
        );
        assert!(
            matches!(second_result.outcome, WorkbenchCommandOutcome::TabCreated { revision, terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {}, .. } if revision == WorkbenchRevision::new(2))
        );
        assert_eq!(
            coordinator
                .workbench
                .as_ref()
                .unwrap()
                .standalone_membership()
                .unwrap(),
            membership
        );
        assert!(matches!(coordinator.prepare_open(
            &orchbee_client_server_protocol::TerminalOpenRequest::new(id(1151), geometry()), None, Some(worker(1)), None,
        ), PreparedOpen::Ready(OpenOutcome::Opened { terminal_create_request_id }) if terminal_create_request_id == id(1101)));
        for result in [
            coordinator
                .execute_terminal_tab_move(historical_request.clone())
                .unwrap(),
            ready(coordinator.prepare_terminal_tab_create(first_command, None, None, None)),
            ready(coordinator.prepare_terminal_tab_create(second_command, None, None, None)),
        ] {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
                }
            ));
        }
        let fresh = move_command(
            &coordinator,
            1160,
            6,
            first_tab.tab_id.clone(),
            WorkbenchTabMoveDirection::Previous,
        );
        assert!(matches!(
            coordinator
                .execute_terminal_tab_move(fresh)
                .unwrap()
                .outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
            }
        ));
        assert_eq!(coordinator.allocate_identity(), Some(4_u128.to_be_bytes()));
    }
}

#[test]
fn move_replay_precedes_composition_but_new_missing_target_and_noop_do_not() {
    for missing_target in [false, true] {
        let mut coordinator = site();
        let create = tab_command(&coordinator, 1200, 0, "first");
        let _pending = dispatch_tab(&mut coordinator, create, 1201);
        let before = snapshot(&mut coordinator);
        let tab_id = before.terminal_contexts[0].tabs[0].tab_id.clone();
        let retained = move_command(
            &coordinator,
            1202,
            1,
            tab_id.clone(),
            WorkbenchTabMoveDirection::First,
        );
        let result = coordinator
            .execute_terminal_tab_move(retained.clone())
            .unwrap();
        coordinator.standalone = StandaloneTerminal::Creating(id(999));
        assert_eq!(
            coordinator
                .execute_terminal_tab_move(retained.clone())
                .unwrap(),
            result
        );
        let mut conflict = retained;
        conflict.expected_revision = WorkbenchRevision::new(0);
        assert!(matches!(
            coordinator
                .execute_terminal_tab_move(conflict)
                .unwrap()
                .outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ));
        let target = if missing_target {
            WorkbenchTabId::parse(&format!("{:032x}", 999)).unwrap()
        } else {
            tab_id
        };
        let fresh = move_command(
            &coordinator,
            1203,
            1,
            target,
            WorkbenchTabMoveDirection::First,
        );
        assert!(matches!(
            coordinator
                .execute_terminal_tab_move(fresh)
                .unwrap()
                .outcome,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
            }
        ));
        assert!(
            matches!(&coordinator.standalone, StandaloneTerminal::Creating(reference) if reference == &id(999))
        );
        assert_eq!(coordinator.allocate_identity(), Some(2_u128.to_be_bytes()));
    }
}

#[test]
fn workspace_pending_create_move_completion_preserves_origin_and_standalone_states() {
    let mut coordinator = site();
    let context = configure_workspace(&mut coordinator, "/tmp/workspace");
    let first_command = context_tab_command(&coordinator, 1300, 3, context.clone(), "workspace A");
    let first = dispatch_tab(&mut coordinator, first_command.clone(), 1301);
    let first_snapshot = snapshot(&mut coordinator);
    let workspace_index = first_snapshot
        .terminal_contexts
        .iter()
        .position(|candidate| candidate.terminal_context_id == context)
        .unwrap();
    let first_tab = first_snapshot.terminal_contexts[workspace_index].tabs[0].clone();
    let first_pending =
        ready(coordinator.prepare_terminal_tab_create(first_command.clone(), None, None, None));
    assert!(matches!(&first_pending.outcome,
        WorkbenchCommandOutcome::TabCreatePending { revision, tab_id, .. }
        if *revision == WorkbenchRevision::new(4) && tab_id == &first_tab.tab_id
    ));
    let second_command = context_tab_command(&coordinator, 1302, 4, context.clone(), "workspace B");
    let second = dispatch_tab(&mut coordinator, second_command, 1303);
    let second_completion = immediate_tab_completion(&second);
    let second_completed = coordinator
        .complete_terminal_tab_create(second, second_completion, Some(worker(1)))
        .unwrap();
    assert!(matches!(second_completed.result.outcome,
        WorkbenchCommandOutcome::TabCreated { revision, terminal_outcome: WorkbenchTerminalOutcome::Started { .. }, .. }
        if revision == WorkbenchRevision::new(5)
    ));
    assert!(!second_completed.standalone_changed);

    // Workspace moves cover all three slot states; the first Create spans the first two.
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));
    let empty_move = move_workspace_pair(&mut coordinator, &context, 1330);
    assert!(matches!(coordinator.standalone, StandaloneTerminal::Empty));
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(first_command.clone(), None, None, None,)),
        first_pending
    );

    let first_open = pending_first_open(&mut coordinator, 1350, 1351);
    let creating_move = move_workspace_pair(&mut coordinator, &context, 1331);
    assert!(matches!(&coordinator.standalone,
        StandaloneTerminal::Creating(reference) if reference == &id(1351)
    ));
    // Finish at a different position from creation; two swaps alone restore the old index.
    let pending_move = move_workspace_pair(&mut coordinator, &context, 1333);
    let after_creating_move = snapshot(&mut coordinator);
    assert_ne!(
        after_creating_move.terminal_contexts[workspace_index].tabs[0].tab_id,
        first_tab.tab_id
    );
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(first_command.clone(), None, None, None,)),
        first_pending
    );
    let first_completion = immediate_tab_completion(&first);
    let first_completed = coordinator
        .complete_terminal_tab_create(first, first_completion, Some(worker(1)))
        .unwrap();
    assert!(matches!(&first_completed.result.outcome,
        WorkbenchCommandOutcome::TabCreated { revision, terminal_context_id, tab_id,
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. }, .. }
        if *revision == WorkbenchRevision::new(4)
            && terminal_context_id == &context && tab_id == &first_tab.tab_id
    ));
    assert!(!first_completed.standalone_changed);
    assert_eq!(snapshot(&mut coordinator), after_creating_move);
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(first_command, None, None, None,)),
        first_completed.result
    );
    assert!(matches!(&coordinator.standalone,
        StandaloneTerminal::Creating(reference) if reference == &id(1351)
    ));

    let open_completion = immediate_completion(first_open.command());
    coordinator
        .complete(first_open, open_completion, Some(worker(1)))
        .unwrap();
    assert_eq!(snapshot(&mut coordinator), after_creating_move);
    let settled_move = move_workspace_pair(&mut coordinator, &context, 1332);
    let final_snapshot = snapshot(&mut coordinator);
    for (request, response) in [empty_move, creating_move, pending_move, settled_move] {
        assert_eq!(
            coordinator.execute_terminal_tab_move(request).unwrap(),
            response
        );
        assert_eq!(snapshot(&mut coordinator), final_snapshot);
    }
    assert!(matches!(&coordinator.standalone,
        StandaloneTerminal::Settled { reference,
            outcome: OpenOutcome::Opened { terminal_create_request_id } }
        if reference == &id(1351) && terminal_create_request_id == &id(1351)
    ));
    let final_tab = final_snapshot.terminal_contexts[workspace_index]
        .tabs
        .iter()
        .find(|tab| tab.tab_id == first_tab.tab_id)
        .unwrap();
    assert_eq!(primary_binding(final_tab), primary_binding(&first_tab));
}

fn move_workspace_pair(
    coordinator: &mut SiteCoordinator,
    context: &WorkbenchTerminalContextId,
    request: u128,
) -> (WorkbenchCommand, WorkbenchCommandResult) {
    let before = snapshot(coordinator);
    let index = before
        .terminal_contexts
        .iter()
        .position(|candidate| &candidate.terminal_context_id == context)
        .unwrap();
    assert_eq!(before.terminal_contexts[index].tabs.len(), 2);
    let tab_id = before.terminal_contexts[index].tabs[0].tab_id.clone();
    let request = move_command_for_context(
        coordinator,
        request,
        before.revision.value(),
        context.clone(),
        tab_id.clone(),
        WorkbenchTabMoveDirection::Last,
    );
    let response = coordinator
        .execute_terminal_tab_move(request.clone())
        .unwrap();
    let mut expected = before;
    expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
    expected.terminal_contexts[index].tabs.swap(0, 1);
    assert_eq!(
        response.outcome,
        WorkbenchCommandOutcome::TabMoved {
            revision: expected.revision,
            terminal_context_id: context.clone(),
            tab_id,
        }
    );
    assert_eq!(snapshot(coordinator), expected);
    (request, response)
}
fn primary_binding(
    tab: &orchbee_workbench_protocol::Tab,
) -> orchbee_workbench_protocol::PresentationBinding {
    let orchbee_workbench_protocol::PanelNode::Panel {
        primary_presentation,
        ..
    } = &tab.root
    else {
        panic!("simple Workspace Tab expected")
    };
    primary_presentation
        .binding
        .clone()
        .expect("Workspace Tab binding expected")
}
