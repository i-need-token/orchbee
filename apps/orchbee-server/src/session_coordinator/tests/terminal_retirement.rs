//! Terminal binding retirement: a Close that ends a terminal frees its place under the binding
//! bound, late requests naming it read it as unknown, and a full Session names its limit.

use super::panel_split::{close_leaf, workspace_leaf};
use super::tab_create::{context_tab_command, dispatch_tab, immediate_completion};
use super::*;
use crate::terminal_coordinator::MAXIMUM_TERMINAL_BINDINGS;
use orchbee_capability_node_protocol::TerminalLifecycleAction;
use orchbee_client_server_protocol::{StartOutcome, StartRejectedReason, TerminalStartRequest};
use orchbee_workbench_protocol::{
    WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason, WorkbenchTerminalOutcome,
};

/// Creates one Tab in the fixture Workspace, settles its terminal as started, and closes it
/// through Close publication. Returns the create reference its binding used.
fn create_and_close(coordinator: &mut SessionCoordinator, base: u128) -> u128 {
    let committed = snapshot(coordinator);
    let context = committed.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let create = context_tab_command(
        coordinator,
        base,
        committed.revision.value(),
        context,
        "cycle",
    );
    let pending = dispatch_tab(coordinator, create, base + 1);
    let completion = immediate_completion(pending.command());
    let completed = coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    assert!(
        matches!(
            completed.result.outcome,
            WorkbenchCommandOutcome::TabCreated {
                terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
                ..
            }
        ),
        "create {base} must start: {:?}",
        completed.result.outcome
    );
    let (context, tab, panel) = workspace_leaf(coordinator);
    close_leaf(coordinator, context, tab, panel, base + 2);
    base + 1
}

#[test]
fn closing_terminals_keeps_creates_admitted_past_the_binding_bound() {
    let mut coordinator = session();
    let cycles = u128::try_from(MAXIMUM_TERMINAL_BINDINGS).unwrap() + 2;
    for cycle in 0..cycles {
        create_and_close(&mut coordinator, 0x10_0000 + cycle * 4);
    }
    assert!(!coordinator.terminals.at_binding_limit());
    // Each retired binding left the Worker one retirement to send, bounded like the bindings.
    let mut retirements = 0;
    while let Some(request) = coordinator.next_worker_retirement(&[worker(1)], expiry()) {
        assert_eq!(request.action, TerminalLifecycleAction::Retire);
        retirements += 1;
    }
    assert_eq!(retirements, MAXIMUM_TERMINAL_BINDINGS);
}

#[test]
fn a_retired_binding_reads_as_unknown_and_its_worker_retirement_is_generation_bound() {
    let mut coordinator = session();
    let retired = create_and_close(&mut coordinator, 0x20_0000);
    let start = |create: u128, request: u128| {
        ClientMessage::TerminalStart(
            TerminalStartRequest::new(id(request), id(create), geometry()).unwrap(),
        )
    };
    for create in [retired, 0x20_00ff] {
        assert!(matches!(
            coordinator.prepare(
                start(create, create + 0x1000),
                Some(worker(1)),
                Some(expiry()),
                &LaunchCapture::absent()
            ),
            PreparedOperation::Ready(ServerMessage::TerminalStartResult(result))
                if result.outcome() == StartOutcome::Rejected {
                    reason: StartRejectedReason::TerminalCreateUnavailable
                }
        ));
    }
    assert_eq!(coordinator.viewer_target(&id(retired), worker(1)), None);
    let request = coordinator
        .next_worker_retirement(&[worker(1)], expiry())
        .expect("the Worker that disposed the terminal is told to forget it");
    assert_eq!(request.action, TerminalLifecycleAction::Retire);
    assert_eq!(
        coordinator.next_worker_retirement(&[worker(1)], expiry()),
        None
    );
    create_and_close(&mut coordinator, 0x20_0010);
    // A new Worker generation never held that Disposed record, so nothing is sent to it.
    assert_eq!(
        coordinator.next_worker_retirement(&[worker(2)], expiry()),
        None
    );
}

#[test]
fn a_create_refused_at_the_binding_bound_names_the_terminal_limit() {
    let mut coordinator = session();
    for reference in 0..u128::try_from(MAXIMUM_TERMINAL_BINDINGS).unwrap() {
        dispatch(coordinator.prepare(
            create(0x30_0000 + reference),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
    }
    assert!(coordinator.terminals.at_binding_limit());
    let committed = snapshot(&mut coordinator);
    let command = context_tab_command(
        &coordinator,
        0x31_0000,
        committed.revision.value(),
        committed.navigation.workspaces[0]
            .terminal_context_id
            .clone(),
        "full",
    );
    let PreparedTerminalTabCreate::Ready(result) = coordinator.prepare_terminal_tab_create(
        command,
        CreateInputs::new(
            Some(id(0x31_0001)),
            Some(expiry()),
            Some(worker(1)),
            LaunchCapture::absent(),
        ),
    ) else {
        panic!("a full Session must refuse before dispatch")
    };
    assert_eq!(
        result.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalLimitReached
        }
    );
}
