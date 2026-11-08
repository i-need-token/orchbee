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

    // Drain and exit arrive with Server drain; an unknown request type is unsupported too.
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
