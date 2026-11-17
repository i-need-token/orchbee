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
use std::{io::Read, path::Path};

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

/// Steps that fail once each time the test arms them: a failed accept drops the candidate it was
/// admitting, and a failed spawn drops the handler it was handed.
fn faulty_steps(accept_fault: Arc<AtomicBool>, spawn_fault: Arc<AtomicBool>) -> Steps {
    Steps {
        accept: Box::new(move |listener| {
            let (stream, ()) = listener.accept()?;
            if accept_fault.swap(false, Ordering::AcqRel) {
                drop(stream);
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            Ok(stream)
        }),
        spawn: Box::new(move |handler| {
            if spawn_fault.swap(false, Ordering::AcqRel) {
                drop(handler);
                return Err(io::ErrorKind::OutOfMemory.into());
            }
            thread::Builder::new()
                .name("orchbee-local-client".into())
                .spawn(handler)
        }),
    }
}

/// Reads the Workbench snapshot on an admitted connection; any snapshot answer proves service.
fn answers_snapshot_read(client: &mut UnixStream) -> bool {
    use orchbee_workbench_protocol::{
        WorkbenchResponse, parse_workbench_request, read_workbench_response,
        write_workbench_request,
    };
    let read = parse_workbench_request(
        br#"{"kind":"workbench.snapshot.read","protocolVersion":1,"requestId":"11111111111111111111111111111111"}"#,
    )
    .unwrap();
    write_workbench_request(client, &read).is_ok()
        && matches!(
            read_workbench_response(client),
            Ok(WorkbenchResponse::Snapshot(_))
        )
}

#[test]
fn one_failed_accept_and_one_failed_handler_start_drop_only_their_candidates() {
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
    let accept_fault = Arc::new(AtomicBool::new(false));
    let spawn_fault = Arc::new(AtomicBool::new(false));
    let mut service = start_with(
        &endpoint,
        ServerIdentity::local(None, None),
        terminals,
        agents,
        Arc::new(SessionShutdown::default()),
        Arc::default(),
        faulty_steps(Arc::clone(&accept_fault), Arc::clone(&spawn_fault)),
    )
    .unwrap();
    let mut admitted = connect(&endpoint);
    handshake_client::admit(&mut admitted).unwrap();
    assert!(answers_snapshot_read(&mut admitted));

    accept_fault.store(true, Ordering::Release);
    assert!(handshake_client::answered(&mut connect(&endpoint)).is_err());
    assert!(!accept_fault.load(Ordering::Acquire));
    spawn_fault.store(true, Ordering::Release);
    assert!(handshake_client::answered(&mut connect(&endpoint)).is_err());
    assert!(!spawn_fault.load(Ordering::Acquire));

    assert!(answers_snapshot_read(&mut admitted));
    let mut later = connect(&endpoint);
    handshake_client::admit(&mut later).unwrap();
    assert!(answers_snapshot_read(&mut later));
    service
        .stop_before(Instant::now() + Duration::from_secs(5))
        .unwrap();
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
        Arc::default(),
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
    assert!(service.control.streams.lock().unwrap().open.is_empty());
    assert!(service.thread.is_none());
}
