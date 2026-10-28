use super::*;
use crate::test_support::WorkbenchFixture;
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState,
    HostTerminalId, HostTerminalIdentity, TerminalLaunchFailure, TerminalLaunchOutcome,
    TerminalLaunchResult, TerminalOperationAuthorization, WorkerGeneration, WorkerId,
};
use orchbee_client_server_protocol::{
    CreateFailureReason, Geometry, StartMode, TerminalCreateRequest,
};
use orchbee_workbench_protocol::{
    PanelNode, ProtocolVersion, SessionId, Snapshot, SnapshotReadIntent, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchSnapshotOutcome,
};
use std::num::NonZeroU64;

mod agent_registration;
mod checkpoint_restore;
mod exit_close;
mod failed_launch_close;
mod launch_read;
mod multileaf_membership;
mod panel_split;
mod tab_create;
mod terminal_selection;

fn id(value: u128) -> RequestId {
    RequestId::parse(&format!("{value:032x}")).unwrap()
}
fn session() -> SessionCoordinator {
    SessionCoordinator::new(Some(
        WorkbenchAuthority::workspace_fixture(SessionId::parse(&format!("{:032x}", 999)).unwrap())
            .unwrap(),
    ))
}
/// A Session without the fixture Workspace, for scenarios whose configured Workspace is the only one.
fn empty_session() -> SessionCoordinator {
    SessionCoordinator::new(Some(
        WorkbenchAuthority::new(SessionId::parse(&format!("{:032x}", 999)).unwrap()).unwrap(),
    ))
}
fn worker(generation: u64) -> WorkerPlacement {
    WorkerPlacement {
        worker_id: WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
    }
}
fn expiry() -> LeaseExpiresAtUnixMilliseconds {
    LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(10_000).unwrap())
}
fn geometry() -> Geometry {
    Geometry::new(80, 24, 8, 16).unwrap()
}
fn create(value: u128) -> ClientMessage {
    ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        id(value),
        None,
        None,
        StartMode::Deferred,
        None,
    ))
}
fn dispatch(prepared: PreparedOperation) -> Box<PendingOperation> {
    match prepared {
        PreparedOperation::Dispatch(pending) => pending,
        other => panic!("expected dispatch: {other:?}"),
    }
}
fn identity() -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([3; 16]).unwrap(),
        HostEpoch::from_bytes([4; 16]).unwrap(),
        HostTerminalId::from_bytes([5; 16]).unwrap(),
    )
}
fn launch_authorization(command: &CapabilityNodeMessage) -> TerminalOperationAuthorization {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(request) = command else {
        panic!("expected launch")
    };
    request.authorization()
}
fn snapshot(session: &mut SessionCoordinator) -> Snapshot {
    let response = session
        .execute_workbench(
            WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 888)).unwrap(),
            })
            .try_into()
            .unwrap(),
            false,
        )
        .unwrap();
    let WorkbenchResponse::Snapshot(result) = response else {
        panic!("expected snapshot result")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("expected available snapshot")
    };
    snapshot
}
fn assert_membership(session: &mut SessionCoordinator, count: usize) {
    let snapshot = snapshot(session);
    assert_eq!(snapshot.terminal_contexts.len(), 1);
    assert_eq!(snapshot.terminal_contexts[0].tabs.len(), count);
    assert_eq!(snapshot.revision.value(), u64::try_from(count).unwrap());
}

#[test]
fn close_session_identity_remains_observable_after_publication_retirement() {
    use orchbee_workbench_protocol::{
        WorkbenchAction, WorkbenchCommand, WorkbenchTabMoveDirection,
    };
    let mut coordinator = session();
    coordinator
        .workbench
        .as_mut()
        .unwrap()
        .append_fixture_tab(PresentationBinding {
            terminal_id: TerminalId::parse("11111111111111111111111111111111").unwrap(),
            worker_id: orchbee_workbench_protocol::WorkerId::parse(
                "22222222222222222222222222222222",
            )
            .unwrap(),
        })
        .unwrap();
    let before = snapshot(&mut coordinator);
    let authority = coordinator.workbench.as_mut().unwrap();
    let grant = WorkbenchGrant::manage_navigation(before.session_id.clone());
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 500)).unwrap(),
        session_id: before.session_id.clone(),
        expected_revision: before.revision,
        action: WorkbenchAction::TabMove {
            terminal_context_id: before.navigation.workspaces[0].terminal_context_id.clone(),
            tab_id: before.terminal_contexts[0].tabs[0].tab_id.clone(),
            direction: WorkbenchTabMoveDirection::First,
        },
    };
    let orchbee_server::WorkbenchTabMoveBegin::Admitted(admission) =
        authority.begin_terminal_tab_move(Some(&grant), command)
    else {
        panic!("Move admission expected")
    };
    assert!(matches!(
        admission.retire_composition_failure().outcome,
        orchbee_workbench_protocol::WorkbenchCommandOutcome::Unavailable { .. }
    ));
    assert_eq!(
        coordinator.workbench_session_matches(&before.session_id),
        Some(true)
    );
    assert_eq!(
        coordinator.workbench_session_matches(&SessionId::parse(&format!("{:032x}", 501)).unwrap()),
        Some(false)
    );
    let response = coordinator
        .execute_workbench(
            WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 502)).unwrap(),
            })
            .try_into()
            .unwrap(),
            false,
        )
        .unwrap();
    assert!(matches!(
        response,
        WorkbenchResponse::Snapshot(orchbee_workbench_protocol::WorkbenchSnapshotResult {
            outcome: WorkbenchSnapshotOutcome::Unavailable { .. },
            ..
        })
    ));
}
