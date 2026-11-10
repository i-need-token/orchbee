//! The Client endpoint's handshake admission: each refusal reaches the candidate with this
//! Server's facts, and only an answered offer enters the Client phase.

use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use orchbee_connection_handshake::{
    AcceptorMessage, AcceptorProfile, Build, BuildEnvironment, ConnectionKind, ControlRequest,
    EnvironmentName, Extensions, Fingerprint, Generation, GenerationRange, InitiatorMessage, Offer,
    ProductVersion, Rejection, RejectionReason as Refusal, RequestKind, RequestName,
    UnknownRequest,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::LocalStream as UnixStream;

use super::super::{ConnectionSlots, ServerIdentity, handle_candidate};
use super::handshake_client;
use crate::{
    agent_service::{AgentPolicy, AgentService},
    client_connection_registry::{
        ClientConnectionAdmission, ClientConnectionRegistry, MAXIMUM_ACTIVE_CLIENT_CONNECTIONS,
    },
    local_terminal_service::LocalTerminalService,
    workbench_service::{WorkbenchPolicy, WorkbenchService},
    worker_registry::WorkerRegistry,
};

/// Runs one candidate against `identity` and `registry`; the returned stream is the Client side.
fn candidate(
    identity: ServerIdentity,
    registry: Arc<Mutex<ClientConnectionRegistry>>,
) -> (UnixStream, thread::JoinHandle<()>) {
    let (client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let handler = thread::spawn(move || {
        let terminals = LocalTerminalService::new(Arc::new(Mutex::new(WorkerRegistry::new())));
        let terminals = Arc::new(terminals);
        let workbench = WorkbenchService::new(
            WorkbenchPolicy {
                read: true,
                manage_navigation: false,
                associate_directory: false,
                create_terminal: false,
                manage_terminal_lifecycle: false,
            },
            Arc::clone(&terminals),
        );
        let agents = AgentService::open(AgentPolicy {
            read: true,
            manage: false,
        })
        .unwrap();
        let slots = Arc::new(ConnectionSlots::new());
        handle_candidate(
            server,
            &registry,
            &identity,
            slots.acquire().unwrap(),
            &terminals,
            &workbench,
            &agents,
        );
    });
    (client, handler)
}

fn fresh() -> Arc<Mutex<ClientConnectionRegistry>> {
    Arc::new(Mutex::new(ClientConnectionRegistry::new()))
}

fn range(lowest: u16, highest: u16) -> GenerationRange {
    GenerationRange::new(
        Generation::new(lowest).unwrap(),
        Generation::new(highest).unwrap(),
    )
    .unwrap()
}

fn version(text: &str) -> ProductVersion {
    ProductVersion::new(text).unwrap()
}

/// A release Server that supports `generations`.
fn release_server(generations: GenerationRange) -> ServerIdentity {
    let mut profile = AcceptorProfile::local(
        ConnectionKind::ClientServer,
        version("2.0.0"),
        Build::default(),
        EnvironmentName::release(),
    );
    profile.generations = generations;
    ServerIdentity {
        profile,
        worker: None,
    }
}

fn release_offer(generations: GenerationRange) -> Offer {
    let mut offer = Offer::local(
        ConnectionKind::ClientServer,
        version("1.0.0"),
        Build::default(),
        EnvironmentName::release(),
    );
    offer.generations = generations;
    offer
}

fn refused(reply: AcceptorMessage) -> Rejection {
    match reply {
        AcceptorMessage::Rejection(rejection) => rejection,
        other => panic!("expected a rejection: {other:?}"),
    }
}

/// Offers and expects a refusal before the Client phase, then the closed connection.
fn refusal(
    identity: ServerIdentity,
    registry: Arc<Mutex<ClientConnectionRegistry>>,
    offer: Offer,
) -> Rejection {
    let (mut client, handler) = candidate(identity, registry);
    let reply = handshake_client::exchange(&mut client, &InitiatorMessage::Offer(offer)).unwrap();
    assert_eq!(client.read(&mut [0]).unwrap(), 0);
    handler.join().unwrap();
    refused(reply)
}

#[test]
fn a_release_server_uses_the_highest_common_generation_and_names_the_older_side() {
    for (offered, generation) in [(range(1, 5), 3), (range(1, 2), 2)] {
        let (mut client, handler) = candidate(release_server(range(2, 3)), fresh());
        let reply = handshake_client::exchange(
            &mut client,
            &InitiatorMessage::Offer(release_offer(offered)),
        )
        .unwrap();
        let AcceptorMessage::Answer(answer) = reply else {
            panic!("expected an answer: {reply:?}")
        };
        assert_eq!(answer.generation.get(), generation);
        assert_eq!(answer.product_version, version("2.0.0"));
        write_message(&mut client, &LocalClientMessage::ClientHello).unwrap();
        assert!(matches!(
            read_message(&mut client).unwrap(),
            LocalClientMessage::ClientAccepted { .. }
        ));
        drop(client);
        handler.join().unwrap();
    }
    for (offered, reason) in [
        (range(1, 1), Refusal::InitiatorTooOld),
        (range(4, 5), Refusal::InitiatorTooNew),
    ] {
        let rejection = refusal(release_server(range(2, 3)), fresh(), release_offer(offered));
        assert_eq!(rejection.reason, reason);
        // The refusal carries the Server's facts, so the Client can name the older side.
        assert_eq!(rejection.generations, range(2, 3));
        assert_eq!(rejection.product_version, version("2.0.0"));
    }
}

#[test]
fn another_environment_and_another_development_build_are_refused() {
    let ours = orchbee_local_runtime::environment_name();
    let other = ["ob-test-ffffffffffffffff", "ob-test-eeeeeeeeeeeeeeee"]
        .into_iter()
        .map(|name| EnvironmentName::new(name).unwrap())
        .find(|name| *name != ours)
        .unwrap();
    let mut offer = handshake_client::offer();
    offer.environment =
        BuildEnvironment::development(other.clone(), Fingerprint::from_bytes([1; 32])).unwrap();
    let rejection = refusal(ServerIdentity::local(None, None), fresh(), offer);
    assert_eq!(rejection.reason, Refusal::EnvironmentMismatch);
    assert_eq!(rejection.environment, ours);

    let (mut client, handler) = candidate(ServerIdentity::local(None, None), fresh());
    let reply = handshake_client::query(&mut client, other).unwrap();
    handler.join().unwrap();
    assert_eq!(refused(reply).reason, Refusal::EnvironmentMismatch);

    let mut offer = handshake_client::offer();
    let mut fingerprint = *offer.environment.fingerprint().unwrap().as_bytes();
    fingerprint[0] ^= 1;
    offer.environment =
        BuildEnvironment::development(ours, Fingerprint::from_bytes(fingerprint)).unwrap();
    let rejection = refusal(ServerIdentity::local(None, None), fresh(), offer);
    assert_eq!(rejection.reason, Refusal::FingerprintMismatch);
}

#[test]
fn a_full_session_refuses_offers_with_capacity() {
    let registry = fresh();
    for _ in 0..MAXIMUM_ACTIVE_CLIENT_CONNECTIONS {
        assert!(matches!(
            registry.lock().unwrap().admit(),
            ClientConnectionAdmission::Accepted { .. }
        ));
    }
    let before = registry.lock().unwrap().clone();
    let rejection = refusal(
        ServerIdentity::local(None, None),
        Arc::clone(&registry),
        handshake_client::offer(),
    );
    assert_eq!(rejection.reason, Refusal::Capacity);
    assert_eq!(*registry.lock().unwrap(), before);
}

#[test]
fn a_pre_handshake_peer_and_unserved_requests_close_with_one_reply() {
    // A Client from before the handshake opens with an OBLC frame.
    let (mut client, handler) = candidate(ServerIdentity::local(None, None), fresh());
    write_message(&mut client, &LocalClientMessage::ClientHello).unwrap();
    let reply = orchbee_connection_handshake::read_acceptor_message(&mut client).unwrap();
    handler.join().unwrap();
    assert_eq!(refused(reply).reason, Refusal::Malformed);

    // A Server without a lifecycle owner serves neither drain nor exit; an unknown request type
    // is unsupported too.
    let environment = orchbee_local_runtime::environment_name();
    let unknown = UnknownRequest::new(RequestName::new("rotate").unwrap(), environment.clone());
    for (name, request) in [
        ("drain", control(RequestKind::Drain, &environment)),
        ("exit", control(RequestKind::Exit, &environment)),
        ("rotate", InitiatorMessage::UnknownRequest(unknown.unwrap())),
    ] {
        let (mut client, handler) = candidate(ServerIdentity::local(None, None), fresh());
        let reply = handshake_client::exchange(&mut client, &request).unwrap();
        handler.join().unwrap();
        assert!(
            matches!(&reply, AcceptorMessage::Unsupported(reply) if reply.request.as_str() == name),
            "{name}: {reply:?}"
        );
    }
}

fn control(kind: RequestKind, environment: &EnvironmentName) -> InitiatorMessage {
    InitiatorMessage::Request(ControlRequest {
        kind,
        environment: environment.clone(),
        extensions: Extensions::new(),
    })
}

#[test]
fn a_silent_candidate_is_refused_with_timeout() {
    let (mut client, handler) = candidate(ServerIdentity::local(None, None), fresh());
    client.flush().unwrap();
    let reply = orchbee_connection_handshake::read_acceptor_message(&mut client).unwrap();
    handler.join().unwrap();
    assert_eq!(refused(reply).reason, Refusal::Timeout);
}

/// Runs one candidate against a managed lifecycle owner; the returned stream is the Client side.
/// With `session`, the Server holds that Session's fixture Workspace.
#[cfg(windows)]
fn managed_candidate(
    shutdown: Arc<crate::session_shutdown::SessionShutdown>,
    session: Option<orchbee_workbench_protocol::SessionId>,
) -> (UnixStream, thread::JoinHandle<()>) {
    let (client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let handler = thread::spawn(move || {
        let workers = Arc::new(Mutex::new(WorkerRegistry::new()));
        let terminals = Arc::new(match session {
            Some(session) => LocalTerminalService::session_workspace_fixture(workers, session),
            None => LocalTerminalService::new(workers),
        });
        let workbench = WorkbenchService::new(
            WorkbenchPolicy {
                read: true,
                manage_navigation: false,
                associate_directory: false,
                create_terminal: false,
                manage_terminal_lifecycle: false,
            },
            Arc::clone(&terminals),
        );
        let agents = AgentService::open(AgentPolicy {
            read: true,
            manage: false,
        })
        .unwrap();
        let slots = Arc::new(ConnectionSlots::new());
        super::super::handle_candidate_control(
            server,
            &fresh(),
            &ServerIdentity::local(None, None),
            slots.acquire().unwrap(),
            &terminals,
            &workbench,
            super::super::stop_control::StopControl {
                agents: &agents,
                shutdown: Some(&shutdown),
            },
        );
    });
    (client, handler)
}

#[cfg(windows)]
#[test]
fn a_drained_server_refuses_offers_with_stopping_and_drain_repeats() {
    use orchbee_connection_handshake::ProcessState;
    let shutdown = Arc::new(crate::session_shutdown::SessionShutdown::default());
    let environment = orchbee_local_runtime::environment_name();
    let in_flight = shutdown.enter().unwrap();
    for _ in 0..2 {
        let (mut client, handler) = managed_candidate(Arc::clone(&shutdown), None);
        let reply =
            handshake_client::exchange(&mut client, &control(RequestKind::Drain, &environment))
                .unwrap();
        handler.join().unwrap();
        assert!(
            matches!(&reply, AcceptorMessage::DrainResult(result) if result.state == ProcessState::Draining),
            "{reply:?}"
        );
    }
    let (mut client, handler) = managed_candidate(Arc::clone(&shutdown), None);
    let reply = handshake_client::exchange(
        &mut client,
        &InitiatorMessage::Offer(handshake_client::offer()),
    )
    .unwrap();
    handler.join().unwrap();
    assert_eq!(refused(reply).reason, Refusal::Stopping);
    let (mut client, handler) = managed_candidate(Arc::clone(&shutdown), None);
    let reply = handshake_client::query(&mut client, environment).unwrap();
    handler.join().unwrap();
    let result = handshake_client::query_result(reply).unwrap();
    assert_eq!(result.processes()[0].state, ProcessState::Draining);
    // Work admitted before the drain still settles.
    assert!(!shutdown.settle_records(std::time::Instant::now()));
    drop(in_flight);
    assert!(shutdown.settle_records(std::time::Instant::now()));
}

#[cfg(windows)]
#[test]
fn exit_replies_only_once_the_owner_ends_the_process() {
    use crate::session_shutdown::Work;
    let shutdown = Arc::new(crate::session_shutdown::SessionShutdown::default());
    let environment = orchbee_local_runtime::environment_name();
    let (mut client, handler) = managed_candidate(Arc::clone(&shutdown), None);
    let owner = {
        let shutdown = Arc::clone(&shutdown);
        thread::spawn(move || {
            while shutdown.take_within(Duration::from_millis(10)) != Some(Work::Exit) {}
            // Offers meet an exiting Server before its end.
            let (mut client, handler) = managed_candidate(Arc::clone(&shutdown), None);
            let reply = handshake_client::exchange(
                &mut client,
                &InitiatorMessage::Offer(handshake_client::offer()),
            )
            .unwrap();
            handler.join().unwrap();
            assert_eq!(refused(reply).reason, Refusal::Stopping);
            assert!(shutdown.end(std::time::Instant::now() + Duration::from_secs(5)));
        })
    };
    let reply =
        handshake_client::exchange(&mut client, &control(RequestKind::Exit, &environment)).unwrap();
    assert!(matches!(reply, AcceptorMessage::ExitResult(_)), "{reply:?}");
    handler.join().unwrap();
    owner.join().unwrap();
}

#[cfg(windows)]
fn session_stop(session: &str, stop: &str) -> orchbee_workbench_protocol::WorkbenchRequest {
    orchbee_workbench_protocol::parse_workbench_request(
        format!(
            r#"{{"kind":"workbench.session-stop","protocolVersion":1,"requestId":"33333333333333333333333333333333","sessionId":"{session}","stopId":"{stop}"}}"#
        )
        .as_bytes(),
    )
    .unwrap()
}

#[cfg(windows)]
fn session_stop_outcome(
    client: &mut UnixStream,
    request: &orchbee_workbench_protocol::WorkbenchRequest,
) -> Result<
    orchbee_workbench_protocol::SessionStopOutcome,
    orchbee_workbench_protocol::WorkbenchWireError,
> {
    orchbee_workbench_protocol::write_workbench_request(client, request)?;
    match orchbee_workbench_protocol::read_workbench_response(client)? {
        orchbee_workbench_protocol::WorkbenchResponse::SessionStop(result) => Ok(result.outcome),
        other => panic!("{other:?}"),
    }
}

/// An admitted Client records a stop exactly as `server stop` would, for its own Session only, and
/// the connection admits nothing once the stop is recorded.
#[cfg(windows)]
#[test]
fn an_admitted_client_records_its_session_stop_once() {
    use crate::session_shutdown::Work;
    use orchbee_local_client_protocol::{
        SessionStopRequest, SessionStopStatus, StopAction, StopId,
    };
    use orchbee_workbench_protocol::{SessionStopOutcome, SessionStopRejection};
    const SESSION: &str = "22222222222222222222222222222222";
    const STOP: &str = "0123456789abcdef0123456789abcdef";
    let shutdown = Arc::new(crate::session_shutdown::SessionShutdown::default());
    let session = orchbee_workbench_protocol::SessionId::parse(SESSION).unwrap();
    let (mut client, handler) = managed_candidate(Arc::clone(&shutdown), Some(session));
    handshake_client::admit(&mut client).unwrap();

    let other_session = session_stop("44444444444444444444444444444444", STOP);
    assert_eq!(
        session_stop_outcome(&mut client, &other_session).unwrap(),
        SessionStopOutcome::Rejected {
            reason: SessionStopRejection::SessionMismatch
        }
    );
    assert!(!shutdown.stopping());
    assert_eq!(
        session_stop_outcome(&mut client, &session_stop(SESSION, STOP)).unwrap(),
        SessionStopOutcome::Recorded {}
    );
    // The recorded stop refuses the next record, a repeat included, by ending the connection.
    assert!(session_stop_outcome(&mut client, &session_stop(SESSION, STOP)).is_err());
    handler.join().unwrap();

    let id = StopId::new(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef_u128.to_be_bytes()).unwrap();
    let Some(Work::Stop(attempt)) = shutdown.take_within(Duration::ZERO) else {
        panic!("the recorded stop is queued");
    };
    assert_eq!(attempt.id, id);
    assert_eq!(shutdown.take_within(Duration::ZERO), None);
    // `server stop-status` reads it under the same identity, and another identity finds it busy.
    let status = shutdown.request(&SessionStopRequest {
        id,
        action: StopAction::Status,
    });
    assert!(
        matches!(status.status, SessionStopStatus::Pending { .. }),
        "{status:?}"
    );
    let other = StopId::new([7; 16]).unwrap();
    let busy = shutdown.request(&SessionStopRequest {
        id: other,
        action: StopAction::Start,
    });
    assert!(matches!(busy.status, SessionStopStatus::Busy { active_id } if active_id == id));
}

#[cfg(windows)]
#[test]
fn a_drained_server_records_no_session_stop() {
    const SESSION: &str = "22222222222222222222222222222222";
    let shutdown = Arc::new(crate::session_shutdown::SessionShutdown::default());
    let session = orchbee_workbench_protocol::SessionId::parse(SESSION).unwrap();
    let (mut client, handler) = managed_candidate(Arc::clone(&shutdown), Some(session));
    handshake_client::admit(&mut client).unwrap();
    shutdown.drain();
    let stop = session_stop(SESSION, "0123456789abcdef0123456789abcdef");
    assert!(session_stop_outcome(&mut client, &stop).is_err());
    handler.join().unwrap();
    assert!(!shutdown.stopping());
}
