use super::*;
use crate::local_client_service::tests::handshake_client;
use orchbee_connection_handshake::{
    AcceptorMessage, EnvironmentName, InitiatorMessage, Offer, ProcessState,
    RejectionReason as Refusal,
};
use orchbee_local_client_protocol::{
    LocalClientMessage, RejectionReason, SessionStopRequest, SessionStopStatus, StopAction, StopId,
    read_message, write_message,
};
use std::io::Read;

fn connect(endpoint: &Path) -> UnixStream {
    let client = UnixStream::connect(endpoint).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    client
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    client
}

/// Answers the offer, then sends one Client phase message and reads its reply.
fn exchange(endpoint: &Path, request: LocalClientMessage) -> LocalClientMessage {
    let mut client = connect(endpoint);
    handshake_client::answered(&mut client).unwrap();
    write_message(&mut client, &request).unwrap();
    read_message(&mut client).unwrap()
}

#[test]
fn control_fences_admission_freezes_new_hellos_and_joins_idle_and_partial_clients() {
    let endpoint = crate::tests::endpoint();
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        crate::worker_registry::WorkerRegistry::new(),
    ))));
    let agents = Arc::new(
        AgentService::open(crate::agent_service::AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let shutdown = Arc::new(SessionShutdown::default());
    let mut service = start(
        &endpoint,
        ServerIdentity::local(None, None),
        terminals,
        agents,
        Arc::clone(&shutdown),
    )
    .unwrap();
    let mut admitted = connect(&endpoint);
    handshake_client::admit(&mut admitted).unwrap();
    // A peer from another environment never reaches the stop owner.
    let mut foreign = handshake_client::offer();
    foreign.environment = Offer::local(
        orchbee_connection_handshake::ConnectionKind::ClientServer,
        foreign.product_version.clone(),
        foreign.build.clone(),
        EnvironmentName::new("ob-test-ffffffffffffffff").unwrap(),
    )
    .environment;
    assert!(matches!(
        handshake_client::exchange(&mut connect(&endpoint), &InitiatorMessage::Offer(foreign)),
        Ok(AcceptorMessage::Rejection(rejection)) if rejection.reason == Refusal::EnvironmentMismatch
    ));
    assert!(shutdown.enter().is_some());
    let request = SessionStopRequest {
        id: StopId::new([1; 16]).unwrap(),
        action: StopAction::Start,
    };
    assert!(
        matches!(exchange(&endpoint, LocalClientMessage::SessionStopRequest(request)), LocalClientMessage::SessionStopResult(result) if matches!(result.status, SessionStopStatus::Pending { .. }))
    );
    // A recorded stop still answers offers, refuses the Client phase, and reports `stopping`.
    assert_eq!(
        exchange(&endpoint, LocalClientMessage::ClientHello),
        LocalClientMessage::Rejected {
            reason: RejectionReason::Stopping
        }
    );
    let reply = handshake_client::query(
        &mut connect(&endpoint),
        orchbee_local_runtime::environment_name(),
    )
    .unwrap();
    let result = handshake_client::query_result(reply).unwrap();
    assert_eq!(result.processes()[0].state, ProcessState::Stopping);
    let mut partial = UnixStream::connect(&endpoint).unwrap();
    partial
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    service
        .stop_before(Instant::now() + Duration::from_secs(5))
        .unwrap();
    let mut byte = [0];
    assert_eq!(admitted.read(&mut byte).unwrap(), 0);
    assert_eq!(partial.read(&mut byte).unwrap(), 0);
    assert!(service.control.streams.lock().unwrap().is_empty());
    assert!(service.thread.is_none());
}
