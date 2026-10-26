use super::*;
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewCompletion, ClientViewCompletionOutcome, ClientViewControl,
    ClientViewControlReason, ClientViewControlStatus, ClientViewIntent, ClientViewObservation,
    ClientViewOutcome, ClientViewPhase, ClientViewSelection, ClientViewSequence,
    WorkbenchClientViewRequest,
};
use std::num::NonZeroU64;

fn execute(
    connection: &WorkbenchClientConnection<'_>,
    session: &orchbee_workbench_protocol::SessionId,
    action: ClientViewAction,
) -> ClientViewOutcome {
    execute_lane(
        connection,
        session,
        action,
        orchbee_workbench_protocol::MultiplexLaneId::ROOT,
    )
}
fn execute_lane(
    connection: &WorkbenchClientConnection<'_>,
    session: &orchbee_workbench_protocol::SessionId,
    action: ClientViewAction,
    lane: orchbee_workbench_protocol::MultiplexLaneId,
) -> ClientViewOutcome {
    let WorkbenchResponse::ClientView(result) = response(connection.execute_scoped(
        WorkbenchRequest::ClientView(WorkbenchClientViewRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 900)).unwrap(),
            session_id: session.clone(),
            action,
        }),
        lane,
    )) else {
        panic!("Client response");
    };
    result.validate().unwrap();
    result.outcome
}
fn service(read: bool, navigate: bool) -> WorkbenchService {
    WorkbenchService::new(
        WorkbenchPolicy {
            read,
            manage_navigation: navigate,
            associate_directory: false,
            create_terminal: false,
            manage_terminal_lifecycle: false,
        },
        Arc::new(LocalTerminalService::workspace_fixture(Arc::new(
            Mutex::new(WorkerRegistry::new()),
        ))),
    )
}
fn bootstrap(service: &WorkbenchService) -> orchbee_workbench_protocol::Snapshot {
    let WorkbenchResponse::Snapshot(result) = response(service.execute(
        WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 901)).unwrap(),
        }),
    )) else {
        panic!("snapshot");
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available");
    };
    snapshot
}
fn sample(
    snapshot: &orchbee_workbench_protocol::Snapshot,
    completion: Option<ClientViewCompletion>,
) -> ClientViewAction {
    ClientViewAction::Synchronize {
        heartbeat: ClientViewSequence::new(1).unwrap(),
        view_revision: ClientViewSequence::new(1).unwrap(),
        view: Box::new(ClientViewObservation {
            workbench_revision: snapshot.revision,
            split_revision: None,
            phase: ClientViewPhase::Ready,
            selection: Some(ClientViewSelection {
                terminal_context_id: snapshot.navigation.workspaces[0]
                    .terminal_context_id
                    .clone(),
                tab_id: None,
                panel_id: None,
                zoomed: false,
                panel_count: 0,
            }),
        }),
        completion,
    }
}
#[test]
fn client_view_sessions_bind_identity_route_exactly_and_settle_disconnect() {
    let service = service(true, true);
    let snapshot = bootstrap(&service);
    let session = &snapshot.session_id;
    let first = service.client_connection(NonZeroU64::new(1).unwrap());
    let publisher = orchbee_workbench_protocol::MultiplexLaneId::new(2).unwrap();
    let second = service.client_connection(NonZeroU64::new(2).unwrap());
    let caller = service.client_connection(NonZeroU64::new(3).unwrap());
    let ClientViewOutcome::Synchronized {
        client: first_view, ..
    } = execute_lane(&first, session, sample(&snapshot, None), publisher)
    else {
        panic!("first view");
    };
    let ClientViewOutcome::Synchronized {
        client: second_view,
        ..
    } = execute(&second, session, sample(&snapshot, None))
    else {
        panic!("second view");
    };
    assert_eq!(first_view.client_id.as_str(), format!("{:032x}", 1));
    assert_ne!(first_view.client_id, second_view.client_id);
    let command = ClientViewControl {
        operation_id: WorkbenchRequestId::parse(&format!("{:032x}", 904)).unwrap(),
        client_id: first_view.client_id.clone(),
        expected_view_revision: first_view.view_revision,
        minimum_workbench_revision: snapshot.revision,
        intent: ClientViewIntent::Context {
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
        },
    };
    let submit = ClientViewAction::Submit {
        command: Box::new(command.clone()),
    };
    assert!(
        matches!(execute(&caller,session,submit.clone()),ClientViewOutcome::Operation { operation } if operation.status == ClientViewControlStatus::Queued {})
    );
    assert!(matches!(
        execute(&second, session, sample(&snapshot, None)),
        ClientViewOutcome::Synchronized { control: None, .. }
    ));
    assert!(
        matches!(execute_lane(&first,session,sample(&snapshot,None),publisher),ClientViewOutcome::Synchronized { control:Some(delivered),.. } if *delivered==command)
    );
    assert!(matches!(
        execute(
            &second,
            session,
            sample(
                &snapshot,
                Some(ClientViewCompletion {
                    operation_id: command.operation_id.clone(),
                    outcome: ClientViewCompletionOutcome::Applied {}
                })
            )
        ),
        ClientViewOutcome::Conflict {}
    ));
    let wrong_session =
        orchbee_workbench_protocol::SessionId::parse(&format!("{:032x}", 905)).unwrap();
    assert!(matches!(
        execute(&caller, &wrong_session, ClientViewAction::List {}),
        ClientViewOutcome::Denied {}
    ));
    let sibling = orchbee_workbench_protocol::MultiplexLaneId::new(3).unwrap();
    assert!(matches!(
        execute_lane(&first, session, sample(&snapshot, None), sibling),
        ClientViewOutcome::Conflict {}
    ));
    first.close_lane(sibling);
    assert!(matches!(
        execute(&caller, session, ClientViewAction::Read { operation_id: command.operation_id.clone() }),
        ClientViewOutcome::Operation { operation } if operation.status == ClientViewControlStatus::Delivered {}
    ));
    first.close_lane(publisher);
    assert!(
        matches!(execute(&caller,session,submit),ClientViewOutcome::Operation { operation } if operation.status==ClientViewControlStatus::Incomplete { reason:ClientViewControlReason::ClientDisconnected,view:None })
    );
    assert!(
        matches!(execute(&caller,session,ClientViewAction::List {}),ClientViewOutcome::Listed { clients } if clients.len()==1 && clients[0].client_id==second_view.client_id)
    );
    drop(first);
    let after = bootstrap(&service);
    assert_eq!(
        snapshot, after,
        "routing cannot mutate canonical navigation"
    );
}
#[test]
fn client_view_control_requires_navigation_permission_but_status_is_read_only() {
    let service = service(true, false);
    let snapshot = bootstrap(&service);
    let connection = service.client_connection(NonZeroU64::new(1).unwrap());
    let ClientViewOutcome::Synchronized { client, .. } =
        execute(&connection, &snapshot.session_id, sample(&snapshot, None))
    else {
        panic!("read publication");
    };
    let id = WorkbenchRequestId::parse(&format!("{:032x}", 906)).unwrap();
    assert!(matches!(
        execute(
            &connection,
            &snapshot.session_id,
            ClientViewAction::Submit {
                command: Box::new(ClientViewControl {
                    operation_id: id.clone(),
                    client_id: client.client_id,
                    expected_view_revision: client.view_revision,
                    minimum_workbench_revision: snapshot.revision,
                    intent: ClientViewIntent::Context {
                        terminal_context_id: snapshot.navigation.workspaces[0]
                            .terminal_context_id
                            .clone()
                    }
                })
            }
        ),
        ClientViewOutcome::Denied {}
    ));
    assert!(matches!(
        execute(
            &connection,
            &snapshot.session_id,
            ClientViewAction::Read { operation_id: id }
        ),
        ClientViewOutcome::NotFound {}
    ));
    let denied = super::client_view::service(false, false);
    let denied_session = denied.client_connection(NonZeroU64::new(2).unwrap());
    assert!(matches!(
        execute(
            &denied_session,
            &snapshot.session_id,
            ClientViewAction::List {}
        ),
        ClientViewOutcome::Denied {}
    ));
}
