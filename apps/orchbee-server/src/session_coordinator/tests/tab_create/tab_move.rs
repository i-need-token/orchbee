use super::*;
use crate::test_support::WorkbenchFixture;
use orchbee_workbench_protocol::{
    WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchTerminalContextId,
};

fn move_command(
    coordinator: &SessionCoordinator,
    request: u128,
    revision: u64,
    tab_id: WorkbenchTabId,
    direction: WorkbenchTabMoveDirection,
) -> WorkbenchCommand {
    let workbench = coordinator.workbench.as_ref().unwrap();
    let terminal_context_id = workbench
        .fixture_membership()
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
    coordinator: &SessionCoordinator,
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
        session_id: workbench.session_id().clone(),
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
        let mut coordinator = session();
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
                    terminal_context_id: baseline.navigation.workspaces[0]
                        .terminal_context_id
                        .clone(),
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
            .fixture_membership()
            .unwrap();
        coordinator
            .workbench
            .as_mut()
            .unwrap()
            .retire_terminal_membership();
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
                terminal_context_id: baseline.navigation.workspaces[0]
                    .terminal_context_id
                    .clone(),
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
                .fixture_membership()
                .unwrap(),
            membership
        );

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
fn workspace_pending_create_move_completion_preserves_origin_and_initial_workspace_states() {
    let mut coordinator = session();
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

    // Workspace moves cover all three slot states; the first Create spans the first two.

    let empty_move = move_workspace_pair(&mut coordinator, &context, 1330);

    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(first_command.clone(), None, None, None,)),
        first_pending
    );

    let first_open = pending_first_tab(&mut coordinator, 1350, 1351);
    let creating_move = move_workspace_pair(&mut coordinator, &context, 1331);

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

    assert_eq!(snapshot(&mut coordinator), after_creating_move);
    assert_eq!(
        ready(coordinator.prepare_terminal_tab_create(first_command, None, None, None,)),
        first_completed.result
    );

    let open_completion = immediate_completion(first_open.command());
    coordinator
        .complete_terminal_tab_create(first_open, open_completion, Some(worker(1)))
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

    let final_tab = final_snapshot.terminal_contexts[workspace_index]
        .tabs
        .iter()
        .find(|tab| tab.tab_id == first_tab.tab_id)
        .unwrap();
    assert_eq!(primary_binding(final_tab), primary_binding(&first_tab));
}

fn move_workspace_pair(
    coordinator: &mut SessionCoordinator,
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
    let mut request = move_command_for_context(
        coordinator,
        request,
        before.revision.value(),
        context.clone(),
        tab_id.clone(),
        WorkbenchTabMoveDirection::Last,
    );
    let WorkbenchAction::TabMove {
        terminal_context_id,
        tab_id,
        ..
    } = request.action
    else {
        unreachable!()
    };
    request.action = WorkbenchAction::TabMoveIndex {
        terminal_context_id,
        tab_id: tab_id.clone(),
        insert_index: orchbee_workbench_protocol::WorkbenchTabInsertIndex::new(2).unwrap(),
    };
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
